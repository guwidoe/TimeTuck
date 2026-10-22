//! Full exact builtin summaries in stable private chunks, without per-row Rc.
//! This is neither compressed Event decoding nor an event-owner recycling pool.
use super::event_queue::QueuedEvent;
use super::*;
use crate::int_collector_counts::SummarySeed;
use std::any::TypeId;

type Pair = (usize, usize);
type Quad = (usize, usize, usize, usize);
type PairChange = TupleChange<Pair, (Pair, IntSummary)>;
type QuadChange = TupleChange<Quad, (Quad, IntSummary)>;
type PairSeed = TupleChange<Pair, (Pair, SummarySeed)>;
type QuadSeed = TupleChange<Quad, (Quad, SummarySeed)>;
#[cfg(test)]
type Raw<K> = [Option<TupleChange<K, IntSummary>>; 2];
const CHUNK: usize = 64;

// Chunk creation is lazy and rare. Keep its complete array temporary out of
// every scalar group update, retaining the original initialization order.
#[cold]
#[inline(never)]
fn empty_chunk<T>(empty: fn() -> T) -> Box<[T; CHUNK]> {
    Box::new(std::array::from_fn(|_| empty()))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    Pair,
    Quad,
}
pub(super) fn kind<K: 'static>() -> Option<Kind> {
    let ty = TypeId::of::<K>();
    if ty == TypeId::of::<Pair>() {
        Some(Kind::Pair)
    } else if ty == TypeId::of::<Quad>() {
        Some(Kind::Quad)
    } else {
        None
    }
}
pub(super) fn accepts<I: 'static, T: 'static>(kind: Kind) -> bool {
    match kind {
        Kind::Pair => {
            TypeId::of::<I>() == TypeId::of::<Pair>()
                && TypeId::of::<T>() == TypeId::of::<(Pair, IntSummary)>()
        }
        Kind::Quad => {
            TypeId::of::<I>() == TypeId::of::<Quad>()
                && TypeId::of::<T>() == TypeId::of::<(Quad, IntSummary)>()
        }
    }
}

