//! Exact builtin graph input values. Unlike group summaries, these captured
//! occurrences have no public owner view. Snapshots copy values; they need not
//! preserve distinct private historical allocations. Generic/public nodes never
//! select this backend, and all callback/emission work remains in GroupNode.
use super::*;
const PAGE_SIZE: usize = 64;
type Page<V> = [Option<V>; PAGE_SIZE];
type SharedPage<V> = [Option<Arc<V>>; PAGE_SIZE];
type PairIndex<I> = fn(&I, usize, usize) -> Option<usize>;
type Undo<V> = Vec<(usize, Option<V>)>;
pub(in crate::incremental_stream) type Copier<V> = fn(&V) -> V;

pub(in crate::incremental_stream) fn group_copier<K: Clone + 'static>() -> Option<Copier<(K, i64)>>
{
    let id = TypeId::of::<K>();
    let exact = id == TypeId::of::<usize>()
        || id == TypeId::of::<(usize, usize)>()
        || id == TypeId::of::<(usize, usize, usize)>()
        || id == TypeId::of::<((usize, usize), usize)>()
        || id == TypeId::of::<(usize, (usize, usize))>()
        || id == TypeId::of::<((usize, usize), (usize, usize))>()
        || id == TypeId::of::<(usize, usize, usize, usize)>();
    // The exact TypeId gate, rather than needs_drop, certifies that this Clone
    // can only be the builtin value copy. No custom Clone implementation runs.
    exact.then_some(Clone::clone)
}

