//! Allocation-free small frequency sets, persistent trees for larger collectors.
mod tree;
use tree::CountsTree;
const INLINE_VALUES: usize = 2;
#[derive(Clone, Debug)]
pub(crate) enum IntCollectorCounts {
    Inline {
        entries: [(i64, usize); INLINE_VALUES],
        len: usize,
    },
    Tree(CountsTree),
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
                        .all(|(&(value, count), (key, n))| value == key && count == n)
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
            Self::Tree(tree) => tree.get(value).is_some(),
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
                        let mut tree = CountsTree::new();
                        for &(key, count) in &entries[..*len] {
                            tree.insert_count(key, count);
                        }
                        tree.increment(value);
                        *self = Self::Tree(tree);
                    }
                }
            }
            Self::Tree(tree) => {
                tree.increment(value);
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
                if !tree.remove(value) {
                    return false;
                }
                // A lower demotion threshold avoids rebuilding a tree on
                // every alternation around the inline threshold.
                if tree.len() <= INLINE_VALUES / 2 {
                    let mut entries = [(0, 0); INLINE_VALUES];
                    let len = tree.len();
                    if let Some(key) = tree.min() {
                        entries[0] = (key, tree.get(key).expect("present sole frequency"));
                    }
                    *self = Self::Inline { entries, len };
                }
            }
        }
        true
    }
    pub(crate) fn replace(&mut self, old: i64, new: i64) -> bool {
        if old == new {
            return self.contains(old);
        }
        // Keep the ordinary inline demotion when removing one class would
        // cross its threshold. Larger trees retain their representation.
        if let Self::Tree(tree) = self
            && tree.len() > INLINE_VALUES / 2 + 1
        {
            return tree.replace_one(old, new);
        }
        if !self.remove(old) {
            return false;
        }
        self.increment(new);
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
            Self::Tree(tree) => tree.min(),
        }
    }
    pub(crate) fn max(&self) -> Option<i64> {
        match self {
            Self::Inline { entries, len } => entries[..*len].last().map(|e| e.0),
            Self::Tree(tree) => tree.max(),
        }
    }
}
