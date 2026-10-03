//! Immutable inspection of ordered stream state with inexpensive snapshots.
//!
//! Cloning shares an AVL tree root; internal updates copy only the search and
//! rebalance paths. Keys and values on unchanged paths are shared, not cloned.
//! Snapshots retain Send/Sync when their keys and values do. Identity and contents
//! must remain immutable. Stream graph scheduling remains single-threaded.
//!
//! ```
//! use timetuck::{SimpleScore, StreamGraphBuilder, TupleChange};
//! let mut builder = StreamGraphBuilder::<SimpleScore>::new();
//! let source = builder.source::<usize, String>();
//! let mut graph = builder.build();
//! let mut transaction = graph.transaction();
//! transaction.apply(&source, TupleChange::put(0, "before".into()))?;
//! transaction.commit()?;
//! let snapshot = graph.source_rows(&source)?.clone();
//! let mut transaction = graph.transaction();
//! transaction.apply(&source, TupleChange::put(0, "after".into()))?;
//! transaction.commit()?;
//! assert_eq!(snapshot[&0], "before");
//! assert_eq!(graph.source_rows(&source)?[&0], "after");
//! # Ok::<(), timetuck::GraphError>(())
//! ```
use std::{
    cmp::Ordering,
    collections::BTreeMap,
    fmt,
    ops::{Bound, Index},
    sync::Arc,
};

