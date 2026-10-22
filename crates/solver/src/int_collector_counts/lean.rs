//! Frequency storage using the owning collector's existing total count.
//! One class needs no stored frequency; two classes need only the first one.
//! Larger sets retain the original persistent tree and lower demotion threshold.
use super::CountsTree;
#[derive(Clone, Default)]
pub(crate) enum LeanCounts {
    #[default]
    Empty,
    One(i64),
    Two([i64; 2], usize),
    Tree(CountsTree),
}
impl LeanCounts {
    fn entries(&self, total: usize) -> Option<([(i64, usize); 2], usize)> {
        match self {
            Self::Empty => Some(([(0, 0); 2], 0)),
            Self::One(v) => Some(([(*v, total), (0, 0)], 1)),
            Self::Two([a, b], n) => Some(([(*a, *n), (*b, total - n)], 2)),
            Self::Tree(_) => None,
        }
    }
    pub(crate) fn equivalent(&self, other: &Self, total: usize) -> bool {
        match (self.entries(total), other.entries(total)) {
            (Some((a, an)), Some((b, bn))) => an == bn && a[..an] == b[..bn],
            (None, None) => match (self, other) {
                (Self::Tree(a), Self::Tree(b)) => a == b,
                _ => unreachable!(),
            },
            (Some((entries, n)), None) | (None, Some((entries, n))) => {
                let tree = match (self, other) {
                    (Self::Tree(t), _) | (_, Self::Tree(t)) => t,
                    _ => unreachable!(),
                };
                n == tree.len() && entries[..n].iter().copied().eq(tree.iter())
            }
        }
    }
    pub(crate) fn debug(&self, total: usize) -> impl std::fmt::Debug + '_ {
        struct View<'a>(&'a LeanCounts, usize);
        impl std::fmt::Debug for View<'_> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                if let Some((entries, len)) = self.0.entries(self.1) {
                    f.debug_struct("Inline")
                        .field("entries", &entries)
                        .field("len", &len)
                        .finish()
                } else if let LeanCounts::Tree(tree) = self.0 {
                    f.debug_tuple("Tree").field(tree).finish()
                } else {
                    unreachable!()
                }
            }
        }
        View(self, total)
    }
    #[cfg(test)]
    pub(crate) fn frequency(&self, value: i64, total: usize) -> Option<usize> {
        match self {
            Self::Empty => None,
            Self::One(v) => (*v == value).then_some(total),
            Self::Two([a, b], n) => {
                if *a == value {
                    Some(*n)
                } else if *b == value {
                    Some(total - n)
                } else {
                    None
                }
            }
            Self::Tree(tree) => tree.get(value),
        }
    }
    pub(crate) fn contains(&self, value: i64) -> bool {
        match self {
            Self::Empty => false,
            Self::One(v) => *v == value,
            Self::Two([a, b], _) => *a == value || *b == value,
            Self::Tree(tree) => tree.get(value).is_some(),
        }
    }
    #[inline(always)]
    pub(crate) fn increment(&mut self, value: i64, total: usize) {
        match self {
            Self::Empty => *self = Self::One(value),
            Self::One(v) if *v == value => {}
            Self::One(old) => {
                *self = if value < *old {
                    Self::Two([value, *old], 1)
                } else {
                    Self::Two([*old, value], total)
                };
            }
            Self::Two([a, _], n) if *a == value => *n += 1,
            Self::Two([_, b], _) if *b == value => {}
            Self::Two([a, b], n) => {
                let mut tree = CountsTree::new();
                tree.insert_count(*a, *n);
                tree.insert_count(*b, total - *n);
                tree.increment(value);
                *self = Self::Tree(tree);
            }
            Self::Tree(tree) => tree.increment(value),
        }
    }
    #[inline(always)]
    pub(crate) fn remove(&mut self, value: i64, total: usize) -> bool {
        match self {
            Self::Empty => return false,
            Self::One(v) => {
                if *v != value {
                    return false;
                }
                if total == 1 {
                    *self = Self::Empty;
                }
            }
            Self::Two([a, b], n) => {
                if *a == value {
                    if *n > 1 {
                        *n -= 1;
                    } else {
                        *self = Self::One(*b);
                    }
                } else if *b == value {
                    if total - *n == 1 {
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
                if tree.len() <= 1 {
                    *self = match tree.min() {
                        Some(v) => Self::One(v),
                        None => Self::Empty,
                    };
                }
            }
        }
        true
    }
    pub(crate) fn replace(&mut self, old: i64, new: i64, total: usize) -> bool {
        if old == new {
            return self.contains(old);
        }
        if let Self::Tree(tree) = self
            && tree.len() > 2
        {
            return tree.replace_one(old, new);
        }
        if !self.remove(old, total) {
            return false;
        }
        self.increment(new, total - 1);
        true
    }
    pub(crate) fn len(&self) -> usize {
        match self {
            Self::Empty => 0,
            Self::One(_) => 1,
            Self::Two(_, _) => 2,
            Self::Tree(t) => t.len(),
        }
    }
    pub(crate) fn min(&self) -> Option<i64> {
        match self {
            Self::Empty => None,
            Self::One(v) => Some(*v),
            Self::Two([a, _], _) => Some(*a),
            Self::Tree(t) => t.min(),
        }
    }
    pub(crate) fn max(&self) -> Option<i64> {
        match self {
            Self::Empty => None,
            Self::One(v) => Some(*v),
            Self::Two([_, b], _) => Some(*b),
            Self::Tree(t) => t.max(),
        }
    }
}
