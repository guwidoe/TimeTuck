//! Complete, bounded usize input universes use compact membership buckets.
//! Keys retain their original Arc representatives; bit order is input ID order.
//! Only changed buckets are copied, and ordinary key-map journals undo updates.
use super::{JoinComparison, SnapshotMap};
use std::{
    any::{Any, TypeId},
    ops::Bound,
    sync::Arc,
};
type SharedBuckets<I, K> = Vec<(Arc<K>, Arc<SnapshotMap<I, ()>>)>;

#[derive(Clone)]
struct Members {
    words: Vec<u64>,
    len: usize,
}
impl Members {
    fn new(count: usize) -> Self {
        Self {
            words: vec![0; count.div_ceil(64)],
            len: 0,
        }
    }
    fn insert(&mut self, id: usize) {
        let bit = 1u64 << (id % 64);
        let word = &mut self.words[id / 64];
        if *word & bit == 0 {
            *word |= bit;
            self.len += 1;
        }
    }
    fn remove(&mut self, id: usize) {
        let bit = 1u64 << (id % 64);
        let word = &mut self.words[id / 64];
        if *word & bit != 0 {
            *word &= !bit;
            self.len -= 1;
        }
    }
    fn ids(&self) -> impl Iterator<Item = usize> + '_ {
        self.words
            .iter()
            .enumerate()
            .flat_map(|(word_index, word)| {
                let mut bits = *word;
                std::iter::from_fn(move || {
                    if bits == 0 {
                        None
                    } else {
                        let bit = bits.trailing_zeros() as usize;
                        bits &= bits - 1;
                        Some(word_index * 64 + bit)
                    }
                })
            })
    }
}

type KeyCode = [usize; 4];
type KeyMapper<K> = fn(&K) -> KeyCode;
type CapturedEntries<K> = Vec<(KeyCode, Option<(Arc<K>, Arc<Members>)>)>;
enum SavedBuckets<K> {
    Full(SnapshotMap<K, Members>),
    Delta(CapturedEntries<K>),
}
pub(super) struct Capture<K> {
    count: usize,
    buckets: SavedBuckets<K>,
}

