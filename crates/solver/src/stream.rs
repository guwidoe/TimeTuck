//! Typed, eager constraint streams over a snapshot of explicitly supplied facts.
//!
//! Rows form a multiset: duplicates remain until `distinct` is requested. Each
//! operation preserves encounter order, including hash-indexed joins and groups.
//! These streams can run inside a scoped constraint callback; affected callbacks
//! rebuild their streams. This module does not implement incremental tuple graphs.
//!
//! ```
//! use timetuck::{ConstraintStream, HardSoftScore};
//! let jobs = ConstraintStream::new([(0, 3), (0, 4), (1, 2)]);
//! let loads = jobs.group_collect(|job| job.0, |job| job.1);
//! let score = loads.score(|(_, load)| {
//!     HardSoftScore::new(-(load.sum - 5).max(0), 0)
//! }).unwrap();
//! assert_eq!(score, HardSoftScore::new(-2, 0));
//! ```

use crate::int_collector_counts::IntCollectorCounts;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::hash::Hash;

use crate::AdditiveScore;

/// An integer multiset's collected values. Empty extrema are `None`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IntSummary {
    pub count: usize,
    pub distinct_count: usize,
    /// Wrapping signed 64-bit sum, independent of insertion/retraction order.
    pub sum: i64,
    pub min: Option<i64>,
    pub max: Option<i64>,
}

/// A retractable integer collector. Each removal retracts one occurrence.
///
/// Values are captured at insertion; changing a source fact requires removing
/// its old mapped value and inserting its new one. Counts are native `usize`.
/// Insertion panics before modifying state if the total count would overflow.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IntCollector {
    values: IntCollectorCounts,
    count: usize,
    sum: i64,
}

impl IntCollector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, value: i64) {
        let count = self.count.checked_add(1).expect("collector count overflow");
        self.values.increment(value);
        self.count = count;
        self.sum = self.sum.wrapping_add(value);
    }

    /// Returns false without modifying state if this value is absent.
    pub fn remove(&mut self, value: i64) -> bool {
        if !self.values.remove(value) {
            return false;
        }
        self.count -= 1;
        self.sum = self.sum.wrapping_sub(value);
        true
    }

    pub(crate) fn replace_captured(&mut self, old: i64, new: i64) -> bool {
        if !self.values.contains(old) {
            return false;
        }
        // Preserve the remove-then-insert count preflight before changing any
        // frequencies. Both arithmetic operations are primitive and cannot call
        // user code; a present capture guarantees a nonzero total count.
        let count = self.count - 1;
        let count = count.checked_add(1).expect("collector count overflow");
        if !self.values.replace(old, new) {
            return false;
        }
        self.count = count;
        self.sum = self.sum.wrapping_sub(old).wrapping_add(new);
        true
    }

    // Internal guards need the occurrence count, without computing extrema.
    #[inline]
    pub(crate) fn occurrence_count(&self) -> usize {
        self.count
    }

    pub fn summary(&self) -> IntSummary {
        IntSummary {
            count: self.count,
            distinct_count: self.values.len(),
            sum: self.sum,
            min: self.values.min(),
            max: self.values.max(),
        }
    }
}

impl FromIterator<i64> for IntCollector {
    fn from_iter<I: IntoIterator<Item = i64>>(iter: I) -> Self {
        let mut collector = Self::new();
        for value in iter {
            collector.insert(value);
        }
        collector
    }
}

/// A snapshot relation, with one score match per row, including duplicates.
///
/// Facts, IDs, references and tuples are all valid row types. Include stable IDs
/// when equality must distinguish otherwise identical domain objects. Optional
/// planning values are ordinary Rust values here: callers explicitly choose
/// whether unassigned facts enter the stream. No implicit assignment filtering.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConstraintStream<T> {
    rows: Vec<T>,
}

impl<T> ConstraintStream<T> {
    pub fn new(rows: impl IntoIterator<Item = T>) -> Self {
        Self {
            rows: rows.into_iter().collect(),
        }
    }

    pub fn as_slice(&self) -> &[T] {
        &self.rows
    }

