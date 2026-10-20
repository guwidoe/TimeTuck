//! Direct, sparse-grown hot page rows with an ordered arbitrary-key fallback.
use super::{Page, PageKey};
use std::{
    collections::{BTreeMap, btree_map},
    sync::Arc,
};

type Owner<K, V> = Arc<Page<K, V>>;
type Row<K, V> = Vec<Option<Owner<K, V>>>;
const ROWS: usize = 4096;
const COLUMNS: usize = 64;
// These power-of-two bounds let the complete routing condition use one
// unsigned test. Keep every high bit: arbitrary usize coordinates must fall
// back to the tree rather than aliasing a low direct-index slot.
const _: () = assert!(ROWS.is_power_of_two() && COLUMNS.is_power_of_two());
#[inline(always)]
fn hot(key: PageKey) -> Option<(usize, usize)> {
    let [a, b, p, q] = key.0;
    let outside = a | b | (p & !(ROWS - 1)) | (q & !(COLUMNS - 1));
    (outside == 0).then_some((p, q))
}
pub(super) struct Directory<K, V> {
    dense: Vec<Row<K, V>>,
    dense_len: usize,
    tree: BTreeMap<PageKey, Owner<K, V>>,
}
impl<K, V> Clone for Directory<K, V> {
    fn clone(&self) -> Self {
        Self {
            dense: self.dense.clone(),
            dense_len: self.dense_len,
            tree: self.tree.clone(),
        }
    }
}
impl<K, V> Drop for Directory<K, V> {
    fn drop(&mut self) {
        self.clear();
    }
}
impl<K, V> Directory<K, V> {
    pub(super) fn new() -> Self {
        Self {
            dense: Vec::new(),
            dense_len: 0,
            tree: BTreeMap::new(),
        }
    }
    #[inline(always)]
    pub(super) fn get(&self, key: &PageKey) -> Option<&Owner<K, V>> {
        match hot(*key) {
            Some((p, q)) => self.dense.get(p)?.get(q)?.as_ref(),
            None => self.tree.get(key),
        }
    }
    #[inline(always)]
    pub(super) fn get_mut(&mut self, key: &PageKey) -> Option<&mut Owner<K, V>> {
        match hot(*key) {
            Some((p, q)) => self.dense.get_mut(p)?.get_mut(q)?.as_mut(),
            None => self.tree.get_mut(key),
        }
    }
    #[inline(always)]
    pub(super) fn entry(&mut self, key: PageKey) -> Entry<'_, K, V> {
        match hot(key) {
            Some((p, q)) => Entry::Dense {
                rows: &mut self.dense,
                count: &mut self.dense_len,
                p,
                q,
            },
            None => Entry::Tree(self.tree.entry(key)),
        }
    }
    pub(super) fn remove(&mut self, key: &PageKey) -> Option<Owner<K, V>> {
        self.entry(*key).remove()
    }
    pub(super) fn iter(&self) -> Iter<'_, K, V> {
        Iter {
            dense: DenseIter {
                rows: &self.dense,
                front: 0,
                back: self.dense.len() * COLUMNS,
                remaining: self.dense_len,
            },
            tree: self.tree.iter(),
        }
    }
    pub(super) fn keys(&self) -> Keys<'_, K, V> {
        Keys(self.iter())
    }
    pub(super) fn values(&self) -> Values<'_, K, V> {
        Values(self.iter())
    }
    pub(super) fn retain(&mut self, mut keep: impl FnMut(&PageKey, &mut Owner<K, V>) -> bool) {
        let keys: Vec<_> = self.keys().collect();
        for key in keys {
            if !keep(&key, self.get_mut(&key).expect("directory key")) {
                self.remove(&key);
            }
        }
    }
    pub(super) fn clear(&mut self) {
        // Release page owners in the same globally sorted order as the original
        // tree. Moving the directories into iterators requires no allocation.
        let dense = std::mem::take(&mut self.dense);
        let tree = std::mem::take(&mut self.tree);
        self.dense_len = 0;
        let mut dense = dense
            .into_iter()
            .enumerate()
            .flat_map(|(p, row)| {
                row.into_iter()
                    .enumerate()
                    .filter_map(move |(q, page)| page.map(|page| (PageKey([0, 0, p, q]), page)))
            })
            .peekable();
        let mut tree = tree.into_iter().peekable();
        loop {
            let take_dense = match (dense.peek(), tree.peek()) {
                (None, None) => break,
                (Some(_), None) => true,
                (None, Some(_)) => false,
                (Some((d, _)), Some((t, _))) => d < t,
            };
            if take_dense {
                drop(dense.next());
            } else {
                drop(tree.next());
            }
        }
    }
    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.dense_len + self.tree.len()
    }
    #[cfg(test)]
    pub(super) fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

