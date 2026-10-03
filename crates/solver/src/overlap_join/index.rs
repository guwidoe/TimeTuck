//! Persistent interval index, derived from TimeTuck's own AVL map.
use std::{cmp::Ordering, sync::Arc};
type Link<K, V> = Option<Arc<Node<K, V>>>;
struct Node<K, V> {
    key: Arc<K>,
    value: Arc<V>,
    max_end: Arc<V>,
    left: Link<K, V>,
    right: Link<K, V>,
    height: usize,
    size: usize,
}
fn height<K, V>(n: &Link<K, V>) -> usize {
    n.as_ref().map_or(0, |n| n.height)
}
fn size<K, V>(n: &Link<K, V>) -> usize {
    n.as_ref().map_or(0, |n| n.size)
}
fn node<K, V: Ord>(
    key: Arc<K>,
    value: Arc<V>,
    left: Link<K, V>,
    right: Link<K, V>,
) -> Arc<Node<K, V>> {
    let mut max_end = value.clone();
    for child in [&left, &right].into_iter().flatten() {
        if child.max_end > max_end {
            max_end = child.max_end.clone();
        }
    }
    Arc::new(Node {
        max_end,
        key,
        value,
        height: 1 + height(&left).max(height(&right)),
        size: 1 + size(&left) + size(&right),
        left,
        right,
    })
}
fn balance<K, V: Ord>(
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
fn put<K: Ord, V: Ord>(root: &Link<K, V>, key: Arc<K>, value: Arc<V>) -> Arc<Node<K, V>> {
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
fn remove_first<K, V: Ord>(n: &Arc<Node<K, V>>) -> Link<K, V> {
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
fn erase<K: Ord, V: Ord>(root: &Link<K, V>, key: &K) -> (Link<K, V>, bool) {
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

pub(super) struct IntervalIndex<I, K> {
    root: Link<(K, I), K>,
}
impl<I, K> Clone for IntervalIndex<I, K> {
    fn clone(&self) -> Self {
        Self {
            root: self.root.clone(),
        }
    }
}
impl<I: Ord, K: Ord> IntervalIndex<I, K> {
    pub(super) fn new() -> Self {
        Self { root: None }
    }
    pub(super) fn is_empty(&self) -> bool {
        self.root.is_none()
    }
    pub(super) fn insert(&mut self, start: K, id: I, end: K) {
        self.root = Some(put(&self.root, Arc::new((start, id)), Arc::new(end)));
    }
    pub(super) fn remove(&mut self, key: &(K, I)) {
        self.root = erase(&self.root, key).0;
    }
    pub(super) fn query<'a>(&'a self, start: &'a K, end: &'a K) -> Matches<'a, I, K> {
        let mut out = Matches {
            stack: Vec::new(),
            start,
            end,
        };
        out.push(self.root.as_deref());
        out
    }
}
pub(super) struct Matches<'a, I, K> {
    stack: Vec<&'a Node<(K, I), K>>,
    start: &'a K,
    end: &'a K,
}
impl<'a, I, K: Ord> Matches<'a, I, K> {
    fn push(&mut self, mut node: Option<&'a Node<(K, I), K>>) {
        while let Some(n) = node {
            if n.max_end.as_ref() <= self.start {
                return;
            }
            if &n.key.0 < self.end {
                self.stack.push(n);
            }
            node = n.left.as_deref();
        }
    }
}
impl<'a, I, K: Ord> Iterator for Matches<'a, I, K> {
    type Item = &'a I;
    fn next(&mut self) -> Option<Self::Item> {
        while let Some(n) = self.stack.pop() {
            self.push(n.right.as_deref());
            if n.value.as_ref() > self.start {
                return Some(&n.key.1);
            }
        }
        None
    }
}
impl<I, K: Ord> std::iter::FusedIterator for Matches<'_, I, K> {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    fn verify(n: &Link<(i32, usize), i32>, keys: &mut Vec<(i32, usize)>) -> (usize, usize, i32) {
        let Some(n) = n else { return (0, 0, i32::MIN) };
        let (lh, ls, lm) = verify(&n.left, keys);
        keys.push(*n.key);
        let (rh, rs, rm) = verify(&n.right, keys);
        assert!(lh.abs_diff(rh) <= 1);
        assert_eq!(n.height, 1 + lh.max(rh));
        assert_eq!(n.size, 1 + ls + rs);
        assert_eq!(*n.max_end, (*n.value).max(lm).max(rm));
        (n.height, n.size, *n.max_end)
    }
    #[test]
    fn rotations_deletions_maximum_shrink_and_snapshots_match_naive_queries() {
        let mut index = IntervalIndex::new();
        let mut rows = BTreeMap::new();
        let mut seed = 20261117_u64;
        let mut next = || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            (seed >> 32) as u32
        };
        let mut snapshots = Vec::new();
        for step in 0..4000 {
            let key = ((next() % 17) as i32 - 8, next() as usize % 32);
            if step % 3 == 0 {
                index.remove(&key);
                rows.remove(&key);
            } else {
                let end = (next() % 21) as i32 - 10;
                index.insert(key.0, key.1, end);
                rows.insert(key, end);
            }
            let mut keys = Vec::new();
            verify(&index.root, &mut keys);
            assert_eq!(keys, rows.keys().copied().collect::<Vec<_>>());
            for _ in 0..3 {
                let a = (next() % 25) as i32 - 12;
                let b = (next() % 25) as i32 - 12;
                assert_eq!(
                    index.query(&a, &b).copied().collect::<Vec<_>>(),
                    rows.iter()
                        .filter(|((s, _), e)| *s < b && **e > a)
                        .map(|((_, id), _)| *id)
                        .collect::<Vec<_>>()
                );
            }
            if step % 997 == 0 {
                snapshots.push((index.clone(), rows.clone()));
            }
        }
        for (key, _) in rows {
            index.remove(&key);
        }
        assert!(index.is_empty());
        for (saved, rows) in snapshots {
            let mut keys = Vec::new();
            verify(&saved.root, &mut keys);
            for a in -12..12 {
                for b in -12..12 {
                    assert_eq!(
                        saved.query(&a, &b).copied().collect::<Vec<_>>(),
                        rows.iter()
                            .filter(|((s, _), e)| *s < b && **e > a)
                            .map(|((_, id), _)| *id)
                            .collect::<Vec<_>>()
                    );
                }
            }
        }
    }
}
