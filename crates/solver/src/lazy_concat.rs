//! Indexed candidate groups materialized only when accessed.
use crate::unique_indices;
use std::{cell::OnceCell, error::Error, fmt, iter::FusedIterator};
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LazyConcatError {
    LengthOverflow,
    Index {
        index: usize,
        length: usize,
    },
    SupplierLength {
        group: usize,
        expected: usize,
        actual: usize,
    },
}
impl fmt::Display for LazyConcatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LengthOverflow => f.write_str("concatenated candidate length overflow"),
            Self::Index { index, length } => write!(f, "Index: {index}, Size: {length}"),
            Self::SupplierLength {
                group,
                expected,
                actual,
            } => write!(
                f,
                "candidate group {group} supplied {actual} items, expected {expected}"
            ),
        }
    }
}
impl Error for LazyConcatError {}
enum Contents<'a, T> {
    Borrowed(&'a [T]),
    Owned(Vec<T>),
    Deferred {
        supplier: Box<dyn Fn() -> Vec<T> + 'a>,
        cached: OnceCell<Result<Vec<T>, LazyConcatError>>,
    },
}
struct Group<'a, T> {
    end: usize,
    contents: Contents<'a, T>,
}
/// Concatenation of immutable candidate groups with known lengths.
///
/// Length checks, construction and iterator creation never invoke suppliers.
/// Access materializes only the selected group and caches its contents; elements
/// can be borrowed without Clone. Equal elements at different positions remain
/// separate occurrences. Empty groups are omitted and their suppliers never run.
/// Lookup uses binary search over cumulative group lengths.
///
/// A supplier must return its advertised number of items. A mismatch is cached
/// as an error and cannot shift later indices. If a supplier panics, its cache
/// remains empty and a subsequent access retries it; external callback effects
/// are not rolled back. Recursive access to the same deferred group is unsupported.
/// This collection uses single-threaded interior caching and is not Sync.
/// Append requires exclusive access; existing borrowed items/iterators therefore
/// cannot be invalidated by appending. Total-length overflow leaves it unchanged.
///
/// ```
/// use timetuck::LazyConcat;
/// let mut candidates=LazyConcat::new();
/// candidates.push(vec![10,20])?;
/// candidates.push_deferred(2, || vec![30,40])?;
/// assert_eq!(candidates.len(),4);
/// assert_eq!(*candidates.get(2)?,30);
/// let mut drawn=candidates.samples(42).collect::<Result<Vec<_>,_>>()?;
/// drawn.sort();
/// assert_eq!(drawn,vec![&10,&20,&30,&40]);
/// # Ok::<(), timetuck::LazyConcatError>(())
/// ```
pub struct LazyConcat<'a, T> {
    groups: Vec<Group<'a, T>>,
    length: usize,
}
impl<T> Default for LazyConcat<'_, T> {
    fn default() -> Self {
        Self {
            groups: vec![],
            length: 0,
        }
    }
}
impl<'a, T> LazyConcat<'a, T> {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn len(&self) -> usize {
        self.length
    }
    pub fn is_empty(&self) -> bool {
        self.length == 0
    }
    fn append(&mut self, length: usize, contents: Contents<'a, T>) -> Result<(), LazyConcatError> {
        let end = self
            .length
            .checked_add(length)
            .ok_or(LazyConcatError::LengthOverflow)?;
        if length != 0 {
            self.groups.push(Group { end, contents });
        }
        self.length = end;
        Ok(())
    }
    pub fn push(&mut self, values: Vec<T>) -> Result<(), LazyConcatError> {
        self.append(values.len(), Contents::Owned(values))
    }
    pub fn push_slice(&mut self, values: &'a [T]) -> Result<(), LazyConcatError> {
        self.append(values.len(), Contents::Borrowed(values))
    }
    pub fn push_deferred(
        &mut self,
        length: usize,
        supplier: impl Fn() -> Vec<T> + 'a,
    ) -> Result<(), LazyConcatError> {
        self.append(
            length,
            Contents::Deferred {
                supplier: Box::new(supplier),
                cached: OnceCell::new(),
            },
        )
    }
    pub fn get(&self, index: usize) -> Result<&T, LazyConcatError> {
        if index >= self.length {
            return Err(LazyConcatError::Index {
                index,
                length: self.length,
            });
        }
        let group = self.groups.partition_point(|g| g.end <= index);
        let start = if group == 0 {
            0
        } else {
            self.groups[group - 1].end
        };
        let selected = &self.groups[group];
        let values: &[T] = match &selected.contents {
            Contents::Borrowed(values) => values,
            Contents::Owned(values) => values,
            Contents::Deferred { supplier, cached } => {
                let result = cached.get_or_init(|| {
                    let values = supplier();
                    let expected = selected.end - start;
                    if values.len() != expected {
                        Err(LazyConcatError::SupplierLength {
                            group,
                            expected,
                            actual: values.len(),
                        })
                    } else {
                        Ok(values)
                    }
                });
                result.as_ref().map_err(Clone::clone)?
            }
        };
        Ok(&values[index - start])
    }
    pub fn iter(
        &self,
    ) -> impl DoubleEndedIterator<Item = Result<&T, LazyConcatError>>
    + ExactSizeIterator
    + FusedIterator
    + '_ {
        (0..self.length).map(|i| self.get(i))
    }
    /// Seeded, lazy sampling without replacement over indexed occurrences. Only
    /// groups containing selected occurrences load. Allocation grows with draws,
    /// not total candidate count. Supplier errors are emitted for their selected
    /// indices; other groups remain accessible. No Java PRNG trace parity.
    pub fn samples(
        &self,
        seed: u64,
    ) -> impl FusedIterator<Item = Result<&T, LazyConcatError>> + '_ {
        unique_indices(self.length as u128, seed)
            .map(|index| self.get(index.expect("seeded draws stay within their bound") as usize))
    }
}