    pub fn into_vec(self) -> Vec<T> {
        self.rows
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub fn filter(self, predicate: impl FnMut(&T) -> bool) -> Self {
        Self::new(self.rows.into_iter().filter(predicate))
    }

    pub fn map<U>(self, mapping: impl FnMut(T) -> U) -> ConstraintStream<U> {
        ConstraintStream::new(self.rows.into_iter().map(mapping))
    }

    pub fn flat_map<U, I: IntoIterator<Item = U>>(
        self,
        mapping: impl FnMut(T) -> I,
    ) -> ConstraintStream<U> {
        ConstraintStream::new(self.rows.into_iter().flat_map(mapping))
    }

    pub fn concat(self, other: Self) -> Self {
        Self::new(self.rows.into_iter().chain(other.rows))
    }

    /// Emits the first occurrence of each equal row.
    pub fn distinct(self) -> Self
    where
        T: Clone + Eq + Hash,
    {
        let mut seen = HashSet::new();
        self.filter(|row| seen.insert(row.clone()))
    }

    /// Nested-loop join, with left encounter order then right encounter order.
    /// Matching a row with itself is allowed. Duplicate rows multiply matches.
    pub fn join<U: Clone>(
        self,
        other: &ConstraintStream<U>,
        mut predicate: impl FnMut(&T, &U) -> bool,
    ) -> ConstraintStream<(T, U)>
    where
        T: Clone,
    {
        let mut rows = Vec::new();
        for left in self.rows {
            for right in &other.rows {
                if predicate(&left, right) {
                    rows.push((left.clone(), right.clone()));
                }
            }
        }
        ConstraintStream::new(rows)
    }

    /// Equality join using a right-side hash index. Key equality must be an
    /// equivalence relation; tuple keys express several equality conditions.
    pub fn join_equal<U: Clone, K: Eq + Hash>(
        self,
        other: &ConstraintStream<U>,
        mut left_key: impl FnMut(&T) -> K,
        mut right_key: impl FnMut(&U) -> K,
    ) -> ConstraintStream<(T, U)>
    where
        T: Clone,
    {
        let mut index: HashMap<K, Vec<&U>> = HashMap::new();
        for right in &other.rows {
            index.entry(right_key(right)).or_default().push(right);
        }
        let mut rows = Vec::new();
        for left in self.rows {
            if let Some(matches) = index.get(&left_key(&left)) {
                for &right in matches {
                    rows.push((left.clone(), right.clone()));
                }
            }
        }
        ConstraintStream::new(rows)
    }

    /// Keeps each left occurrence once if any right row matches. Self matches
    /// count; exclude them explicitly using stable IDs when required.
    pub fn if_exists<U>(
        self,
        other: &ConstraintStream<U>,
        mut predicate: impl FnMut(&T, &U) -> bool,
    ) -> Self {
        self.filter(|left| other.rows.iter().any(|right| predicate(left, right)))
    }

    pub fn if_not_exists<U>(
        self,
        other: &ConstraintStream<U>,
        mut predicate: impl FnMut(&T, &U) -> bool,
    ) -> Self {
        self.filter(|left| !other.rows.iter().any(|right| predicate(left, right)))
    }

    /// One pair for each pair of distinct input positions, in ascending position
    /// order. Equal values at different positions still form a pair.
    pub fn unique_pairs(self) -> ConstraintStream<(T, T)>
    where
        T: Clone,
    {
        let mut pairs = Vec::new();
        for (i, left) in self.rows.iter().enumerate() {
            for right in &self.rows[i + 1..] {
                pairs.push((left.clone(), right.clone()));
            }
        }
        ConstraintStream::new(pairs)
    }

    /// Emits one group for each observed key, in first-encounter order.
    /// Empty input yields no groups. Row order within each group is preserved.
    pub fn group_by<K: Clone + Eq + Hash>(
        self,
        mut key: impl FnMut(&T) -> K,
    ) -> ConstraintStream<(K, Vec<T>)> {
        let mut positions = HashMap::new();
        let mut groups: Vec<(K, Vec<T>)> = Vec::new();
        for row in self.rows {
            let key = key(&row);
            let position = *positions.entry(key.clone()).or_insert_with(|| {
                let position = groups.len();
                groups.push((key, Vec::new()));
                position
            });
            groups[position].1.push(row);
        }
        ConstraintStream::new(groups)
    }

    /// Collects integer values separately for each observed key. Mapping is
    /// evaluated once per row in input order; no phantom empty group is emitted.
    pub fn group_collect<K: Clone + Eq + Hash>(
        self,
        mut key: impl FnMut(&T) -> K,
        mut value: impl FnMut(&T) -> i64,
    ) -> ConstraintStream<(K, IntSummary)> {
        let mut positions = HashMap::new();
        let mut groups: Vec<(K, IntCollector)> = Vec::new();
        for row in self.rows {
            let key = key(&row);
            let position = *positions.entry(key.clone()).or_insert_with(|| {
                let position = groups.len();
                groups.push((key, IntCollector::new()));
                position
            });
            groups[position].1.insert(value(&row));
        }
        ConstraintStream::new(
            groups
                .into_iter()
                .map(|(key, group)| (key, group.summary())),
        )
    }

    /// Collects all rows, including an empty stream's zero-valued summary.
    pub fn collect_int(&self, mapping: impl FnMut(&T) -> i64) -> IntSummary {
        self.rows
            .iter()
            .map(mapping)
            .collect::<IntCollector>()
            .summary()
    }

    /// Adds one ordinary score contribution per row. Positive values reward;
    /// negative values penalize. Structural state belongs to the planning model
    /// and is rejected before arithmetic could silently discard it.
    pub fn score<S: AdditiveScore>(
        &self,
        mut contribution: impl FnMut(&T) -> S,
    ) -> Result<S, StreamScoreError> {
        let mut score = S::ZERO;
        for (row, fact) in self.rows.iter().enumerate() {
            let part = contribution(fact);
            if part.structural() != 0 {
                return Err(StreamScoreError { row });
            }
            score = score + part;
        }
        Ok(score)
    }
}

impl<T> FromIterator<T> for ConstraintStream<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        Self::new(iter)
    }
}

impl<T> IntoIterator for ConstraintStream<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.rows.into_iter()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StreamScoreError {
    /// Zero-based input row with a nonzero structural contribution.
    pub row: usize,
}

impl fmt::Display for StreamScoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "stream row {} has a structural score contribution",
            self.row
        )
    }
}

impl std::error::Error for StreamScoreError {}

#[cfg(test)]
mod group_inverse_count_tests;

#[cfg(test)]
mod owned_collector_box_tests;
