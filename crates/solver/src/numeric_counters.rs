//! Explicit wrapping signed counters and reclaimable nonnegative occurrence slots.
use std::{collections::HashMap, error::Error, fmt};

macro_rules! signed_counter {
    ($name:ident, $integer:ty) => {
        /// Mutable by exclusive borrow, with explicit wrapping arithmetic and
        /// ordinary numeric casts. Equality and ordering compare current values.
        #[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
        pub struct $name($integer);
        impl $name {
            pub const fn new(value: $integer) -> Self {
                Self(value)
            }
            pub fn increment(&mut self) -> $integer {
                self.0 = self.0.wrapping_add(1);
                self.0
            }
            pub fn decrement(&mut self) -> $integer {
                self.0 = self.0.wrapping_sub(1);
                self.0
            }
            pub const fn as_i32(self) -> i32 {
                self.0 as i32
            }
            pub const fn as_i64(self) -> i64 {
                self.0 as i64
            }
            pub fn as_f32(self) -> f32 {
                self.0 as f32
            }
            pub fn as_f64(self) -> f64 {
                self.0 as f64
            }
        }
    };
}
signed_counter!(SignedCounter32, i32);
signed_counter!(SignedCounter64, i64);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OccurrenceCountError {
    MissingCount(u32),
    Overflow(u32),
}
impl fmt::Display for OccurrenceCountError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "occurrence count: {self:?}")
    }
}
impl Error for OccurrenceCountError {}

/// Actual allocated logical slots are retained and reused after a count reaches
/// zero. A hash index provides lookups; this does not claim linear-probe layout.
/// Keys and counts are nonnegative. Failed decrements/overflow preserve state.
/// Display follows slot order, so reclaiming a slot can change entry order.
#[derive(Clone, Debug, Default)]
pub struct OccurrenceCounts {
    slots: Vec<Option<(u32, u32)>>,
    index: HashMap<u32, usize>,
    vacant: Vec<usize>,
}
impl OccurrenceCounts {
    pub fn new() -> Self {
        Self::default()
    }
    /// Number of retained logical entry slots, not the backing Vec's capacity.
    pub fn capacity(&self) -> usize {
        self.slots.len()
    }
    pub fn get_count(&self, key: u32) -> u32 {
        self.index
            .get(&key)
            .map_or(0, |&i| self.slots[i].expect("indexed live slot").1)
    }
    pub fn increment(&mut self, key: u32) -> Result<u32, OccurrenceCountError> {
        if let Some(&i) = self.index.get(&key) {
            let entry = self.slots[i].as_mut().expect("indexed live slot");
            entry.1 = entry
                .1
                .checked_add(1)
                .ok_or(OccurrenceCountError::Overflow(key))?;
            return Ok(entry.1);
        }
        let i = if let Some(i) = self.vacant.pop() {
            self.slots[i] = Some((key, 1));
            i
        } else {
            self.slots.push(Some((key, 1)));
            self.slots.len() - 1
        };
        self.index.insert(key, i);
        Ok(1)
    }
    pub fn decrement(&mut self, key: u32) -> Result<u32, OccurrenceCountError> {
        let &i = self
            .index
            .get(&key)
            .ok_or(OccurrenceCountError::MissingCount(key))?;
        let entry = self.slots[i].as_mut().expect("indexed live slot");
        entry.1 -= 1;
        let count = entry.1;
        if count == 0 {
            self.slots[i] = None;
            self.index.remove(&key);
            self.vacant.push(i);
        }
        Ok(count)
    }
}
impl fmt::Display for OccurrenceCounts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("{")?;
        for (i, &(key, count)) in self.slots.iter().flatten().enumerate() {
            if i > 0 {
                f.write_str(", ")?;
            }
            write!(f, "{key}: {count}")?;
        }
        f.write_str("}")
    }
}