pub(super) struct FixedIndex<I, K> {
    keys: Vec<Arc<I>>,
    #[cfg(test)]
    equality_visit_disabled: bool,
    buckets: SnapshotMap<K, Members>,
    index: fn(&I) -> usize,
    capture_key: Option<KeyMapper<K>>,
    frames: Vec<Vec<KeyCode>>,
    spare: Vec<Vec<KeyCode>>,
}
impl<I, K> Clone for FixedIndex<I, K> {
    fn clone(&self) -> Self {
        Self {
            keys: self.keys.clone(),
            #[cfg(test)]
            equality_visit_disabled: self.equality_visit_disabled,
            buckets: self.buckets.clone(),
            index: self.index,
            capture_key: self.capture_key,
            frames: Vec::new(),
            spare: Vec::new(),
        }
    }
}
impl<I, K> FixedIndex<I, K> {
    pub(super) fn new(
        keys: Vec<Arc<I>>,
        buckets: SharedBuckets<I, K>,
        index: fn(&I) -> usize,
    ) -> Self {
        let buckets = buckets
            .into_iter()
            .map(|(key, ids)| {
                let mut members = Members::new(keys.len());
                for id in ids.keys() {
                    members.insert(index(id));
                }
                (key, Arc::new(members))
            })
            .collect();
        Self {
            keys,
            #[cfg(test)]
            equality_visit_disabled: false,
            buckets: SnapshotMap::from_shared_sorted(buckets),
            index,
            capture_key: None,
            frames: Vec::new(),
            spare: Vec::new(),
        }
    }
    fn record_key(&mut self, key: &K) {
        if let (Some(map), Some(frame)) = (self.capture_key, self.frames.last_mut()) {
            frame.push(map(key));
        }
    }
    pub(super) fn clear(&mut self) {
        if let (Some(map), Some(frame)) = (self.capture_key, self.frames.last_mut()) {
            frame.extend(self.buckets.keys().map(map));
        }
        self.buckets.clear();
    }
    pub(super) fn begin(&mut self) -> usize {
        let token = self.buckets.begin_journal();
        assert_eq!(
            token,
            self.frames.len(),
            "fixed index capture journal order"
        );
        self.frames.push(self.spare.pop().unwrap_or_default());
        token
    }
    pub(super) fn commit(&mut self, token: usize) {
        self.buckets.commit_journal(token);
        let mut frame = self.frames.pop().expect("fixed index capture frame");
        if let Some(parent) = self.frames.last_mut() {
            if parent.is_empty() {
                std::mem::swap(parent, &mut frame);
            } else {
                parent.append(&mut frame);
            }
        }
        frame.clear();
        self.spare.push(frame);
    }
    pub(super) fn rollback(&mut self, token: usize) {
        self.buckets.rollback_journal(token);
        let mut frame = self.frames.pop().expect("fixed index capture frame");
        frame.clear();
        self.spare.push(frame);
    }
    pub(super) fn install(&mut self, saved: Self) {
        if let (Some(map), Some(frame)) = (self.capture_key, self.frames.last_mut()) {
            frame.extend(self.buckets.keys().chain(saved.buckets.keys()).map(map));
        }
        self.buckets.install_snapshot(saved.buckets);
    }
    pub(super) fn capture_current(&self, token: usize) -> Capture<K> {
        assert!(
            token < self.frames.len(),
            "active fixed index capture token"
        );
        let buckets = if let Some(map) = self.capture_key {
            let mut codes: Vec<_> = self.frames[token..].iter().flatten().copied().collect();
            codes.sort_unstable();
            codes.dedup();
            let mut entries: CapturedEntries<K> =
                codes.into_iter().map(|key| (key, None)).collect();
            // The key index is small; scan its shared entries once, retaining
            // actual owners only for changed buckets. No input-ID universe is copied.
            for (key, value) in self.buckets.shared_entries() {
                if let Ok(slot) = entries.binary_search_by_key(&map(&key), |(code, _)| *code) {
                    entries[slot].1 = Some((key, value));
                }
            }
            SavedBuckets::Delta(entries)
        } else {
            SavedBuckets::Full(self.buckets.clone())
        };
        Capture {
            count: self.keys.len(),
            buckets,
        }
    }
    pub(super) fn install_capture(&mut self, saved: Capture<K>) {
        assert_eq!(self.keys.len(), saved.count, "fixed index capture universe");
        let entries = match saved.buckets {
            SavedBuckets::Full(rows) => {
                // Nonprimitive keys retain the original full map owner policy.
                self.buckets.install_snapshot(rows);
                return;
            }
            SavedBuckets::Delta(entries) => entries,
        };
        let map = self.capture_key.expect("primitive fixed index capture");
        if let Some(frame) = self.frames.last_mut() {
            frame.extend(entries.iter().map(|(code, _)| *code));
        }
        let old = self.buckets.shared_entries();
        let mut next = Vec::with_capacity(old.len() + entries.len());
        let mut old = old.into_iter().peekable();
        for (code, entry) in entries {
            while old.peek().is_some_and(|(key, _)| map(key) < code) {
                next.push(old.next().unwrap());
            }
            if old.peek().is_some_and(|(key, _)| map(key) == code) {
                old.next();
            }
            if let Some(entry) = entry {
                next.push(entry);
            }
        }
        next.extend(old);
        // Rebuild only the small bucket map on selected-candidate installation.
        // Its existing journal preserves outer undo and exact key representatives.
        self.buckets
            .install_snapshot(SnapshotMap::from_shared_sorted(next));
    }
}
impl<I, K: Ord + Clone> FixedIndex<I, K> {
    // The existing numeric capture mapper certifies exact primitive K. Complete
    // FixedIndex bindings already certify usize occurrence IDs. Other routes
    // retain their original iterator, including every custom-key comparison.
    pub(super) fn supports_equality_visit(&self) -> bool {
        #[cfg(test)]
        if self.equality_visit_disabled {
            return false;
        }
        self.capture_key.is_some()
    }
    #[cfg(test)]
    pub(super) fn disable_equality_visit_for_tests(&mut self) {
        self.equality_visit_disabled = true;
    }
    // Visit in word/bit order using the SAME canonical input-key references as
    // matching(). Reserve the full bucket before any output cloning/callback.
    // This avoids returning the much larger equality/range composite iterator.
    pub(super) fn for_each_equal<O>(
        &self,
        key: &K,
        output: &mut Vec<O>,
        mut emit: impl FnMut(&mut Vec<O>, &I),
    ) {
        let members = self.buckets.get(key);
        output.reserve(members.map_or(0, |members| members.len));
        if let Some(members) = members {
            for (word_index, word) in members.words.iter().enumerate() {
                let mut bits = *word;
                while bits != 0 {
                    let bit = bits.trailing_zeros() as usize;
                    bits &= bits - 1;
                    emit(output, self.keys[word_index * 64 + bit].as_ref());
                }
            }
        }
    }

