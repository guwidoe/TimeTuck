use crate::ReversibleCollector;
use std::{cmp::Ordering, fmt, sync::Arc};

type Link<V> = Option<Arc<Node<V>>>;
#[derive(Debug)]
struct Node<V> {
    key: Arc<V>,
    count: usize,
    left: Link<V>,
    right: Link<V>,
    height: usize,
    size: usize,
}
impl<V> Clone for Node<V> {
    fn clone(&self) -> Self {
        Self {
            key: self.key.clone(),
            count: self.count,
            left: self.left.clone(),
            right: self.right.clone(),
            height: self.height,
            size: self.size,
        }
    }
}
fn height<V>(root: &Link<V>) -> usize {
    root.as_ref().map_or(0, |n| n.height)
}
fn size<V>(root: &Link<V>) -> usize {
    root.as_ref().map_or(0, |n| n.size)
}
fn refresh<V>(node: &mut Node<V>) {
    node.height = 1 + height(&node.left).max(height(&node.right));
    node.size = 1 + size(&node.left) + size(&node.right);
}
fn rotate_left<V>(root: &mut Link<V>) {
    let mut old = root.take().expect("rotation root");
    let mut pivot = Arc::make_mut(&mut old).right.take().expect("right pivot");
    let middle = Arc::make_mut(&mut pivot).left.take();
    let node = Arc::make_mut(&mut old);
    node.right = middle;
    refresh(node);
    let node = Arc::make_mut(&mut pivot);
    node.left = Some(old);
    refresh(node);
    *root = Some(pivot);
}
fn rotate_right<V>(root: &mut Link<V>) {
    let mut old = root.take().expect("rotation root");
    let mut pivot = Arc::make_mut(&mut old).left.take().expect("left pivot");
    let middle = Arc::make_mut(&mut pivot).right.take();
    let node = Arc::make_mut(&mut old);
    node.left = middle;
    refresh(node);
    let node = Arc::make_mut(&mut pivot);
    node.right = Some(old);
    refresh(node);
    *root = Some(pivot);
}
fn balance<V>(root: &mut Link<V>) {
    let Some(node) = root else { return };
    if height(&node.left) > height(&node.right) + 1 {
        let left = node.left.as_ref().unwrap();
        if height(&left.right) > height(&left.left) {
            rotate_left(&mut Arc::make_mut(node).left);
        }
        rotate_right(root);
    } else if height(&node.right) > height(&node.left) + 1 {
        let right = node.right.as_ref().unwrap();
        if height(&right.left) > height(&right.right) {
            rotate_right(&mut Arc::make_mut(node).right);
        }
        rotate_left(root);
    }
}
// A usize-sized AVL has height below twice usize::BITS. Lookup plans own no
// references: callers can finish every user callback before committing changes.
struct Path {
    directions: [Ordering; 2 * usize::BITS as usize],
    len: usize,
}
impl Path {
    fn as_slice(&self) -> &[Ordering] {
        &self.directions[..self.len]
    }
}
fn locate<V: Ord>(root: &Link<V>, key: &V) -> (Path, Option<usize>) {
    let mut path = Path {
        directions: [Ordering::Equal; 2 * usize::BITS as usize],
        len: 0,
    };
    let mut next = root.as_deref();
    while let Some(node) = next {
        let direction = key.cmp(node.key.as_ref());
        next = match direction {
            Ordering::Less => node.left.as_deref(),
            Ordering::Greater => node.right.as_deref(),
            Ordering::Equal => return (path, Some(node.count)),
        };
        path.directions[path.len] = direction;
        path.len += 1;
    }
    (path, None)
}
fn set_count_at<V>(root: &mut Link<V>, path: &[Ordering], count: usize) {
    let mut node = Arc::make_mut(root.as_mut().expect("prepared existing class"));
    for direction in path {
        let child = if *direction == Ordering::Less {
            &mut node.left
        } else {
            &mut node.right
        };
        node = Arc::make_mut(child.as_mut().expect("prepared existing path"));
    }
    node.count = count;
}
fn insert_at<V>(root: &mut Link<V>, path: &[Ordering], inserted: Arc<Node<V>>) {
    if let Some((direction, tail)) = path.split_first() {
        let node = Arc::make_mut(root.as_mut().expect("prepared insertion path"));
        insert_at(
            if *direction == Ordering::Less {
                &mut node.left
            } else {
                &mut node.right
            },
            tail,
            inserted,
        );
        refresh(node);
        balance(root);
    } else {
        debug_assert!(root.is_none());
        *root = Some(inserted);
    }
}
fn increment<V: Ord>(root: &mut Link<V>, key: Arc<V>) {
    let Some(owner) = root else {
        *root = Some(Arc::new(Node {
            key,
            count: 1,
            left: None,
            right: None,
            height: 1,
            size: 1,
        }));
        return;
    };
    let node = Arc::make_mut(owner);
    match key.cmp(&node.key) {
        Ordering::Less => increment(&mut node.left, key),
        Ordering::Greater => increment(&mut node.right, key),
        Ordering::Equal => {
            // The owning collector preflights its total occurrence count.
            node.count += 1;
            return;
        }
    }
    refresh(node);
    balance(root);
}
fn set_count<V: Ord>(root: &mut Link<V>, key: &V, count: usize) {
    let mut node = Arc::make_mut(root.as_mut().expect("present frequency"));
    loop {
        let child = match key.cmp(node.key.as_ref()) {
            Ordering::Less => &mut node.left,
            Ordering::Greater => &mut node.right,
            Ordering::Equal => {
                node.count = count;
                return;
            }
        };
        node = Arc::make_mut(child.as_mut().expect("present frequency path"));
    }
}
// Detach one class while preserving AVL metadata and shared snapshots.
fn detach_first<V>(root: &mut Link<V>) -> Arc<Node<V>> {
    if root.as_ref().unwrap().left.is_some() {
        let node = Arc::make_mut(root.as_mut().unwrap());
        let detached = detach_first(&mut node.left);
        refresh(node);
        balance(root);
        detached
    } else {
        let mut detached = root.take().unwrap();
        let node = Arc::make_mut(&mut detached);
        *root = node.right.take();
        node.height = 1;
        node.size = 1;
        detached
    }
}
fn detach_at<V>(root: &mut Link<V>, path: &[Ordering]) -> Arc<Node<V>> {
    if let Some((direction, tail)) = path.split_first() {
        let node = Arc::make_mut(root.as_mut().expect("prepared removal path"));
        let detached = detach_at(
            if *direction == Ordering::Less {
                &mut node.left
            } else {
                &mut node.right
            },
            tail,
        );
        refresh(node);
        balance(root);
        return detached;
    }
    let node = root.as_ref().unwrap();
    if node.left.is_none() || node.right.is_none() {
        let mut detached = root.take().unwrap();
        let node = Arc::make_mut(&mut detached);
        *root = if node.left.is_none() {
            node.right.take()
        } else {
            node.left.take()
        };
        node.height = 1;
        node.size = 1;
        return detached;
    }
    let node = Arc::make_mut(root.as_mut().unwrap());
    let mut detached = detach_first(&mut node.right);
    let removed = Arc::make_mut(&mut detached);
    std::mem::swap(&mut node.key, &mut removed.key);
    std::mem::swap(&mut node.count, &mut removed.count);
    refresh(node);
    balance(root);
    detached
}