type Link<K, V> = Option<Arc<Node<K, V>>>;
struct Node<K, V> {
    key: Arc<K>,
    value: Arc<V>,
    left: Link<K, V>,
    right: Link<K, V>,
    height: usize,
    size: usize,
}
/// A read-only ordered map of stream facts or matches. A clone is a stable,
/// inexpensive snapshot. Mutation is reserved for the owning stream operators.
pub struct SnapshotMap<K, V> {
    root: Link<K, V>,
}
impl<K, V> Clone for SnapshotMap<K, V> {
    fn clone(&self) -> Self {
        Self {
            root: self.root.clone(),
        }
    }
}
impl<K, V> Default for SnapshotMap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}
fn height<K, V>(n: &Link<K, V>) -> usize {
    n.as_ref().map_or(0, |n| n.height)
}
fn size<K, V>(n: &Link<K, V>) -> usize {
    n.as_ref().map_or(0, |n| n.size)
}
fn node<K, V>(key: Arc<K>, value: Arc<V>, left: Link<K, V>, right: Link<K, V>) -> Arc<Node<K, V>> {
    Arc::new(Node {
        key,
        value,
        height: 1 + height(&left).max(height(&right)),
        size: 1 + size(&left) + size(&right),
        left,
        right,
    })
}
fn balance<K, V>(
    key: Arc<K>,
    value: Arc<V>,
    left: Link<K, V>,
    right: Link<K, V>,
) -> Arc<Node<K, V>> {
    if height(&left) > height(&right) + 1 {
        let l = left.as_ref().expect("left-heavy node");
        if height(&l.left) >= height(&l.right) {
            let r = node(key, value, l.right.clone(), right);
            node(l.key.clone(), l.value.clone(), l.left.clone(), Some(r))
        } else {
            let pivot = l.right.as_ref().expect("inner left branch");
            let a = node(
                l.key.clone(),
                l.value.clone(),
                l.left.clone(),
                pivot.left.clone(),
            );
            let b = node(key, value, pivot.right.clone(), right);
            node(pivot.key.clone(), pivot.value.clone(), Some(a), Some(b))
        }
    } else if height(&right) > height(&left) + 1 {
        let r = right.as_ref().expect("right-heavy node");
        if height(&r.right) >= height(&r.left) {
            let l = node(key, value, left, r.left.clone());
            node(r.key.clone(), r.value.clone(), Some(l), r.right.clone())
        } else {
            let pivot = r.left.as_ref().expect("inner right branch");
            let a = node(key, value, left, pivot.left.clone());
            let b = node(
                r.key.clone(),
                r.value.clone(),
                pivot.right.clone(),
                r.right.clone(),
            );
            node(pivot.key.clone(), pivot.value.clone(), Some(a), Some(b))
        }
    } else {
        node(key, value, left, right)
    }
}
fn put<K: Ord, V>(root: &Link<K, V>, key: Arc<K>, value: Arc<V>) -> Arc<Node<K, V>> {
    let Some(n) = root else {
        return node(key, value, None, None);
    };
    match key.cmp(&n.key) {
        Ordering::Less => balance(
            n.key.clone(),
            n.value.clone(),
            Some(put(&n.left, key, value)),
            n.right.clone(),
        ),
        Ordering::Greater => balance(
            n.key.clone(),
            n.value.clone(),
            n.left.clone(),
            Some(put(&n.right, key, value)),
        ),
        Ordering::Equal => node(n.key.clone(), value, n.left.clone(), n.right.clone()),
    }
}
fn remove_first<K, V>(n: &Arc<Node<K, V>>) -> Link<K, V> {
    if let Some(left) = &n.left {
        Some(balance(
            n.key.clone(),
            n.value.clone(),
            remove_first(left),
            n.right.clone(),
        ))
    } else {
        n.right.clone()
    }
}
fn erase<K: Ord, V>(root: &Link<K, V>, key: &K) -> (Link<K, V>, bool) {
    let Some(n) = root else { return (None, false) };
    match key.cmp(&n.key) {
        Ordering::Less => {
            let (left, found) = erase(&n.left, key);
            if !found {
                return (root.clone(), false);
            }
            (
                Some(balance(
                    n.key.clone(),
                    n.value.clone(),
                    left,
                    n.right.clone(),
                )),
                true,
            )
        }
        Ordering::Greater => {
            let (right, found) = erase(&n.right, key);
            if !found {
                return (root.clone(), false);
            }
            (
                Some(balance(
                    n.key.clone(),
                    n.value.clone(),
                    n.left.clone(),
                    right,
                )),
                true,
            )
        }
        Ordering::Equal => match (&n.left, &n.right) {
            (None, _) => (n.right.clone(), true),
            (_, None) => (n.left.clone(), true),
            (Some(_), Some(right)) => {
                let mut first = right;
                while let Some(left) = &first.left {
                    first = left;
                }
                (
                    Some(balance(
                        first.key.clone(),
                        first.value.clone(),
                        n.left.clone(),
                        remove_first(right),
                    )),
                    true,
                )
            }
        },
    }
}
impl<K, V> SnapshotMap<K, V> {
    pub(crate) fn new() -> Self {
        Self { root: None }
    }
    pub(crate) fn clear(&mut self) {
        self.root = None;
    }
    pub fn len(&self) -> usize {
        size(&self.root)
    }
    pub(crate) fn entry_at(&self, mut index: usize) -> Option<(&K, &V)> {
        let mut current = self.root.as_ref();
        while let Some(node) = current {
            let left_size = size(&node.left);
            match index.cmp(&left_size) {
                Ordering::Less => current = node.left.as_ref(),
                Ordering::Equal => return Some((&node.key, &node.value)),
                Ordering::Greater => {
                    index -= left_size + 1;
                    current = node.right.as_ref();
                }
            }
        }
        None
    }
    pub fn is_empty(&self) -> bool {
        self.root.is_none()
    }
    pub fn iter(&self) -> Iter<'_, K, V> {
        let mut it = Iter {
            stack: Vec::new(),
            remaining: self.len(),
        };
        it.push_left(self.root.as_deref());
        it
    }
    pub fn keys(&self) -> impl ExactSizeIterator<Item = &K> {
        self.iter().map(|(k, _)| k)
    }
    pub fn values(&self) -> impl ExactSizeIterator<Item = &V> {
        self.iter().map(|(_, v)| v)
    }
    pub fn first_key_value(&self) -> Option<(&K, &V)> {
        let mut n = self.root.as_deref()?;
        while let Some(left) = n.left.as_deref() {
            n = left;
        }
        Some((&n.key, &n.value))
    }
    pub fn last_key_value(&self) -> Option<(&K, &V)> {
        let mut n = self.root.as_deref()?;
        while let Some(right) = n.right.as_deref() {
            n = right;
        }
        Some((&n.key, &n.value))
    }
}
impl<K: Ord, V> SnapshotMap<K, V> {
    /// Internal borrowed bounds. Reversed or empty bounds yield no rows.
    /// Seeks the lower bound in O(log n), then visits only the requested range.
    pub(crate) fn range<'a>(&'a self, lower: Bound<&'a K>, upper: Bound<&'a K>) -> Range<'a, K, V> {
        let mut range = Range {
            stack: Vec::new(),
            upper,
        };
        let mut node = self.root.as_deref();
        while let Some(n) = node {
            let below = match lower {
                Bound::Unbounded => false,
                Bound::Included(key) => n.key.as_ref() < key,
                Bound::Excluded(key) => n.key.as_ref() <= key,
            };
            if below {
                node = n.right.as_deref();
            } else {
                range.stack.push(n);
                node = n.left.as_deref();
            }
        }
        range
    }
    pub fn get(&self, key: &K) -> Option<&V> {
        let mut next = self.root.as_deref();
        while let Some(n) = next {
            match key.cmp(&n.key) {
                Ordering::Less => next = n.left.as_deref(),
                Ordering::Greater => next = n.right.as_deref(),
                Ordering::Equal => return Some(&n.value),
            }
        }
        None
    }
    pub fn contains_key(&self, key: &K) -> bool {
        self.get(key).is_some()
    }
    pub(crate) fn insert(&mut self, key: K, value: V) {
        let root = put(&self.root, Arc::new(key), Arc::new(value));
        self.root = Some(root);
    }
    pub(crate) fn remove(&mut self, key: &K) -> bool {
        let (root, found) = erase(&self.root, key);
        self.root = root;
        found
    }
}
impl<K: Ord, V> Index<&K> for SnapshotMap<K, V> {
    type Output = V;
    fn index(&self, key: &K) -> &V {
        self.get(key).expect("missing snapshot key")
    }
}
impl<K: fmt::Debug, V: fmt::Debug> fmt::Debug for SnapshotMap<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}
impl<K: PartialEq, V: PartialEq> PartialEq for SnapshotMap<K, V> {
    fn eq(&self, r: &Self) -> bool {
        self.len() == r.len() && self.iter().eq(r.iter())
    }
}
impl<K: Eq, V: Eq> Eq for SnapshotMap<K, V> {}
impl<K: PartialEq, V: PartialEq> PartialEq<BTreeMap<K, V>> for SnapshotMap<K, V> {
    fn eq(&self, r: &BTreeMap<K, V>) -> bool {
        self.len() == r.len() && self.iter().eq(r.iter())
    }
}
pub(crate) struct Range<'a, K, V> {
    stack: Vec<&'a Node<K, V>>,
    upper: Bound<&'a K>,
}
impl<'a, K: Ord, V> Iterator for Range<'a, K, V> {
    type Item = (&'a K, &'a V);
    fn next(&mut self) -> Option<Self::Item> {
        let n = self.stack.pop()?;
        let above = match self.upper {
            Bound::Unbounded => false,
            Bound::Included(key) => n.key.as_ref() > key,
            Bound::Excluded(key) => n.key.as_ref() >= key,
        };
        if above {
            self.stack.clear();
            return None;
        }
        let mut node = n.right.as_deref();
        while let Some(next) = node {
            self.stack.push(next);
            node = next.left.as_deref();
        }
        Some((&n.key, &n.value))
    }
}
impl<K: Ord, V> std::iter::FusedIterator for Range<'_, K, V> {}

