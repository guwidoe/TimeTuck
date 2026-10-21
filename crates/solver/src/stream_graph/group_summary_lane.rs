//! Full exact builtin summaries in stable private chunks, without per-row Rc.
//! This is neither compressed Event decoding nor an event-owner recycling pool.
use super::event_queue::QueuedEvent;
use super::*;
use std::any::TypeId;

type Pair = (usize, usize);
type Quad = (usize, usize, usize, usize);
type PairChange = TupleChange<Pair, (Pair, IntSummary)>;
type QuadChange = TupleChange<Quad, (Quad, IntSummary)>;
#[cfg(test)]
type Raw<K> = [Option<TupleChange<K, IntSummary>>; 2];
const CHUNK: usize = 64;

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
}
impl SummaryRef<'_> {
    pub(super) fn typed<I: 'static, T: 'static>(&self) -> &TupleChange<I, T> {
        // Concrete per-arm vtables retain the same foldable type checks as Event.
        match self {
            Self::Pair(row) => (*row as &dyn Any).downcast_ref(),
            Self::Quad(row) => (*row as &dyn Any).downcast_ref(),
        }
        .expect("certified summary receiver")
    }
    pub(super) fn is_put(&self) -> bool {
        match self {
            Self::Pair(row) => row.value.is_some(),
            Self::Quad(row) => row.value.is_some(),
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
    fn push(&mut self, row: T, empty: fn() -> T) -> usize {
        let index = self.len;
        if index / CHUNK == self.chunks.len() {
            self.chunks.push(Box::new(std::array::from_fn(|_| empty())));
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
}
impl Lanes {
    // Exact builtin keys make owned type erasure pure scalar copies. No
    // intermediate per-node Raw array or borrowed output carrier is required.
    #[inline(always)]
    pub(super) fn publish<K: Clone + 'static>(
        &mut self,
        change: TupleChange<K, IntSummary>,
        edges: &[Edge],
        pending: &mut VecDeque<(Edge, QueuedEvent)>,
    ) {
        let key = change.id.clone();
        let enriched = TupleChange {
            id: change.id,
            value: change.value.map(|summary| (key, summary)),
        };
        match kind::<K>().expect("certified summary producer") {
            Kind::Pair => {
                let row = (&enriched as &dyn Any)
                    .downcast_ref::<PairChange>()
                    .unwrap()
                    .clone();
                self.put_pair(row, edges, pending);
            }
            Kind::Quad => {
                let row = (&enriched as &dyn Any)
                    .downcast_ref::<QuadChange>()
                    .unwrap()
                    .clone();
                self.put_quad(row, edges, pending);
            }
        }
    }
    pub(super) fn clear(&mut self) {
        // Only exact primitive keys and library NoDrop summaries are stored.
        // Keep the valid full rows/chunks private and overwrite on later use.
        self.pairs.len = 0;
        self.quads.len = 0;
    }
    pub(super) fn pair(&self, index: usize) -> SummaryRef<'_> {
        SummaryRef::Pair(self.pairs.get(index))
    }
    pub(super) fn quad(&self, index: usize) -> SummaryRef<'_> {
        SummaryRef::Quad(self.quads.get(index))
    }
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