/// Immutable, sorted membership snapshot. Multiplicities remain private.
/// Keys must retain stable, lawful ordering; destructors must not panic.
pub struct SnapshotSet<V> {
    root: Link<V>,
}
impl<V> Clone for SnapshotSet<V> {
    fn clone(&self) -> Self {
        Self {
            root: self.root.clone(),
        }
    }
}
impl<V> Default for SnapshotSet<V> {
    fn default() -> Self {
        Self { root: None }
    }
}
impl<V> SnapshotSet<V> {
    pub fn len(&self) -> usize {
        size(&self.root)
    }
    pub fn is_empty(&self) -> bool {
        self.root.is_none()
    }
    pub fn iter(&self) -> SnapshotSetIter<'_, V> {
        SnapshotSetIter {
            inline: [None; 32],
            depth: 0,
            overflow: Vec::new(),
            next: self.root.as_deref(),
            remaining: self.len(),
        }
    }
    pub fn first(&self) -> Option<&V> {
        let mut node = self.root.as_deref()?;
        while let Some(left) = node.left.as_deref() {
            node = left;
        }
        Some(node.key.as_ref())
    }
    pub fn last(&self) -> Option<&V> {
        let mut node = self.root.as_deref()?;
        while let Some(right) = node.right.as_deref() {
            node = right;
        }
        Some(node.key.as_ref())
    }
}
/// Ascending borrowed set traversal, with exact remaining membership count.
/// Ordinary trees use inline traversal storage; unusually deep trees can spill.
pub struct SnapshotSetIter<'a, V> {
    inline: [Option<&'a Node<V>>; 32],
    depth: usize,
    overflow: Vec<&'a Node<V>>,
    next: Option<&'a Node<V>>,
    remaining: usize,
}
impl<'a, V> Iterator for SnapshotSetIter<'a, V> {
    type Item = &'a V;
    fn next(&mut self) -> Option<Self::Item> {
        while let Some(node) = self.next {
            if self.depth < self.inline.len() {
                self.inline[self.depth] = Some(node);
                self.depth += 1;
            } else {
                self.overflow.push(node);
            }
            self.next = node.left.as_deref();
        }
        let node = if let Some(node) = self.overflow.pop() {
            node
        } else if self.depth > 0 {
            self.depth -= 1;
            self.inline[self.depth]
                .take()
                .expect("occupied traversal entry")
        } else {
            return None;
        };
        self.next = node.right.as_deref();
        self.remaining -= 1;
        Some(node.key.as_ref())
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}
impl<V> ExactSizeIterator for SnapshotSetIter<'_, V> {}
impl<V> std::iter::FusedIterator for SnapshotSetIter<'_, V> {}
impl<'a, V> IntoIterator for &'a SnapshotSet<V> {
    type Item = &'a V;
    type IntoIter = SnapshotSetIter<'a, V>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
impl<V: Ord> SnapshotSet<V> {
    fn count(&self, key: &V) -> Option<usize> {
        let mut next = self.root.as_deref();
        while let Some(node) = next {
            match key.cmp(node.key.as_ref()) {
                Ordering::Less => next = node.left.as_deref(),
                Ordering::Greater => next = node.right.as_deref(),
                Ordering::Equal => return Some(node.count),
            }
        }
        None
    }
    pub fn contains(&self, key: &V) -> bool {
        self.count(key).is_some()
    }
    fn remove_one(&mut self, key: &V, count: usize) {
        if count == 1 {
            let (path, found) = locate(&self.root, key);
            assert!(found.is_some(), "present captured class");
            drop(detach_at(&mut self.root, path.as_slice()));
        } else {
            set_count(&mut self.root, key, count - 1);
        }
    }
}
impl<V: fmt::Debug> fmt::Debug for SnapshotSet<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set().entries(self.iter()).finish()
    }
}
impl<V: Ord> PartialEq for SnapshotSet<V> {
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len()
            && self
                .iter()
                .zip(other.iter())
                .all(|(a, b)| a.cmp(b) == Ordering::Equal)
    }
}
impl<V: Ord> Eq for SnapshotSet<V> {}

