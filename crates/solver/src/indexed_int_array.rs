use std::{collections::VecDeque, error::Error, fmt};

/// Integer storage over a movable index window. Unwritten positions read as zero.
/// Writing a high first index does not allocate the preceding positions. Clearing
/// removes the logical window while retaining its allocation for reuse.
#[derive(Clone, Debug)]
pub struct IndexedIntArray {
    values: VecDeque<i32>,
    first: usize,
    max_size: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IndexArrayError {
    Empty,
    IndexOutOfBounds,
    AllocationFailed,
}
impl fmt::Display for IndexArrayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Empty => "Array is empty",
            Self::IndexOutOfBounds => "index is outside the configured array bounds",
            Self::AllocationFailed => "cannot allocate the requested index window",
        })
    }
}
impl Error for IndexArrayError {}
impl Default for IndexedIntArray {
    fn default() -> Self {
        Self::with_max_size(i32::MAX as usize)
    }
}
impl IndexedIntArray {
    pub fn new() -> Self {
        Self::default()
    }
    /// Exclusive upper index bound; a zero bound permits no writes.
    pub fn with_max_size(max_size: usize) -> Self {
        Self {
            values: VecDeque::new(),
            first: 0,
            max_size,
        }
    }
    pub fn first_index(&self) -> Result<usize, IndexArrayError> {
        if self.values.is_empty() {
            Err(IndexArrayError::Empty)
        } else {
            Ok(self.first)
        }
    }
    pub fn last_index(&self) -> Result<usize, IndexArrayError> {
        self.first_index()
            .map(|first| first + self.values.len() - 1)
    }
    pub fn contains_index<I: TryInto<usize>>(&self, index: I) -> bool {
        index
            .try_into()
            .ok()
            .and_then(|i| i.checked_sub(self.first))
            .is_some_and(|offset| offset < self.values.len())
    }
    pub fn get<I: TryInto<usize>>(&self, index: I) -> i32 {
        index
            .try_into()
            .ok()
            .and_then(|i| i.checked_sub(self.first))
            .and_then(|offset| self.values.get(offset))
            .copied()
            .unwrap_or(0)
    }
    /// Reject invalid indices before mutating the window. Signed inputs are
    /// supported through checked conversion, without Java exception semantics.
    pub fn set<I: TryInto<usize>>(&mut self, index: I, value: i32) -> Result<(), IndexArrayError> {
        let index = index
            .try_into()
            .map_err(|_| IndexArrayError::IndexOutOfBounds)?;
        if index >= self.max_size {
            return Err(IndexArrayError::IndexOutOfBounds);
        }
        if self.values.is_empty() {
            self.values
                .try_reserve(1)
                .map_err(|_| IndexArrayError::AllocationFailed)?;
            self.values.push_back(value);
            self.first = index;
        } else if index < self.first {
            let extra = self.first - index;
            self.values
                .try_reserve(extra)
                .map_err(|_| IndexArrayError::AllocationFailed)?;
            for _ in 0..extra {
                self.values.push_front(0);
            }
            self.first = index;
            self.values[0] = value;
        } else {
            let offset = index - self.first;
            let needed = offset + 1;
            if needed > self.values.len() {
                self.values
                    .try_reserve(needed - self.values.len())
                    .map_err(|_| IndexArrayError::AllocationFailed)?;
                self.values.resize(needed, 0);
            }
            self.values[offset] = value;
        }
        Ok(())
    }
    pub fn clear(&mut self) {
        self.values.clear();
        self.first = 0;
    }
}
