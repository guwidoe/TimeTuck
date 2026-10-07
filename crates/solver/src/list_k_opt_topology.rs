//! Inspect cyclic reconnections before constructing a list move.
use crate::{ListKOptError, list_k_opt::edge};
use num_bigint::{BigInt, BigUint};
use std::ops::Range;

/// A validated degree-two reconnection of a cyclic range of absolute positions.
/// Multiple disconnected cycles are retained for inspection; only a single cycle
/// can be converted into an oriented list order. Removed/added edges preserve the
/// supplied order and endpoint direction, although validation is undirected.
/// Restoring a removed edge is allowed: this is not restricted to pure k-opt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ListKOptTopology {
    range: Range<usize>,
    removed: Vec<(usize, usize)>,
    added: Vec<(usize, usize)>,
    neighbors: Vec<Vec<usize>>,
    cycles: Vec<Vec<usize>>,
    cycle_ids: Vec<usize>,
}
impl ListKOptTopology {
    /// Reconnect the original cycle `range.start .. range.end`, including its
    /// wrap edge. Requires at least three positions and equally many removed and
    /// added edges (at least two). Invalid endpoints, duplicate edges, nonoriginal
    /// removals and final degrees other than two are rejected.
    pub fn new(
        range: Range<usize>,
        removed: &[(usize, usize)],
        added: &[(usize, usize)],
    ) -> Result<Self, ListKOptError> {
        let start = range.start;
        let end = range.end;
        let n = end.saturating_sub(start);
        if n < 3 {
            return Err(ListKOptError::TooFewValues);
        }
        if removed.len() < 2 || removed.len() != added.len() {
            return Err(ListKOptError::EdgeCount);
        }
        let check = |edges: &[(usize, usize)]| -> Result<Vec<(usize, usize)>, ListKOptError> {
            let mut result = Vec::with_capacity(edges.len());
            for &(a, b) in edges {
                if a == b || a < start || b < start || a >= end || b >= end {
                    return Err(ListKOptError::InvalidEdge);
                }
                let e = edge(a, b);
                if result.contains(&e) {
                    return Err(ListKOptError::DuplicateEdge);
                }
                result.push(e);
            }
            Ok(result)
        };
        let normalized_removed = check(removed)?;
        let normalized_added = check(added)?;
        let mut edges: Vec<_> = (start..end)
            .map(|i| edge(i, if i + 1 == end { start } else { i + 1 }))
            .collect();
        for e in &normalized_removed {
            let Some(i) = edges.iter().position(|x| x == e) else {
                return Err(ListKOptError::InvalidEdge);
            };
            edges.swap_remove(i);
        }
        for e in normalized_added {
            if edges.contains(&e) {
                return Err(ListKOptError::DuplicateEdge);
            }
            edges.push(e);
        }
        let mut neighbors = vec![Vec::new(); n];
        for (a, b) in edges {
            neighbors[a - start].push(b);
            neighbors[b - start].push(a);
        }
        if neighbors.iter().any(|v| v.len() != 2) {
            return Err(ListKOptError::Degree);
        }

        for row in &mut neighbors {
            row.sort_unstable();
        }
        let mut cycles = Vec::new();
        let mut cycle_ids = vec![usize::MAX; n];
        for first in start..end {
            if cycle_ids[first - start] != usize::MAX {
                continue;
            }
            let id = cycles.len();
            let mut cycle = vec![first];
            cycle_ids[first - start] = id;
            let mut previous = first;
            let mut current = neighbors[first - start][0];
            while current != first {
                cycle_ids[current - start] = id;
                cycle.push(current);
                let row = &neighbors[current - start];
                let next = if row[0] == previous { row[1] } else { row[0] };
                previous = current;
                current = next;
            }
            cycles.push(cycle);
        }
        Ok(Self {
            range,
            removed: removed.to_vec(),
            added: added.to_vec(),
            neighbors,
            cycles,
            cycle_ids,
        })
    }
    pub fn range(&self) -> Range<usize> {
        self.range.clone()
    }
    pub fn removed_edges(&self) -> &[(usize, usize)] {
        &self.removed
    }
    pub fn added_edges(&self) -> &[(usize, usize)] {
        &self.added
    }
    pub fn cycle_count(&self) -> usize {
        self.cycles.len()
    }
    /// Components ordered by their minimum position, each traversed from that
    /// minimum through its smaller neighbor. Labels do not depend on edge order.
    pub fn cycles(&self) -> &[Vec<usize>] {
        &self.cycles
    }
    /// Component labels for positions in `range`, with no sentinel element.
    pub fn cycle_ids(&self) -> &[usize] {
        &self.cycle_ids
    }
    pub fn cycle_id(&self, position: usize) -> Option<usize> {
        self.range
            .contains(&position)
            .then(|| self.cycle_ids[position - self.range.start])
    }
    /// Orient a connected tour from an explicit first position and its neighbor.
    /// Invalid orientation is reported before disconnectedness, matching the
    /// move-construction APIs. This leaves the descriptor unchanged.
    pub fn order_from(&self, first: usize, successor: usize) -> Result<Vec<usize>, ListKOptError> {
        if !self.range.contains(&first)
            || !self.neighbors[first - self.range.start].contains(&successor)
        {
            return Err(ListKOptError::FirstSuccessor);
        }
        if self.cycle_count() != 1 {
            return Err(ListKOptError::Disconnected);
        }
        let cycle = &self.cycles[0];
        let at = cycle
            .iter()
            .position(|&p| p == first)
            .expect("validated cycle member");
        let mut result = cycle.clone();
        result.rotate_left(at);
        if result[1] != successor {
            result[1..].reverse();
        }
        Ok(result)
    }
}

/// Exact number of pure reconnection types for `k` labeled path segments with
/// distinct endpoints. Counts single undirected cycles modulo rotation/reversal,
/// excluding every original cut edge. Zero for k < 2. Uses arbitrary precision.
/// This is an abstract topology count, not the number of legal moves for a model:
/// singleton segments, pins, entity ranges and constraints affect actual moves.
pub fn pure_k_opt_topology_count(k: usize) -> BigUint {
    if k < 2 {
        return BigUint::from(0u8);
    }
    // Inclusion-exclusion over restored cuts. Contracting m selected original
    // edges leaves r=k-m segments, with 2^(r-1)*(r-1)! oriented arrangements.
    // Restoring all k edges contributes the original cycle once.
    let mut arrangements = BigInt::from(1u8);
    for i in 1..k {
        arrangements *= 2u8;
        arrangements *= i;
    }
    let mut choose = BigInt::from(1u8);
    let mut total = BigInt::from(0u8);
    for m in 0..k {
        let term = &choose * &arrangements;
        if m.is_multiple_of(2) {
            total += term;
        } else {
            total -= term;
        }
        let r = k - m;
        if r > 1 {
            arrangements /= 2u8;
            arrangements /= r - 1;
        }
        choose *= r;
        choose /= m + 1;
    }
    if k.is_multiple_of(2) {
        total += 1u8;
    } else {
        total -= 1u8;
    }
    total.to_biguint().expect("nonnegative topology count")
}