    pub(super) fn remove(&mut self, key: &K, id: &I) {
        let mut members = self.buckets.get(key).expect("indexed key").clone();
        members.remove((self.index)(id));
        if members.len == 0 {
            self.buckets.remove(key);
        } else {
            self.buckets.insert(key.clone(), members);
        }
        self.record_key(key);
    }
    pub(super) fn insert(&mut self, key: &K, id: &I) {
        let mut members = self
            .buckets
            .get(key)
            .cloned()
            .unwrap_or_else(|| Members::new(self.keys.len()));
        members.insert((self.index)(id));
        self.buckets.insert(key.clone(), members);
        self.record_key(key);
    }
    pub(super) fn matching<'a>(
        &'a self,
        key: &'a K,
        comparison: JoinComparison,
    ) -> (usize, impl Iterator<Item = &'a I>) {
        use Bound::{Excluded, Included, Unbounded};
        let (exact, range) = match comparison {
            JoinComparison::Equal => (self.buckets.get(key), None),
            JoinComparison::LessThan => (None, Some(self.buckets.range(Excluded(key), Unbounded))),
            JoinComparison::LessThanOrEqual => {
                (None, Some(self.buckets.range(Included(key), Unbounded)))
            }
            JoinComparison::GreaterThan => {
                (None, Some(self.buckets.range(Unbounded, Excluded(key))))
            }
            JoinComparison::GreaterThanOrEqual => {
                (None, Some(self.buckets.range(Unbounded, Included(key))))
            }
        };
        (
            exact.map_or(0, |members| members.len),
            exact
                .into_iter()
                .chain(range.into_iter().flatten().map(|(_, members)| members))
                .flat_map(|members| members.ids())
                .map(|id| self.keys[id].as_ref()),
        )
    }
}

impl<I, K: 'static> FixedIndex<I, K> {
    pub(super) fn enable_delta_capture(&mut self) {
        fn cast<K: 'static, T: 'static>(key: &K) -> &T {
            (key as &dyn Any)
                .downcast_ref()
                .expect("checked primitive join key")
        }
        let kind = TypeId::of::<K>();
        self.capture_key = if kind == TypeId::of::<usize>() {
            Some(|key| [0, 0, 0, *cast::<K, usize>(key)])
        } else if kind == TypeId::of::<(usize, usize)>() {
            Some(|key| {
                let &(a, b) = cast::<K, (usize, usize)>(key);
                [0, 0, a, b]
            })
        } else if kind == TypeId::of::<(usize, usize, usize)>() {
            Some(|key| {
                let &(a, b, c) = cast::<K, (usize, usize, usize)>(key);
                [0, a, b, c]
            })
        } else if kind == TypeId::of::<((usize, usize), usize)>() {
            Some(|key| {
                let &((a, b), c) = cast::<K, ((usize, usize), usize)>(key);
                [0, a, b, c]
            })
        } else if kind == TypeId::of::<(usize, (usize, usize))>() {
            Some(|key| {
                let &(a, (b, c)) = cast::<K, (usize, (usize, usize))>(key);
                [0, a, b, c]
            })
        } else if kind == TypeId::of::<((usize, usize), (usize, usize))>() {
            Some(|key| {
                let &((a, b), (c, d)) = cast::<K, ((usize, usize), (usize, usize))>(key);
                [a, b, c, d]
            })
        } else {
            None
        };
    }
}