pub(super) enum Entry<'a, K, V> {
    Dense {
        rows: &'a mut Vec<Row<K, V>>,
        count: &'a mut usize,
        p: usize,
        q: usize,
    },
    Tree(btree_map::Entry<'a, PageKey, Owner<K, V>>),
}
impl<'a, K, V> Entry<'a, K, V> {
    #[inline(always)]
    pub(super) fn get(&self) -> Option<&Owner<K, V>> {
        match self {
            Self::Dense { rows, p, q, .. } => rows.get(*p)?.get(*q)?.as_ref(),
            Self::Tree(btree_map::Entry::Occupied(entry)) => Some(entry.get()),
            Self::Tree(btree_map::Entry::Vacant(_)) => None,
        }
    }
    #[inline(always)]
    pub(super) fn get_mut(&mut self) -> Option<&mut Owner<K, V>> {
        match self {
            Self::Dense { rows, p, q, .. } => rows.get_mut(*p)?.get_mut(*q)?.as_mut(),
            Self::Tree(btree_map::Entry::Occupied(entry)) => Some(entry.get_mut()),
            Self::Tree(btree_map::Entry::Vacant(_)) => None,
        }
    }
    pub(super) fn or_insert_with(
        self,
        create: impl FnOnce() -> Owner<K, V>,
    ) -> &'a mut Owner<K, V> {
        match self {
            Self::Tree(entry) => entry.or_insert_with(create),
            Self::Dense { rows, count, p, q } => {
                if rows.len() <= p {
                    rows.resize_with(p + 1, Vec::new);
                }
                let row = &mut rows[p];
                if row.len() <= q {
                    row.resize_with(q + 1, || None);
                }
                let slot = &mut row[q];
                if slot.is_none() {
                    let value = create();
                    *slot = Some(value);
                    *count += 1;
                }
                slot.as_mut().unwrap()
            }
        }
    }
    pub(super) fn remove(self) -> Option<Owner<K, V>> {
        match self {
            Self::Tree(btree_map::Entry::Occupied(entry)) => Some(entry.remove()),
            Self::Tree(btree_map::Entry::Vacant(_)) => None,
            Self::Dense { rows, count, p, q } => {
                let row = rows.get_mut(p)?;
                let value = row.get_mut(q)?.take()?;
                *count -= 1;
                while row.last().is_some_and(Option::is_none) {
                    row.pop();
                }
                while rows.last().is_some_and(Vec::is_empty) {
                    rows.pop();
                }
                Some(value)
            }
        }
    }
}

struct DenseIter<'a, K, V> {
    rows: &'a [Row<K, V>],
    front: usize,
    back: usize,
    remaining: usize,
}
impl<K, V> Clone for DenseIter<'_, K, V> {
    fn clone(&self) -> Self {
        Self {
            rows: self.rows,
            front: self.front,
            back: self.back,
            remaining: self.remaining,
        }
    }
}
impl<'a, K, V> Iterator for DenseIter<'a, K, V> {
    type Item = (PageKey, &'a Owner<K, V>);
    fn next(&mut self) -> Option<Self::Item> {
        while self.front < self.back {
            let p = self.front / COLUMNS;
            let q = self.front % COLUMNS;
            let row = &self.rows[p];
            if q >= row.len() {
                self.front = (p + 1) * COLUMNS;
                continue;
            }
            self.front += 1;
            if let Some(page) = &row[q] {
                self.remaining -= 1;
                return Some((PageKey([0, 0, p, q]), page));
            }
        }
        None
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}
impl<K, V> DoubleEndedIterator for DenseIter<'_, K, V> {
    fn next_back(&mut self) -> Option<Self::Item> {
        while self.front < self.back {
            let p = (self.back - 1) / COLUMNS;
            let q = (self.back - 1) % COLUMNS;
            let row = &self.rows[p];
            if q >= row.len() {
                self.back = p * COLUMNS + row.len();
                continue;
            }
            self.back -= 1;
            if let Some(page) = &row[q] {
                self.remaining -= 1;
                return Some((PageKey([0, 0, p, q]), page));
            }
        }
        None
    }
}
impl<K, V> ExactSizeIterator for DenseIter<'_, K, V> {}
impl<K, V> std::iter::FusedIterator for DenseIter<'_, K, V> {}
pub(super) struct Iter<'a, K, V> {
    dense: DenseIter<'a, K, V>,
    tree: btree_map::Iter<'a, PageKey, Owner<K, V>>,
}
impl<'a, K, V> Iterator for Iter<'a, K, V> {
    type Item = (PageKey, &'a Owner<K, V>);
    fn next(&mut self) -> Option<Self::Item> {
        if self.dense.len() == 0 {
            return self.tree.next().map(|(key, value)| (*key, value));
        }
        if self.tree.len() == 0 {
            return self.dense.next();
        }
        let dense = self.dense.clone().next().unwrap().0;
        let tree = *self.tree.clone().next().unwrap().0;
        if dense < tree {
            self.dense.next()
        } else {
            self.tree.next().map(|(key, value)| (*key, value))
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.dense.len() + self.tree.len();
        (len, Some(len))
    }
}
impl<K, V> DoubleEndedIterator for Iter<'_, K, V> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.dense.len() == 0 {
            return self.tree.next_back().map(|(key, value)| (*key, value));
        }
        if self.tree.len() == 0 {
            return self.dense.next_back();
        }
        let dense = self.dense.clone().next_back().unwrap().0;
        let tree = *self.tree.clone().next_back().unwrap().0;
        if dense > tree {
            self.dense.next_back()
        } else {
            self.tree.next_back().map(|(key, value)| (*key, value))
        }
    }
}
impl<K, V> ExactSizeIterator for Iter<'_, K, V> {}
impl<K, V> std::iter::FusedIterator for Iter<'_, K, V> {}
pub(super) struct Keys<'a, K, V>(Iter<'a, K, V>);
impl<K, V> Iterator for Keys<'_, K, V> {
    type Item = PageKey;
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(key, _)| key)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}
impl<K, V> DoubleEndedIterator for Keys<'_, K, V> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.next_back().map(|(key, _)| key)
    }
}
impl<K, V> ExactSizeIterator for Keys<'_, K, V> {}
pub(super) struct Values<'a, K, V>(Iter<'a, K, V>);
impl<'a, K, V> Iterator for Values<'a, K, V> {
    type Item = &'a Owner<K, V>;
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(_, value)| value)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}
impl<K, V> DoubleEndedIterator for Values<'_, K, V> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.next_back().map(|(_, value)| value)
    }
}
impl<K, V> ExactSizeIterator for Values<'_, K, V> {}

#[cfg(test)]
mod tests;
