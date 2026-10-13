//! Role-specific operations for immutable random occurrence cursors.
use std::{error::Error, fmt, iter::FusedIterator};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RandomCursorOperationError {
    RemovalUnsupported,
    RepeatingDrainUnsupported,
}
impl fmt::Display for RandomCursorOperationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::RemovalUnsupported => "random cursors cannot remove source occurrences",
            Self::RepeatingDrainUnsupported => {
                "repeating random cursors cannot drain an unbounded source"
            }
        })
    }
}
impl Error for RandomCursorOperationError {}

/// Random sampling borrows immutable sources or reads a captured snapshot.
/// Removal is rejected without consuming a draw or changing the source/cursor.
pub trait ReadOnlyRandomCursor: Iterator {
    fn try_remove(&mut self) -> Result<(), RandomCursorOperationError> {
        Err(RandomCursorOperationError::RemovalUnsupported)
    }
}
/// Draws with replacement can continue indefinitely. The explicit unbounded
/// drain operation rejects even an empty cursor, without reading it or invoking
/// its consumer. Normal Rust Iterator adapters remain available: use take(limit)
/// before collecting/for_each when consuming a repeating source.
pub trait RepeatingRandomCursor: ReadOnlyRandomCursor {
    fn try_for_each_remaining(
        &mut self,
        _consumer: impl FnMut(Self::Item),
    ) -> Result<(), RandomCursorOperationError> {
        Err(RandomCursorOperationError::RepeatingDrainUnsupported)
    }
}
/// Draws without replacement exhaust on their own. Normal Iterator::for_each
/// drains the remaining occurrences; no source-removal operation is permitted.
pub trait UniqueRandomCursor: ReadOnlyRandomCursor {}

// Factories construct these zero-bookkeeping adapters over their real samplers.
// Keeping constructors private prevents accidentally labeling an arbitrary
// caller iterator as a supplied native sampling role.
#[repr(transparent)]
pub(crate) struct RepeatingCursor<I>(pub(crate) I);
#[repr(transparent)]
pub(crate) struct UniqueCursor<I>(pub(crate) I);
macro_rules! delegate_iterator {
    ($wrapper:ident) => {
        impl<I: Iterator> Iterator for $wrapper<I> {
            type Item = I::Item;
            #[inline]
            fn next(&mut self) -> Option<Self::Item> {
                self.0.next()
            }
            #[inline]
            fn size_hint(&self) -> (usize, Option<usize>) {
                self.0.size_hint()
            }
            #[inline]
            fn nth(&mut self, n: usize) -> Option<Self::Item> {
                self.0.nth(n)
            }
            #[inline]
            fn fold<B, F: FnMut(B, Self::Item) -> B>(self, init: B, f: F) -> B {
                self.0.fold(init, f)
            }
        }
        impl<I: FusedIterator> FusedIterator for $wrapper<I> {}
        impl<I: Iterator> ReadOnlyRandomCursor for $wrapper<I> {}
    };
}
delegate_iterator!(RepeatingCursor);
delegate_iterator!(UniqueCursor);
impl<I: Iterator> RepeatingRandomCursor for RepeatingCursor<I> {}
impl<I: Iterator> UniqueRandomCursor for UniqueCursor<I> {}

impl<D: FnMut(u128) -> u128> ReadOnlyRandomCursor for crate::UniqueIndices<D> {}
impl<D: FnMut(u128) -> u128> UniqueRandomCursor for crate::UniqueIndices<D> {}
impl<T, D: FnMut(u128) -> u128> ReadOnlyRandomCursor for crate::UniqueSliceSamples<'_, T, D> {}
impl<T, D: FnMut(u128) -> u128> UniqueRandomCursor for crate::UniqueSliceSamples<'_, T, D> {}
impl<R: crate::ValueRange + ?Sized, D: FnMut(u128) -> u128> ReadOnlyRandomCursor
    for crate::UniqueRangeSamples<'_, R, D>
{
}
impl<R: crate::ValueRange + ?Sized, D: FnMut(u128) -> u128> UniqueRandomCursor
    for crate::UniqueRangeSamples<'_, R, D>
{
}