enum Universe<I> {
    Fixed {
        count: usize,
        index: fn(&I) -> usize,
    },
    Pair {
        left: usize,
        right: usize,
        index: PairIndex<I>,
    },
}
impl<I> Clone for Universe<I> {
    fn clone(&self) -> Self {
        match *self {
            Self::Fixed { count, index } => Self::Fixed { count, index },
            Self::Pair { left, right, index } => Self::Pair { left, right, index },
        }
    }
}
enum Slots<V> {
    Dense(Vec<Option<V>>),
    Paged(Vec<Option<Box<Page<V>>>>),
}
pub(in crate::incremental_stream) struct Rows<I, V> {
    universe: Universe<I>,
    slots: Slots<V>,
    copy: Copier<V>,
    frames: Vec<Undo<V>>,
    spare: Vec<Undo<V>>,
}
pub(in crate::incremental_stream) struct Capture<V> {
    dimensions: (usize, Option<usize>),
    rows: Undo<V>,
}
impl<I, V> Clone for Rows<I, V> {
    fn clone(&self) -> Self {
        let copy = self.copy;
        let slots = match &self.slots {
            Slots::Dense(values) => {
                Slots::Dense(values.iter().map(|v| v.as_ref().map(copy)).collect())
            }
            Slots::Paged(pages) => Slots::Paged(
                pages
                    .iter()
                    .map(|page| {
                        page.as_ref().map(|page| {
                            Box::new(std::array::from_fn(|i| page[i].as_ref().map(copy)))
                        })
                    })
                    .collect(),
            ),
        };
        Self {
            universe: self.universe.clone(),
            slots,
            copy,
            frames: Vec::new(),
            spare: Vec::new(),
        }
    }
}
impl<I, V> Rows<I, V> {
    pub(super) fn from_dense(
        values: Vec<Option<Arc<V>>>,
        index: fn(&I) -> usize,
        copy: Copier<V>,
    ) -> Self {
        Self {
            universe: Universe::Fixed {
                count: values.len(),
                index,
            },
            slots: Slots::Dense(values.into_iter().map(|v| v.as_deref().map(copy)).collect()),
            copy,
            frames: Vec::new(),
            spare: Vec::new(),
        }
    }
    pub(super) fn from_pair(
        left: usize,
        right: usize,
        index: PairIndex<I>,
        pages: Vec<Option<Box<SharedPage<V>>>>,
        copy: Copier<V>,
    ) -> Self {
        Self {
            universe: Universe::Pair { left, right, index },
            slots: Slots::Paged(
                pages
                    .into_iter()
                    .map(|page| {
                        page.map(|page| {
                            Box::new(std::array::from_fn(|i| page[i].as_deref().map(copy)))
                        })
                    })
                    .collect(),
            ),
            copy,
            frames: Vec::new(),
            spare: Vec::new(),
        }
    }
    fn dimensions(&self) -> (usize, Option<usize>) {
        match self.universe {
            Universe::Fixed { count, .. } => (count, None),
            Universe::Pair { left, right, .. } => (left, Some(right)),
        }
    }
    fn count(&self) -> usize {
        match self.universe {
            Universe::Fixed { count, .. } => count,
            Universe::Pair { left, right, .. } => left * right,
        }
    }
    pub(super) fn index(&self, id: &I) -> Option<usize> {
        match self.universe {
            Universe::Fixed { count, index } => {
                let i = index(id);
                (i < count).then_some(i)
            }
            Universe::Pair { left, right, index } => index(id, left, right),
        }
    }
    pub(super) fn validate(&self, index: Option<usize>) -> usize {
        match self.universe {
            Universe::Fixed { .. } => index.expect("bound group input outside universe"),
            Universe::Pair { .. } => index.expect("bound group pair outside universe"),
        }
    }
    pub(super) fn get(&self, index: usize) -> Option<&V> {
        match &self.slots {
            Slots::Dense(values) => values.get(index).and_then(Option::as_ref),
            Slots::Paged(pages) => pages
                .get(index / PAGE_SIZE)
                .and_then(Option::as_ref)
                .and_then(|page| page[index % PAGE_SIZE].as_ref()),
        }
    }
    fn set(&mut self, index: usize, value: Option<V>) -> Option<V> {
        match &mut self.slots {
            Slots::Dense(values) => std::mem::replace(&mut values[index], value),
            Slots::Paged(pages) => {
                let page = &mut pages[index / PAGE_SIZE];
                if page.is_none() && value.is_none() {
                    return None;
                }
                let page = page.get_or_insert_with(|| Box::new(std::array::from_fn(|_| None)));
                std::mem::replace(&mut page[index % PAGE_SIZE], value)
            }
        }
    }
    pub(super) fn replace(&mut self, index: usize, value: Option<V>) -> bool {
        let old = self.set(index, value);
        let present = old.is_some();
        if let Some(frame) = self.frames.last_mut() {
            frame.push((index, old));
        }
        present
    }
    pub(super) fn clear(&mut self) {
        for index in 0..self.count() {
            if self.get(index).is_some() {
                self.replace(index, None);
            }
        }
    }
    pub(super) fn begin(&mut self) -> usize {
        let token = self.frames.len();
        self.frames.push(self.spare.pop().unwrap_or_default());
        token
    }
    pub(super) fn commit(&mut self, token: usize) {
        assert_eq!(
            self.frames.len(),
            token + 1,
            "owned group input journal order"
        );
        let mut undo = self.frames.pop().unwrap();
        if let Some(parent) = self.frames.last_mut() {
            if parent.is_empty() {
                std::mem::swap(parent, &mut undo);
            } else {
                parent.append(&mut undo);
            }
        }
        undo.clear();
        self.spare.push(undo);
    }
    pub(super) fn rollback(&mut self, token: usize) {
        assert_eq!(
            self.frames.len(),
            token + 1,
            "owned group input journal order"
        );
        let mut undo = self.frames.pop().unwrap();
        for (index, value) in undo.drain(..).rev() {
            self.set(index, value);
        }
        self.spare.push(undo);
    }
    pub(super) fn capture_current(&self, token: usize) -> Capture<V> {
        assert!(
            token < self.frames.len(),
            "active owned group input journal token"
        );
        let mut indices: Vec<_> = self.frames[token..]
            .iter()
            .flat_map(|f| f.iter().map(|(i, _)| *i))
            .collect();
        indices.sort_unstable();
        indices.dedup();
        Capture {
            dimensions: self.dimensions(),
            rows: indices
                .into_iter()
                .map(|i| (i, self.get(i).map(self.copy)))
                .collect(),
        }
    }
    pub(super) fn install_capture(&mut self, captured: Capture<V>) {
        assert_eq!(
            self.dimensions(),
            captured.dimensions,
            "group input capture universe"
        );
        assert!(
            captured.rows.iter().all(|(i, _)| *i < self.count()),
            "group input capture index"
        );
        for (i, v) in captured.rows {
            self.replace(i, v);
        }
    }
    pub(super) fn install(&mut self, saved: Self) {
        assert_eq!(
            self.dimensions(),
            saved.dimensions(),
            "group input universe"
        );
        match saved.slots {
            Slots::Dense(values) => {
                for (i, v) in values.into_iter().enumerate() {
                    self.replace(i, v);
                }
            }
            Slots::Paged(pages) => {
                let count = self.count();
                for (p, page) in pages.into_iter().enumerate() {
                    // Owned full snapshots have copied private values. Neither
                    // equal rows nor None padding need historical Arc identity.
                    if let Some(page) = page {
                        for (c, v) in (*page)
                            .into_iter()
                            .enumerate()
                            .take((count - p * PAGE_SIZE).min(PAGE_SIZE))
                        {
                            self.replace(p * PAGE_SIZE + c, v);
                        }
                    } else {
                        for c in 0..(count - p * PAGE_SIZE).min(PAGE_SIZE) {
                            let i = p * PAGE_SIZE + c;
                            if self.get(i).is_some() {
                                self.replace(i, None);
                            }
                        }
                    }
                }
            }
        }
    }
}