/// One persistent counted AVL; finish shares its root in constant time.
/// The first representative of an ordering class survives until its final
/// occurrence is removed. Replacement has removal-then-insertion semantics.
/// Eligible replacements preflight callbacks and commit prepared paths; other
/// cases stage an isolated root. Clone must preserve the ordering class.
///
/// ```
/// use timetuck::{ReversibleCollector, SortedSetCollector};
/// let mut values = SortedSetCollector::<String>::default();
/// values.insert(&"A".into()); values.insert(&"A".into());
/// let saved = values.finish();
/// assert!(values.replace(&"A".into(), &"B".into()));
/// assert_eq!(values.finish().iter().map(String::as_str).collect::<Vec<_>>(), ["A", "B"]);
/// assert_eq!(saved.iter().map(String::as_str).collect::<Vec<_>>(), ["A"]);
/// ```
/// Ordering and clone panics leave the collector unchanged. Ordering must be
/// stable and lawful; interior key mutation and panicking destructors are unsupported.
pub struct SortedSetCollector<V> {
    values: SnapshotSet<V>,
    count: usize,
}
impl<V> Clone for SortedSetCollector<V> {
    fn clone(&self) -> Self {
        Self {
            values: self.values.clone(),
            count: self.count,
        }
    }
}
impl<V> Default for SortedSetCollector<V> {
    fn default() -> Self {
        Self {
            values: SnapshotSet::default(),
            count: 0,
        }
    }
}
impl<V: Ord + Clone> SortedSetCollector<V> {
    fn add(values: &mut SnapshotSet<V>, value: &V) {
        if let Some(count) = values.count(value) {
            let count = count.checked_add(1).expect("collector count overflow");
            set_count(&mut values.root, value, count);
        } else {
            increment(&mut values.root, Arc::new(value.clone()));
        }
    }
}
impl<V: Ord + Clone> ReversibleCollector<V> for SortedSetCollector<V> {
    type Output = SnapshotSet<V>;
    fn insert(&mut self, value: &V) {
        let count = self.count.checked_add(1).expect("collector count overflow");
        let mut next = self.values.clone();
        Self::add(&mut next, value);
        self.values = next;
        self.count = count;
    }
    fn remove(&mut self, value: &V) -> bool {
        let Some(count) = self.values.count(value) else {
            return false;
        };
        let mut next = self.values.clone();
        next.remove_one(value, count);
        self.values = next;
        self.count -= 1;
        true
    }
    fn replace(&mut self, old: &V, new: &V) -> bool {
        let (old_path, Some(count)) = locate(&self.values.root, old) else {
            return false;
        };
        let (new_path, destination) = locate(&self.values.root, new);
        if count > 1 {
            if let Some(destination) = destination {
                if old_path.as_slice() == new_path.as_slice() {
                    return true;
                }
                let destination = destination
                    .checked_add(1)
                    .expect("collector count overflow");
                // Neither count write changes topology; both prepared paths stay valid.
                set_count_at(&mut self.values.root, old_path.as_slice(), count - 1);
                set_count_at(&mut self.values.root, new_path.as_slice(), destination);
            } else {
                let key = Arc::new(new.clone());
                assert_eq!(
                    key.as_ref().cmp(new),
                    Ordering::Equal,
                    "Clone must preserve ordering class"
                );
                let (path, found) = locate(&self.values.root, key.as_ref());
                assert!(found.is_none(), "stable absent ordering class");
                let inserted = Arc::new(Node {
                    key,
                    count: 1,
                    left: None,
                    right: None,
                    height: 1,
                    size: 1,
                });
                // Allocation and every comparison/clone precede either write.
                set_count_at(&mut self.values.root, old_path.as_slice(), count - 1);
                insert_at(&mut self.values.root, path.as_slice(), inserted);
            }
            return true;
        }
        if let Some(destination) = destination
            && old_path.as_slice() != new_path.as_slice()
        {
            let destination = destination
                .checked_add(1)
                .expect("collector count overflow");
            // Increase the different destination before structural deletion. The
            // count write preserves the old path, so detach needs no comparisons.
            set_count_at(&mut self.values.root, new_path.as_slice(), destination);
            drop(detach_at(&mut self.values.root, old_path.as_slice()));
            return true;
        }
        // Structural removal followed by a new class (or equal singleton with a
        // new representative) retains ordinary isolated staging.
        let mut next = self.values.clone();
        next.remove_one(old, count);
        Self::add(&mut next, new);
        self.values = next;
        true
    }
    fn finish(&self) -> Self::Output {
        self.values.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iterator_spills_and_returns_to_inline_storage_with_exact_remaining_length() {
        // Synthetic ordered tree, deliberately deeper than an ordinary small
        // AVL: exercise iterator spill without allocating millions of classes.
        // Right leaves force pushes while the left spine is still spilled.
        let mut root = None;
        for key in 0..70 {
            root = Some(Arc::new(Node {
                key: Arc::new(2 * key),
                count: 1,
                left: root,
                right: Some(Arc::new(Node {
                    key: Arc::new(2 * key + 1),
                    count: 1,
                    left: None,
                    right: None,
                    height: 1,
                    size: 1,
                })),
                height: key as usize + 2,
                size: (key as usize + 1) * 2,
            }));
        }
        let set = SnapshotSet { root };
        let mut iterator = set.iter();
        for expected in 0..140 {
            assert_eq!(iterator.size_hint(), (140 - expected, Some(140 - expected)));
            assert_eq!(iterator.len(), 140 - expected);
            assert_eq!(iterator.next(), Some(&(expected as i32)));
        }
        assert_eq!(iterator.len(), 0);
        assert!(iterator.next().is_none());
        assert!(iterator.next().is_none());
    }
    #[test]
    fn prepared_edits_preserve_avl_metadata_frequencies_and_saved_roots() {
        use std::collections::BTreeMap;
        fn inspect(root: &Link<i32>, rows: &mut Vec<(i32, usize)>) -> (usize, usize) {
            let Some(node) = root else {
                return (0, 0);
            };
            let (lh, ls) = inspect(&node.left, rows);
            if let Some(&(key, _)) = rows.last() {
                assert!(key < *node.key);
            }
            assert!(node.count > 0);
            rows.push((*node.key, node.count));
            let (rh, rs) = inspect(&node.right, rows);
            assert!(lh.abs_diff(rh) <= 1);
            assert_eq!(node.height, 1 + lh.max(rh));
            assert_eq!(node.size, 1 + ls + rs);
            (node.height, node.size)
        }
        fn check(set: &SnapshotSet<i32>, model: &BTreeMap<i32, usize>) {
            let mut rows = Vec::new();
            inspect(&set.root, &mut rows);
            assert_eq!(
                rows,
                model.iter().map(|(&k, &v)| (k, v)).collect::<Vec<_>>()
            );
            assert_eq!(set.len(), model.len());
        }
        fn remove(model: &mut BTreeMap<i32, usize>, key: i32) -> bool {
            let Some(n) = model.get_mut(&key) else {
                return false;
            };
            *n -= 1;
            if *n == 0 {
                model.remove(&key);
            }
            true
        }
        let mut c = SortedSetCollector::default();
        let mut model = BTreeMap::<i32, usize>::new();
        let mut saved = Vec::new();
        let mut seed = 282_u64;
        for step in 0..6000 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let key = ((seed >> 32) % 257) as i32 - 128;
            match seed % 3 {
                0 => {
                    c.insert(&key);
                    *model.entry(key).or_default() += 1;
                }
                1 => {
                    assert_eq!(c.remove(&key), remove(&mut model, key));
                }
                _ => {
                    let new = key + ((seed >> 12) % 13) as i32 - 6;
                    let found = remove(&mut model, key);
                    if found {
                        *model.entry(new).or_default() += 1;
                    }
                    assert_eq!(c.replace(&key, &new), found);
                }
            }
            check(&c.values, &model);
            assert_eq!(c.count, model.values().sum::<usize>());
            if step % 251 == 0 {
                saved.push((c.finish(), model.clone()));
            }
        }
        for (set, model) in saved {
            check(&set, &model);
        }
    }
}
