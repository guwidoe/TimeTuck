//! Allocation-free small frequency sets, persistent trees for larger collectors.
use crate::SnapshotMap;
const INLINE_VALUES: usize = 2;
#[derive(Clone, Debug)]
pub(crate) enum IntCollectorCounts {
    Inline {
        entries: [(i64, usize); INLINE_VALUES],
        len: usize,
    },
    Tree(SnapshotMap<i64, usize>),
}
impl PartialEq for IntCollectorCounts {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::Inline {
                    entries: a,
                    len: an,
                },
                Self::Inline {
                    entries: b,
                    len: bn,
                },
            ) => a[..*an] == b[..*bn],
            (Self::Tree(a), Self::Tree(b)) => a == b,
            (Self::Inline { entries, len }, Self::Tree(tree))
            | (Self::Tree(tree), Self::Inline { entries, len }) => {
                *len == tree.len()
                    && entries[..*len]
                        .iter()
                        .zip(tree.iter())
                        .all(|(&(value, count), (&key, &n))| value == key && count == n)
            }
        }
    }
}
impl Eq for IntCollectorCounts {}
impl Default for IntCollectorCounts {
    fn default() -> Self {
        Self::Inline {
            entries: [(0, 0); INLINE_VALUES],
            len: 0,
        }
    }
}
impl IntCollectorCounts {
    pub(crate) fn contains(&self, value: i64) -> bool {
        match self {
            Self::Inline { entries, len } => entries[..*len]
                .binary_search_by_key(&value, |e| e.0)
                .is_ok(),
            Self::Tree(tree) => tree.contains_key(&value),
        }
    }
    pub(crate) fn increment(&mut self, value: i64) {
        match self {
            Self::Inline { entries, len } => {
                match entries[..*len].binary_search_by_key(&value, |e| e.0) {
                    Ok(i) => entries[i].1 += 1,
                    Err(i) if *len < INLINE_VALUES => {
                        entries.copy_within(i..*len, i + 1);
                        entries[i] = (value, 1);
                        *len += 1;
                    }
                    Err(_) => {
                        let mut tree = SnapshotMap::new();
                        for &(key, count) in &entries[..*len] {
                            tree.insert(key, count);
                        }
                        tree.insert(value, 1);
                        *self = Self::Tree(tree);
                    }
                }
            }
            Self::Tree(tree) => {
                tree.insert(value, tree.get(&value).copied().unwrap_or(0) + 1);
            }
        }
    }
    pub(crate) fn remove(&mut self, value: i64) -> bool {
        match self {
            Self::Inline { entries, len } => {
                let Ok(i) = entries[..*len].binary_search_by_key(&value, |e| e.0) else {
                    return false;
                };
                if entries[i].1 > 1 {
                    entries[i].1 -= 1;
                } else {
                    entries.copy_within(i + 1..*len, i);
                    *len -= 1;
                    entries[*len] = (0, 0);
                }
            }
            Self::Tree(tree) => {
                let Some(&count) = tree.get(&value) else {
                    return false;
                };
                if count > 1 {
                    tree.insert(value, count - 1);
                } else {
                    tree.remove(&value);
                    // A lower demotion threshold avoids rebuilding a tree on
                    // every alternation around the inline threshold.
                    if tree.len() <= INLINE_VALUES / 2 {
                        let mut entries = [(0, 0); INLINE_VALUES];
                        let len = tree.len();
                        for (slot, (&key, &count)) in entries.iter_mut().zip(tree.iter()) {
                            *slot = (key, count);
                        }
                        *self = Self::Inline { entries, len };
                    }
                }
            }
        }
        true
    }
    pub(crate) fn len(&self) -> usize {
        match self {
            Self::Inline { len, .. } => *len,
            Self::Tree(tree) => tree.len(),
        }
    }
    pub(crate) fn min(&self) -> Option<i64> {
        match self {
            Self::Inline { entries, len } => entries[..*len].first().map(|e| e.0),
            Self::Tree(tree) => tree.first_key_value().map(|(&k, _)| k),
        }
    }
    pub(crate) fn max(&self) -> Option<i64> {
        match self {
            Self::Inline { entries, len } => entries[..*len].last().map(|e| e.0),
            Self::Tree(tree) => tree.last_key_value().map(|(&k, _)| k),
        }
    }
}