// A reference view only: no ownership synthesis, decode allocation or Clone.
pub(super) enum SummaryRef<'a> {
    Pair(&'a PairChange),
    Quad(&'a QuadChange),
    PairSeed(&'a PairSeed),
    QuadSeed(&'a QuadSeed),
}
impl SummaryRef<'_> {
    pub(super) fn typed<I: 'static, T: 'static>(&self) -> &TupleChange<I, T> {
        // Concrete per-arm vtables retain the same foldable type checks as Event.
        match self {
            Self::Pair(row) => (*row as &dyn Any).downcast_ref(),
            Self::Quad(row) => (*row as &dyn Any).downcast_ref(),
            Self::PairSeed(_) | Self::QuadSeed(_) => {
                panic!("seed rows require scoped materialization")
            }
        }
        .expect("certified summary receiver")
    }
    pub(super) fn is_put(&self) -> bool {
        match self {
            Self::Pair(row) => row.value.is_some(),
            Self::Quad(row) => row.value.is_some(),
            Self::PairSeed(row) => row.value.is_some(),
            Self::QuadSeed(row) => row.value.is_some(),
        }
    }
}

struct Arena<T> {
    chunks: Vec<Box<[T; CHUNK]>>,
    len: usize,
}
impl<T> Default for Arena<T> {
    fn default() -> Self {
        Self {
            chunks: Vec::new(),
            len: 0,
        }
    }
}
impl<T> Arena<T> {
    #[inline(always)]
    fn push_with(&mut self, empty: fn() -> T, write: impl FnOnce(&mut T)) -> usize {
        let index = self.len;
        if index / CHUNK == self.chunks.len() {
            self.chunks.push(empty_chunk(empty));
        }
        write(&mut self.chunks[index / CHUNK][index % CHUNK]);
        self.len += 1;
        index
    }
    #[cfg(test)]
    fn push(&mut self, row: T, empty: fn() -> T) -> usize {
        let index = self.len;
        if index / CHUNK == self.chunks.len() {
            self.chunks.push(empty_chunk(empty));
        }
        self.chunks[index / CHUNK][index % CHUNK] = row;
        self.len += 1;
        index
    }
    fn get(&self, index: usize) -> &T {
        assert!(index < self.len, "active summary arena slot");
        &self.chunks[index / CHUNK][index % CHUNK]
    }
}
#[derive(Default)]
pub(super) struct Lanes {
    pairs: Arena<PairChange>,
    quads: Arena<QuadChange>,
    seeds: Option<Box<SeedLanes>>,
}
struct SeedLanes {
    pairs: Option<Arena<PairSeed>>,
    quads: Option<Arena<QuadSeed>>,
}
impl Lanes {
    pub(super) fn enable_seeds(&mut self, pairs: bool, quads: bool) {
        assert!(self.pairs.len == 0 && self.quads.len == 0);
        if pairs || quads {
            self.seeds = Some(Box::new(SeedLanes {
                pairs: pairs.then(Arena::default),
                quads: quads.then(Arena::default),
            }));
        }
    }
    #[inline]
    pub(super) fn seed_enabled<K: 'static>(&self) -> bool {
        let Some(seeds) = &self.seeds else {
            return false;
        };
        match kind::<K>() {
            Some(Kind::Pair) => seeds.pairs.is_some(),
            Some(Kind::Quad) => seeds.quads.is_some(),
            None => false,
        }
    }
    // Exact builtin keys make owned type erasure pure scalar copies. No
    // intermediate per-node Raw array or borrowed output carrier is required.
    #[inline(always)]
    pub(super) fn publish<K: Clone + 'static>(
        &mut self,
        change: TupleChange<K, IntSummary>,
        edges: &[Edge],
        pending: &mut VecDeque<(Edge, QueuedEvent)>,
    ) {
        if self.seed_enabled::<K>() {
            self.publish_seed(change.map(SummarySeed::from_summary), edges, pending);
            return;
        }
        let key = change.id.clone();
        match kind::<K>().expect("certified summary producer") {
            Kind::Pair => {
                let id = *(&change.id as &dyn Any).downcast_ref::<Pair>().unwrap();
                let value_key = *(&key as &dyn Any).downcast_ref::<Pair>().unwrap();
                let index = self.pairs.push_with(
                    || TupleChange::retract((0, 0)),
                    |slot| {
                        slot.id = id;
                        slot.value = change.value.map(|summary| (value_key, summary));
                    },
                );
                for &edge in edges {
                    pending.push_back((edge, QueuedEvent::PairSummary(index)));
                }
            }
            Kind::Quad => {
                let id = *(&change.id as &dyn Any).downcast_ref::<Quad>().unwrap();
                let value_key = *(&key as &dyn Any).downcast_ref::<Quad>().unwrap();
                let index = self.quads.push_with(
                    || TupleChange::retract((0, 0, 0, 0)),
                    |slot| {
                        slot.id = id;
                        slot.value = change.value.map(|summary| (value_key, summary));
                    },
                );
                for &edge in edges {
                    pending.push_back((edge, QueuedEvent::QuadSummary(index)));
                }
            }
        }
    }
    // Only single-receiver producers configured before graph execution use
    // these owned snapshots. Fanout retains one canonical full producer row.
    #[inline(always)]
    pub(super) fn publish_seed<K: Clone + 'static>(
        &mut self,
        change: TupleChange<K, SummarySeed>,
        edges: &[Edge],
        pending: &mut VecDeque<(Edge, QueuedEvent)>,
    ) {
        let key = change.id.clone();
        let lanes = self.seeds.as_mut().expect("configured seed lanes");
        match kind::<K>().expect("certified summary producer") {
            Kind::Pair => {
                let id = *(&change.id as &dyn Any).downcast_ref::<Pair>().unwrap();
                let key = *(&key as &dyn Any).downcast_ref::<Pair>().unwrap();
                let index = lanes.pairs.as_mut().unwrap().push_with(
                    || TupleChange::retract((0, 0)),
                    |slot| {
                        slot.id = id;
                        slot.value = change.value.map(|seed| (key, seed));
                    },
                );
                for &edge in edges {
                    pending.push_back((edge, QueuedEvent::PairSummary(index)));
                }
            }
            Kind::Quad => {
                let id = *(&change.id as &dyn Any).downcast_ref::<Quad>().unwrap();
                let key = *(&key as &dyn Any).downcast_ref::<Quad>().unwrap();
                let index = lanes.quads.as_mut().unwrap().push_with(
                    || TupleChange::retract((0, 0, 0, 0)),
                    |slot| {
                        slot.id = id;
                        slot.value = change.value.map(|seed| (key, seed));
                    },
                );
                for &edge in edges {
                    pending.push_back((edge, QueuedEvent::QuadSummary(index)));
                }
            }
        }
    }
    pub(super) fn clear(&mut self) {
        // Only exact primitive keys and library NoDrop summaries are stored.
        // Keep the valid full rows/chunks private and overwrite on later use.
        self.pairs.len = 0;
        self.quads.len = 0;
        if let Some(seeds) = &mut self.seeds {
            if let Some(rows) = &mut seeds.pairs {
                rows.len = 0;
            }
            if let Some(rows) = &mut seeds.quads {
                rows.len = 0;
            }
        }
    }
    pub(super) fn pair(&self, index: usize) -> SummaryRef<'_> {
        if let Some(rows) = self.seeds.as_ref().and_then(|s| s.pairs.as_ref()) {
            SummaryRef::PairSeed(rows.get(index))
        } else {
            SummaryRef::Pair(self.pairs.get(index))
        }
    }
    pub(super) fn quad(&self, index: usize) -> SummaryRef<'_> {
        if let Some(rows) = self.seeds.as_ref().and_then(|s| s.quads.as_ref()) {
            SummaryRef::QuadSeed(rows.get(index))
        } else {
            SummaryRef::Quad(self.quads.get(index))
        }
    }
    #[cfg(test)]
    fn put_pair(
        &mut self,
        row: PairChange,
        edges: &[Edge],
        pending: &mut VecDeque<(Edge, QueuedEvent)>,
    ) {
        let index = self.pairs.push(row, || TupleChange::retract((0, 0)));
        for &edge in edges {
            pending.push_back((edge, QueuedEvent::PairSummary(index)));
        }
    }
    #[cfg(test)]
    fn put_quad(
        &mut self,
        row: QuadChange,
        edges: &[Edge],
        pending: &mut VecDeque<(Edge, QueuedEvent)>,
    ) {
        let index = self.quads.push(row, || TupleChange::retract((0, 0, 0, 0)));
        for &edge in edges {
            pending.push_back((edge, QueuedEvent::QuadSummary(index)));
        }
    }
}