#[cfg(test)]
mod join_index_capture_tests {
    use super::*;
    fn buckets<K>(keys: impl IntoIterator<Item = K>) -> SharedBuckets<usize, K> {
        keys.into_iter()
            .enumerate()
            .map(|(id, key)| {
                (
                    Arc::new(key),
                    Arc::new(SnapshotMap::from_sorted([(id, ())].into_iter())),
                )
            })
            .collect()
    }
    #[test]
    fn join_index_capture_keeps_changed_buckets_and_exact_keys_without_input_universe_owners() {
        let keys: Vec<_> = (0usize..4).map(Arc::new).collect();
        let mut index = FixedIndex::new(keys.clone(), buckets([0usize, 1, 2, 3]), |id| *id);
        index.enable_delta_capture();
        let original_key = index.buckets.shared_entries()[2].0.clone();
        let original = index.begin();
        index.remove(&0, &0);
        index.insert(&4, &0);
        index.remove(&2, &2);
        index.insert(&2, &2);
        let inner = index.begin();
        index.remove(&1, &1);
        index.commit(inner);
        let new_key = index
            .buckets
            .shared_entries()
            .into_iter()
            .find(|(key, _)| **key == 2)
            .unwrap()
            .0;
        assert!(!Arc::ptr_eq(&new_key, &original_key));
        let saved = index.capture_current(original);
        let nested = index.capture_current(original);
        let SavedBuckets::Delta(entries) = &saved.buckets else {
            panic!("primitive bucket delta");
        };
        assert_eq!(entries.len(), 4);
        assert!(
            entries.iter().all(|(code, _)| code[3] != 3),
            "untouched bucket omitted"
        );
        assert!(
            keys.iter().all(|key| Arc::strong_count(key) == 2),
            "capture never clones input-key universe"
        );
        index.rollback(original);
        assert_eq!(
            index
                .matching(&0, JoinComparison::Equal)
                .1
                .copied()
                .collect::<Vec<_>>(),
            vec![0]
        );
        let before = index
            .buckets
            .shared_entries()
            .into_iter()
            .find(|(key, _)| **key == 2)
            .unwrap()
            .0;
        assert!(Arc::ptr_eq(&before, &original_key));
        let outer = index.begin();
        index.install_capture(nested);
        assert!(index.matching(&0, JoinComparison::Equal).1.next().is_none());
        assert_eq!(
            index
                .matching(&4, JoinComparison::Equal)
                .1
                .copied()
                .collect::<Vec<_>>(),
            vec![0]
        );
        let installed = index
            .buckets
            .shared_entries()
            .into_iter()
            .find(|(key, _)| **key == 2)
            .unwrap()
            .0;
        assert!(Arc::ptr_eq(&installed, &new_key));
        index.rollback(outer);
        let restored = index
            .buckets
            .shared_entries()
            .into_iter()
            .find(|(key, _)| **key == 2)
            .unwrap()
            .0;
        assert!(Arc::ptr_eq(&restored, &original_key));
        index.install_capture(saved);
        assert!(index.matching(&1, JoinComparison::Equal).1.next().is_none());
        assert_eq!(
            index
                .matching(&4, JoinComparison::Equal)
                .1
                .copied()
                .collect::<Vec<_>>(),
            vec![0]
        );
    }
    #[test]
    fn join_index_capture_custom_no_drop_keys_do_not_replay_user_clone_or_comparison() {
        use std::{cell::Cell, cmp::Ordering};
        thread_local! { static CLONES: Cell<usize> = const { Cell::new(0) }; static COMPARES: Cell<usize> = const { Cell::new(0) }; }
        struct Key(usize);
        impl Clone for Key {
            fn clone(&self) -> Self {
                CLONES.with(|n| n.set(n.get() + 1));
                Self(self.0)
            }
        }
        impl PartialEq for Key {
            fn eq(&self, other: &Self) -> bool {
                self.0 == other.0
            }
        }
        impl Eq for Key {}
        impl PartialOrd for Key {
            fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
                Some(self.cmp(other))
            }
        }
        impl Ord for Key {
            fn cmp(&self, other: &Self) -> Ordering {
                COMPARES.with(|n| n.set(n.get() + 1));
                self.0.cmp(&other.0)
            }
        }
        let mut index = FixedIndex::new(
            vec![Arc::new(0usize), Arc::new(1)],
            buckets([Key(0), Key(1)]),
            |id| *id,
        );
        index.enable_delta_capture();
        assert!(!std::mem::needs_drop::<Key>());
        assert!(index.capture_key.is_none());
        let original = index.begin();
        index.remove(&Key(0), &0);
        index.insert(&Key(2), &0);
        let clones = CLONES.with(Cell::get);
        let comparisons = COMPARES.with(Cell::get);
        let saved = index.capture_current(original);
        assert!(matches!(&saved.buckets, SavedBuckets::Full(_)));
        index.rollback(original);
        let outer = index.begin();
        index.install_capture(saved);
        index.rollback(outer);
        assert_eq!(CLONES.with(Cell::get), clones);
        assert_eq!(COMPARES.with(Cell::get), comparisons);
    }
}
