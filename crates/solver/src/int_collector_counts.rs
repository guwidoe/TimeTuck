//! Allocation-free small frequency sets, persistent trees for larger collectors.
mod tree;
use tree::CountsTree;
#[derive(Clone, Default)]
pub(crate) enum IntCollectorCounts {
    #[default]
    Empty,
    One((i64, usize)),
    Two([(i64, usize); 2]),
    Tree(CountsTree),
}
impl std::fmt::Debug for IntCollectorCounts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(values) = self.inline_values() {
            let mut entries = [(0, 0); 2];
            entries[..values.len()].copy_from_slice(values);
            f.debug_struct("Inline")
                .field("entries", &entries)
                .field("len", &values.len())
                .finish()
        } else if let Self::Tree(tree) = self {
            f.debug_tuple("Tree").field(tree).finish()
        } else {
            unreachable!()
        }
    }
}
impl PartialEq for IntCollectorCounts {
    fn eq(&self, other: &Self) -> bool {
        match (self.inline_values(), other.inline_values()) {
            (Some(a), Some(b)) => a == b,
            (None, None) => match (self, other) {
                (Self::Tree(a), Self::Tree(b)) => a == b,
                _ => unreachable!(),
            },
            (Some(entries), None) | (None, Some(entries)) => {
                let tree = if let Self::Tree(tree) = self {
                    tree
                } else if let Self::Tree(tree) = other {
                    tree
                } else {
                    unreachable!()
                };
                entries.len() == tree.len() && entries.iter().copied().eq(tree.iter())
            }
        }
    }
}
impl Eq for IntCollectorCounts {}
impl IntCollectorCounts {
    fn inline_values(&self) -> Option<&[(i64, usize)]> {
        match self {
            Self::Empty => Some(&[]),
            Self::One(value) => Some(std::slice::from_ref(value)),
            Self::Two(values) => Some(values),
            Self::Tree(_) => None,
        }
    }
    pub(crate) fn frequency(&self, value: i64) -> Option<usize> {
        match self {
            Self::Empty => None,
            Self::One((key, n)) => (*key == value).then_some(*n),
            Self::Two([a, b]) => {
                if a.0 == value {
                    Some(a.1)
                } else if b.0 == value {
                    Some(b.1)
                } else {
                    None
                }
            }
            Self::Tree(tree) => tree.get(value),
        }
    }
    pub(crate) fn contains(&self, value: i64) -> bool {
        self.frequency(value).is_some()
    }
    pub(crate) fn increment(&mut self, value: i64) {
        match self {
            Self::Empty => *self = Self::One((value, 1)),
            Self::One((key, n)) if *key == value => *n += 1,
            Self::One(old) => {
                *self = Self::Two(if value < old.0 {
                    [(value, 1), *old]
                } else {
                    [*old, (value, 1)]
                });
            }
            Self::Two([a, b]) if a.0 == value => a.1 += 1,
            Self::Two([a, b]) if b.0 == value => b.1 += 1,
            Self::Two(entries) => {
                let mut tree = CountsTree::new();
                for &(key, n) in entries.iter() {
                    tree.insert_count(key, n);
                }
                tree.increment(value);
                *self = Self::Tree(tree);
            }
            Self::Tree(tree) => tree.increment(value),
        }
    }
    pub(crate) fn remove(&mut self, value: i64) -> bool {
        match self {
            Self::Empty => return false,
            Self::One((key, n)) => {
                if *key != value {
                    return false;
                }
                if *n > 1 {
                    *n -= 1;
                } else {
                    *self = Self::Empty;
                }
            }
            Self::Two([a, b]) => {
                if a.0 == value {
                    if a.1 > 1 {
                        a.1 -= 1;
                    } else {
                        *self = Self::One(*b);
                    }
                } else if b.0 == value {
                    if b.1 > 1 {
                        b.1 -= 1;
                    } else {
                        *self = Self::One(*a);
                    }
                } else {
                    return false;
                }
            }
            Self::Tree(tree) => {
                if !tree.remove(value) {
                    return false;
                }
                // Preserve the original lower demotion threshold.
                if tree.len() <= 1 {
                    *self = match tree.min() {
                        Some(key) => {
                            Self::One((key, tree.get(key).expect("present sole frequency")))
                        }
                        None => Self::Empty,
                    };
                }
            }
        }
        true
    }
    pub(crate) fn replace(&mut self, old: i64, new: i64) -> bool {
        if old == new {
            return self.contains(old);
        }
        if let Self::Tree(tree) = self
            && tree.len() > 2
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
            Self::Empty => 0,
            Self::One(_) => 1,
            Self::Two(_) => 2,
            Self::Tree(t) => t.len(),
        }
    }
    pub(crate) fn min(&self) -> Option<i64> {
        match self {
            Self::Empty => None,
            Self::One(a) => Some(a.0),
            Self::Two([a, _]) => Some(a.0),
            Self::Tree(t) => t.min(),
        }
    }
    pub(crate) fn max(&self) -> Option<i64> {
        match self {
            Self::Empty => None,
            Self::One(a) => Some(a.0),
            Self::Two([_, b]) => Some(b.0),
            Self::Tree(t) => t.max(),
        }
    }
}
#[cfg(test)]
mod state_tests;
