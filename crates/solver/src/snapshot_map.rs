//! Immutable inspection of ordered stream state with inexpensive snapshots.
//!
//! Cloning shares an AVL tree root; internal updates copy only the search and
//! rebalance paths when shared, reusing uniquely owned nodes. Keys and values
//! on unchanged paths are shared, not cloned. All comparisons precede mutation.
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

mod journal;

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
    journal: Option<Box<journal::Journal<K, V>>>,
}
impl<K, V> Clone for SnapshotMap<K, V> {
    fn clone(&self) -> Self {
        Self {
            root: self.root.clone(),
            journal: None,
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
// Clone only shared ownership, never user keys or values.
impl<K, V> Clone for Node<K, V> {
    fn clone(&self) -> Self {
        Self {
            key: self.key.clone(),
            value: self.value.clone(),
            left: self.left.clone(),
            right: self.right.clone(),
            height: self.height,
            size: self.size,
        }
    }
}
fn refresh<K, V>(n: &mut Node<K, V>) {
    n.height = 1 + height(&n.left).max(height(&n.right));
    n.size = 1 + size(&n.left) + size(&n.right);
}
fn rotate_left<K, V>(root: &mut Link<K, V>) {
    let mut old = root.take().expect("rotation root");
    let mut pivot = Arc::make_mut(&mut old).right.take().expect("right pivot");
    let middle = Arc::make_mut(&mut pivot).left.take();
    let n = Arc::make_mut(&mut old);
    n.right = middle;
    refresh(n);
    let p = Arc::make_mut(&mut pivot);
    p.left = Some(old);
    refresh(p);
    *root = Some(pivot);
}
fn rotate_right<K, V>(root: &mut Link<K, V>) {
    let mut old = root.take().expect("rotation root");
    let mut pivot = Arc::make_mut(&mut old).left.take().expect("left pivot");
    let middle = Arc::make_mut(&mut pivot).right.take();
    let n = Arc::make_mut(&mut old);
    n.left = middle;
    refresh(n);
    let p = Arc::make_mut(&mut pivot);
    p.right = Some(old);
    refresh(p);
    *root = Some(pivot);
}
fn balance<K, V>(root: &mut Link<K, V>) {
    // Ancestors already refreshed their uniquely-owned node while unwinding.
    // Avoid repeating the atomic uniqueness check on an unchanged balance.
    let Some(n) = root.as_ref() else { return };
    if height(&n.left) > height(&n.right) + 1 {
        let l = n.left.as_ref().expect("left-heavy");
        if height(&l.right) > height(&l.left) {
            rotate_left(&mut Arc::make_mut(root.as_mut().unwrap()).left);
        }
        rotate_right(root);
    } else if height(&n.right) > height(&n.left) + 1 {
        let r = n.right.as_ref().expect("right-heavy");
        if height(&r.left) > height(&r.right) {
            rotate_right(&mut Arc::make_mut(root.as_mut().unwrap()).right);
        }
        rotate_left(root);
    }
}
// Resolve all user comparisons before mutating ownership or nodes. A comparison
// panic must preserve even the exact original root, including unshared trees.
// An AVL tree with usize-sized node count has height below twice usize::BITS.
// Keep this bounded search plan on the stack instead of allocating per update.
struct Path {
    directions: [Ordering; 2 * usize::BITS as usize],
    len: usize,
}
impl Path {
    fn as_slice(&self) -> &[Ordering] {
        &self.directions[..self.len]
    }
}
fn locate<'a, K: Ord, V>(root: &'a Link<K, V>, key: &K) -> (Path, Option<&'a Arc<V>>) {
    let mut path = Path {
        directions: [Ordering::Equal; 2 * usize::BITS as usize],
        len: 0,
    };
    let mut next = root.as_deref();
    while let Some(n) = next {
        let direction = key.cmp(&n.key);
        match direction {
            Ordering::Equal => return (path, Some(&n.value)),
            Ordering::Less => next = n.left.as_deref(),
            Ordering::Greater => next = n.right.as_deref(),
        }
        path.directions[path.len] = direction;
        path.len += 1;
    }
    (path, None)
}
// Replacing a value cannot change the tree shape, heights or subtree sizes.
// Follow the prevalidated path once, copying only shared nodes; no unwind pass
// or balancing is needed. Retain the original representative of an equal key.
fn replace<K, V>(root: &mut Link<K, V>, path: &[Ordering], value: Arc<V>) {
    let mut node = Arc::make_mut(root.as_mut().expect("prepared replacement"));
    for direction in path {
        let child = if *direction == Ordering::Less {
            &mut node.left
        } else {
            &mut node.right
        };
        node = Arc::make_mut(child.as_mut().expect("prepared replacement path"));
    }
    node.value = value;
}
fn put<K, V>(root: &mut Link<K, V>, path: &[Ordering], key: Option<Arc<K>>, value: Arc<V>) {
    if let Some((&direction, tail)) = path.split_first() {
        let n = Arc::make_mut(root.as_mut().expect("prepared path"));
        put(
            if direction == Ordering::Less {
                &mut n.left
            } else {
                &mut n.right
            },
            tail,
            key,
            value,
        );
        refresh(n);
        balance(root);
    } else if let Some(n) = root {
        Arc::make_mut(n).value = value;
    } else {
        *root = Some(Arc::new(Node {
            key: key.expect("new key"),
            value,
            left: None,
            right: None,
            height: 1,
            size: 1,
        }));
    }
}
fn remove_first<K, V>(root: &mut Link<K, V>) -> (Arc<K>, Arc<V>) {
    let n = Arc::make_mut(root.as_mut().expect("nonempty successor"));
    if n.left.is_some() {
        let entry = remove_first(&mut n.left);
        refresh(n);
        balance(root);
        entry
    } else {
        let entry = (n.key.clone(), n.value.clone());
        let right = n.right.take();
        *root = right;
        entry
    }
}
fn erase<K, V>(root: &mut Link<K, V>, path: &[Ordering]) {
    let n = Arc::make_mut(root.as_mut().expect("prepared removal"));
    if let Some((&direction, tail)) = path.split_first() {
        erase(
            if direction == Ordering::Less {
                &mut n.left
            } else {
                &mut n.right
            },
            tail,
        );
    } else if n.left.is_none() {
        let child = n.right.take();
        *root = child;
        return;
    } else if n.right.is_none() {
        let child = n.left.take();
        *root = child;
        return;
    } else {
        let (key, value) = remove_first(&mut n.right);
        n.key = key;
        n.value = value;
    }
    refresh(n);
    balance(root);
}
impl<K, V> SnapshotMap<K, V> {
    // Identity only: equal roots imply unchanged content, but independently
    // constructed equal maps need not share a root.
    pub(crate) fn shares_root(&self, other: &Self) -> bool {
        match (&self.root, &other.root) {
            (None, None) => true,
            (Some(left), Some(right)) => Arc::ptr_eq(left, right),
            _ => false,
        }
    }

    pub(crate) fn new() -> Self {
        Self {
            root: None,
            journal: None,
        }
    }
    pub(crate) fn clear(&mut self) {
        self.preserve_journal();
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
        let (path, found) = locate(&self.root, &key);
        if let Some(old) = found {
            drop(key);
            journal::record_replacement(&mut self.journal, &path, old);
            replace(&mut self.root, path.as_slice(), Arc::new(value));
        } else {
            self.preserve_journal();
            put(
                &mut self.root,
                path.as_slice(),
                Some(Arc::new(key)),
                Arc::new(value),
            );
        }
    }
    pub(crate) fn remove(&mut self, key: &K) -> bool {
        let (path, found) = locate(&self.root, key);
        let found = found.is_some();
        if found {
            self.preserve_journal();
            erase(&mut self.root, path.as_slice());
        }
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
    fn unshared_updates_reuse_nodes_through_replacements_and_rotations() {
        fn addresses<K, V>(root: &Link<K, V>, out: &mut HashSet<usize>) {
            if let Some(n) = root {
                out.insert(Arc::as_ptr(n) as usize);
                addresses(&n.left, out);
                addresses(&n.right, out);
            }
        }
        for order in [vec![3, 2, 1, 0, 4, 5, 6], vec![3, 1, 2, 6, 4, 5, 0]] {
            let mut map = SnapshotMap::new();
            let mut before = HashSet::new();
            for key in order {
                map.insert(key, key);
                let mut after = HashSet::new();
                addresses(&map.root, &mut after);
                assert!(before.is_subset(&after));
                assert_eq!(after.len(), before.len() + 1);
                before = after;
                check(&map.root);
            }
            for key in 0..7 {
                map.insert(key, 99);
                let mut after = HashSet::new();
                addresses(&map.root, &mut after);
                assert_eq!(after, before);
                assert_eq!(map.get(&key), Some(&99));
                check(&map.root);
            }
            for key in [3, 0, 6, 2, 1, 4, 5] {
                assert!(map.remove(&key));
                let mut after = HashSet::new();
                addresses(&map.root, &mut after);
                assert!(after.is_subset(&before));
                assert_eq!(before.len(), after.len() + 1);
                before = after;
                check(&map.root);
            }
        }
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
        for shared in [false, true] {
            let original = shared.then(|| map.clone());
            let address = Arc::as_ptr(map.root.as_ref().unwrap()) as usize;
            for fail in 1..=7 {
                count.set(fail);
                assert!(
                    catch_unwind(AssertUnwindSafe(|| map.insert(Key(128, count.clone()), 128)))
                        .is_err()
                );
                assert_eq!(Arc::as_ptr(map.root.as_ref().unwrap()) as usize, address);
                count.set(fail);
                assert!(
                    catch_unwind(AssertUnwindSafe(|| map.insert(Key(0, count.clone()), 999)))
                        .is_err()
                );
                assert_eq!(Arc::as_ptr(map.root.as_ref().unwrap()) as usize, address);
                count.set(fail);
                assert!(
                    catch_unwind(AssertUnwindSafe(|| map.remove(&Key(0, count.clone())))).is_err()
                );
                assert_eq!(Arc::as_ptr(map.root.as_ref().unwrap()) as usize, address);
                count.set(0);
                check(&map.root);
                assert_eq!(
                    map.iter().map(|(k, v)| (k.0, *v)).collect::<Vec<_>>(),
                    (0..127).map(|k| (k, k)).collect::<Vec<_>>()
                );
                if let Some(original) = &original {
                    assert!(Arc::ptr_eq(
                        map.root.as_ref().unwrap(),
                        original.root.as_ref().unwrap()
                    ));
                }
            }
        }
        count.set(0);
        check(&map.root);
        assert_eq!(map.len(), 127);
    }
}