// Test-only retained reference implementation for differential publication.
#[cfg(test)]
pub(super) enum GroupOutput<'node> {
    Pair(&'node mut Raw<Pair>),
    Quad(&'node mut Raw<Quad>),
}
#[cfg(test)]
impl<'node> GroupOutput<'node> {
    pub(super) fn new<K: 'static>(raw: &'node mut Raw<K>) -> Self {
        match kind::<K>().expect("certified summary producer") {
            Kind::Pair => Self::Pair((raw as &mut dyn Any).downcast_mut().unwrap()),
            Kind::Quad => Self::Quad((raw as &mut dyn Any).downcast_mut().unwrap()),
        }
    }
    pub(super) fn kind(&self) -> Kind {
        match self {
            Self::Pair(_) => Kind::Pair,
            Self::Quad(_) => Kind::Quad,
        }
    }
    pub(super) fn publish(
        mut self,
        edges: &[Edge],
        pending: &mut VecDeque<(Edge, QueuedEvent)>,
        lanes: &mut Lanes,
    ) {
        fn enrich<K: Clone>(change: TupleChange<K, IntSummary>) -> TupleChange<K, (K, IntSummary)> {
            let key = change.id.clone();
            change.map(|summary| (key, summary))
        }
        match &mut self {
            Self::Pair(raw) => {
                let first = raw[0].take().map(enrich);
                let second = raw[1].take().map(enrich);
                if let Some(row) = first {
                    lanes.put_pair(row, edges, pending);
                }
                if let Some(row) = second {
                    lanes.put_pair(row, edges, pending);
                }
            }
            Self::Quad(raw) => {
                let first = raw[0].take().map(enrich);
                let second = raw[1].take().map(enrich);
                if let Some(row) = first {
                    lanes.put_quad(row, edges, pending);
                }
                if let Some(row) = second {
                    lanes.put_quad(row, edges, pending);
                }
            }
        }
    }
}
#[cfg(test)]
impl Drop for GroupOutput<'_> {
    fn drop(&mut self) {
        match self {
            Self::Pair(raw) => {
                raw[0] = None;
                raw[1] = None;
            }
            Self::Quad(raw) => {
                raw[0] = None;
                raw[1] = None;
            }
        }
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod seed_tests;
