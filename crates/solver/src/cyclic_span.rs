//! Allocation-free circular reversal, shared by directed list neighborhoods.
use std::{error::Error, fmt};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CyclicSpanError {
    pub start: usize,
    pub end: usize,
    pub len: usize,
}
impl fmt::Display for CyclicSpanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "circular boundaries ({}, {}) exceed length {}",
            self.start, self.end, self.len
        )
    }
}
impl Error for CyclicSpanError {}

/// Reverse the half-open circular span from `start` to `end`, preserving all
/// other positions. Descending boundaries wrap through the end of the slice.
/// Both boundaries may equal the slice length; equal boundaries mean empty.
/// Invalid boundaries leave the slice unchanged. No Clone bound or allocation.
/// Unlike a complete list-move plan this primitive does not rotate a tour anchor.
pub fn reverse_cyclic_span<T>(
    values: &mut [T],
    start: usize,
    end: usize,
) -> Result<(), CyclicSpanError> {
    let len = values.len();
    if start > len || end > len {
        return Err(CyclicSpanError { start, end, len });
    }
    let count = if start <= end {
        end - start
    } else {
        len - start + end
    };
    let position = |offset| {
        if offset < len - start {
            start + offset
        } else {
            offset - (len - start)
        }
    };
    for i in 0..count / 2 {
        values.swap(position(i), position(count - 1 - i));
    }
    Ok(())
}