/// Ascending iterator over a borrowed snapshot; uses logarithmic traversal space.
pub struct Iter<'a, K, V> {
    stack: Vec<&'a Node<K, V>>,
    remaining: usize,
}
impl<'a, K, V> Iter<'a, K, V> {
    fn push_left(&mut self, mut node: Option<&'a Node<K, V>>) {
        while let Some(n) = node {
            self.stack.push(n);
            node = n.left.as_deref();
        }
    }
}
impl<'a, K, V> Iterator for Iter<'a, K, V> {
    type Item = (&'a K, &'a V);
    fn next(&mut self) -> Option<Self::Item> {
        let n = self.stack.pop()?;
        self.push_left(n.right.as_deref());
        self.remaining -= 1;
        Some((&n.key, &n.value))
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}
impl<K, V> ExactSizeIterator for Iter<'_, K, V> {}
impl<K, V> std::iter::FusedIterator for Iter<'_, K, V> {}
impl<'a, K, V> IntoIterator for &'a SnapshotMap<K, V> {
    type Item = (&'a K, &'a V);
    type IntoIter = Iter<'a, K, V>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;
    use std::{
        cell::Cell,
        collections::HashSet,
        panic::{AssertUnwindSafe, catch_unwind},
    };
    fn check<K: Ord, V>(n: &Link<K, V>) -> (usize, usize, Option<&K>, Option<&K>) {
        let Some(n) = n else {
            return (0, 0, None, None);
        };
        let (lh, ls, lmin, lmax) = check(&n.left);
        let (rh, rs, rmin, rmax) = check(&n.right);
        assert!(lh.abs_diff(rh) <= 1);
        assert_eq!(n.height, 1 + lh.max(rh));
        assert_eq!(n.size, 1 + ls + rs);
        if let Some(k) = lmax {
            assert!(k < n.key.as_ref());
        }
        if let Some(k) = rmin {
            assert!(k > n.key.as_ref());
        }
        (
            n.height,
            n.size,
            lmin.or(Some(&n.key)),
            rmax.or(Some(&n.key)),
        )
    }
    #[test]
    fn bounded_traversal_matches_direct_filtering_and_preserves_snapshots() {
        use Bound::{Excluded, Included, Unbounded};
        let mut map = SnapshotMap::new();
        for key in -24..=24 {
            if key % 3 != 0 {
                map.insert(key, key * 2);
            }
        }
        let saved = map.clone();
        for key in -24..=24 {
            if key % 2 == 0 {
                map.remove(&key);
            }
        }
        for snapshot in [&saved, &map] {
            let keys = (-26..=26).collect::<Vec<_>>();
            let bounds = std::iter::once(Unbounded)
                .chain(keys.iter().flat_map(|key| [Included(key), Excluded(key)]))
                .collect::<Vec<_>>();
            for &lower in &bounds {
                for &upper in &bounds {
                    let expected = snapshot
                        .iter()
                        .filter(|(k, _)| {
                            let low = match lower {
                                Unbounded => true,
                                Included(b) => *k >= b,
                                Excluded(b) => *k > b,
                            };
                            let high = match upper {
                                Unbounded => true,
                                Included(b) => *k <= b,
                                Excluded(b) => *k < b,
                            };
                            low && high
                        })
                        .collect::<Vec<_>>();
                    let mut actual = snapshot.range(lower, upper);
                    assert_eq!(actual.by_ref().collect::<Vec<_>>(), expected);
                    assert!(actual.next().is_none());
                    assert!(actual.next().is_none());
                }
            }
        }
        let empty = SnapshotMap::<i32, i32>::new();
        assert!(empty.range(Unbounded, Unbounded).next().is_none());
    }
    #[test]
    fn sorted_reverse_and_generated_edits_match_standard_map_with_old_snapshots() {
        let mut map = SnapshotMap::new();
        let mut reference = BTreeMap::new();
        let mut saved = Vec::new();
        let mut seed = 20261016_u64;
        for step in 0..20_000 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let key = if step < 1024 {
                step
            } else if step < 2048 {
                2047 - step
            } else {
                (seed >> 32) as usize % 1024
            };
            if (1024..2048).contains(&step) || (step >= 2048 && seed.is_multiple_of(3)) {
                assert_eq!(map.remove(&key), reference.remove(&key).is_some());
            } else {
                map.insert(key, step);
                reference.insert(key, step);
            }
            check(&map.root);
            assert_eq!(map, reference);
            assert_eq!(map.first_key_value(), reference.first_key_value());
            assert_eq!(map.last_key_value(), reference.last_key_value());
            assert_eq!(map.iter().len(), reference.len());
            if step % 991 == 0 {
                saved.push((map.clone(), reference.clone()));
            }
        }
        for (snapshot, expected) in saved {
            check(&snapshot.root);
            assert_eq!(snapshot, expected);
        }
        for key in 0..1024 {
            map.remove(&key);
            check(&map.root);
        }
        assert!(map.is_empty());
        assert_eq!(map.iter().next(), None);
    }
    #[test]
    fn snapshots_and_updates_share_payloads_and_copy_logarithmic_paths() {
        // Neither keys nor values implement Clone.
        #[derive(Eq, PartialEq, Ord, PartialOrd)]
        struct Key(usize);
        struct Value(usize);
        fn pointers<K, V>(n: &Link<K, V>, out: &mut HashSet<usize>) {
            if let Some(n) = n {
                out.insert(Arc::as_ptr(n) as usize);
                pointers(&n.left, out);
                pointers(&n.right, out);
            }
        }
        let mut map = SnapshotMap::new();
        for k in 0..8192 {
            map.insert(Key(k), Value(k));
        }
        let base = map.clone();
        assert!(Arc::ptr_eq(
            map.root.as_ref().unwrap(),
            base.root.as_ref().unwrap()
        ));
        for key in [0, 1, 4095, 8191, 8192] {
            let mut candidate = base.clone();
            let mut before = HashSet::new();
            pointers(&base.root, &mut before);
            candidate.insert(Key(key), Value(999));
            check(&candidate.root);
            let mut after = HashSet::new();
            pointers(&candidate.root, &mut after);
            assert!(after.difference(&before).count() <= 4 * height(&base.root) + 1);
            assert_eq!(candidate.get(&Key(key)).unwrap().0, 999);
            if key < 8192 {
                assert_eq!(base.get(&Key(key)).unwrap().0, key);
            }
            candidate.remove(&Key(key));
            check(&candidate.root);
            after.clear();
            pointers(&candidate.root, &mut after);
            assert!(after.difference(&before).count() <= 8 * height(&base.root) + 2);
        }
        assert_eq!(base.len(), 8192);
    }
    #[test]
    fn comparison_panics_preserve_the_exact_original_root() {
        struct Key(i32, Rc<Cell<usize>>);
        impl PartialEq for Key {
            fn eq(&self, r: &Self) -> bool {
                self.0 == r.0
            }
        }
        impl Eq for Key {}
        impl PartialOrd for Key {
            fn partial_cmp(&self, r: &Self) -> Option<Ordering> {
                Some(self.cmp(r))
            }
        }
        impl Ord for Key {
            fn cmp(&self, r: &Self) -> Ordering {
                let left = self.1.get();
                if left > 0 {
                    self.1.set(left - 1);
                    assert_ne!(left, 1, "comparison failure");
                }
                self.0.cmp(&r.0)
            }
        }
        let count = Rc::new(Cell::new(0));
        let mut map = SnapshotMap::new();
        for k in 0..127 {
            map.insert(Key(k, count.clone()), k);
        }
        let original = map.clone();
        for fail in 1..=7 {
            count.set(fail);
            assert!(
                catch_unwind(AssertUnwindSafe(|| map.insert(Key(128, count.clone()), 128)))
                    .is_err()
            );
            assert!(Arc::ptr_eq(
                map.root.as_ref().unwrap(),
                original.root.as_ref().unwrap()
            ));
            count.set(fail);
            assert!(catch_unwind(AssertUnwindSafe(|| map.remove(&Key(0, count.clone())))).is_err());
            assert!(Arc::ptr_eq(
                map.root.as_ref().unwrap(),
                original.root.as_ref().unwrap()
            ));
        }
        count.set(0);
        check(&map.root);
        assert_eq!(map.len(), 127);
    }
}
