//! Bounded selection of a value legal for every member of a planning group.
use crate::{Assignment, FiniteProblem, ModelError, Move, ValueRange, random::SplitMix64};
use std::{error::Error, fmt, sync::Arc};

/// A shared immutable range identity. Clone this handle to share one range across
/// several members; input deduplication and equality use Arc identity, not a scan
/// of potentially enormous ranges.
pub type SharedValueRange<'a, V> = Arc<dyn ValueRange<Value = V> + 'a>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommonValueError {
    EmptyGroup,
    VariableIndex { variable: usize, count: usize },
    DuplicateVariable { variable: usize },
    PinnedVariable { variable: usize },
    ScanLimit { size: u128, limit: u128 },
    Model(ModelError),
}
impl fmt::Display for CommonValueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyGroup => f.write_str("a common-value group must not be empty"),
            Self::VariableIndex { variable, count } => {
                write!(f, "variable {variable} is outside {count} variables")
            }
            Self::DuplicateVariable { variable } => {
                write!(f, "variable {variable} occurs twice in the group")
            }
            Self::PinnedVariable { variable } => write!(f, "variable {variable} is pinned"),
            Self::ScanLimit { size, limit } => write!(
                f,
                "exact intersection needs {size} entries, exceeding scan limit {limit}"
            ),
            Self::Model(error) => error.fmt(f),
        }
    }
}
impl Error for CommonValueError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        if let Self::Model(e) = self {
            Some(e)
        } else {
            None
        }
    }
}
/// Rejection draws followed by an optional complete scan of the smallest range.
/// A scan larger than max_exact_values returns ScanLimit, never a false empty
/// intersection. Bounds limit range operations; user callbacks remain cooperative.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommonValueBudget {
    pub random_draws: u64,
    pub max_exact_values: u128,
}
impl Default for CommonValueBudget {
    fn default() -> Self {
        Self {
            random_draws: 64,
            max_exact_values: 100_000,
        }
    }
}
/// Ten draws per source occurrence, saturating instead of wrapping on huge ranges.
/// Empty ranges need no probes; all nonempty ranges produce a positive budget.
pub fn intersection_probe_budget(size: u128) -> u64 {
    u64::try_from(size.saturating_mul(10)).unwrap_or(u64::MAX)
}

/// Intersection membership over immutable range identities. Values are not
/// collected at construction. Input order is ignored by equality and duplicate
/// handles are removed; the first smallest input is the sampling source.
/// Equal contents in independently allocated ranges are distinct identities.
///
/// Nonoptional destinations are represented by Some(&value); contains(None)
/// always rejects an absent destination. V itself may carry application data.
///
/// ```
/// use std::sync::Arc;
/// use timetuck::*;
/// let a: SharedValueRange<'_, i64> = Arc::new(IntegerValueRange::new(0, 6, 1)?);
/// let b: SharedValueRange<'_, i64> = Arc::new(EnumeratedValueRange::new([3, 8]));
/// let common = CommonValueRanges::new([a, b])?;
/// assert_eq!(common.pick_exact(None, 42, 10)?, Some(3));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone)]
pub struct CommonValueRanges<'a, V: Clone + Eq> {
    ranges: Vec<SharedValueRange<'a, V>>,
    smallest: usize,
}
impl<V: Clone + Eq> fmt::Debug for CommonValueRanges<'_, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CommonValueRanges")
            .field("range_count", &self.ranges.len())
            .field("smallest_size", &self.smallest_size())
            .finish()
    }
}
impl<V: Clone + Eq> PartialEq for CommonValueRanges<'_, V> {
    fn eq(&self, other: &Self) -> bool {
        self.ranges.len() == other.ranges.len()
            && self
                .ranges
                .iter()
                .all(|a| other.ranges.iter().any(|b| Arc::ptr_eq(a, b)))
    }
}
impl<V: Clone + Eq> Eq for CommonValueRanges<'_, V> {}
impl<'a, V: Clone + Eq> CommonValueRanges<'a, V> {
    pub fn new(
        ranges: impl IntoIterator<Item = SharedValueRange<'a, V>>,
    ) -> Result<Self, CommonValueError> {
        let mut unique = Vec::new();
        for range in ranges {
            if !unique.iter().any(|r| Arc::ptr_eq(r, &range)) {
                unique.push(range);
            }
        }
        let smallest = unique
            .iter()
            .enumerate()
            .min_by_key(|(_, r)| r.len())
            .map(|(i, _)| i)
            .ok_or(CommonValueError::EmptyGroup)?;
        Ok(Self {
            ranges: unique,
            smallest,
        })
    }
    /// Fetch a solution-wide range exactly once. Member count does not cause
    /// repeated lookups; the caller has already established shared-range scope.
    pub fn from_shared_with(fetch: impl FnOnce() -> SharedValueRange<'a, V>) -> Self {
        Self {
            ranges: vec![fetch()],
            smallest: 0,
        }
    }
    pub fn range_count(&self) -> usize {
        self.ranges.len()
    }
    pub fn smallest_size(&self) -> u128 {
        self.ranges[self.smallest].len()
    }
    pub fn contains(&self, value: Option<&V>) -> bool {
        value.is_some_and(|v| self.ranges.iter().all(|r| r.contains(v)))
    }
    fn legal(&self, value: &V, excluded: Option<&V>) -> bool {
        excluded != Some(value) && self.contains(Some(value))
    }
    /// Uniform reservoir draw over legal occurrences in the smallest range.
    /// Distinct values are uniform when that source contains no duplicates.
    /// Reject an oversized scan before reading source values. None proves that
    /// no admissible occurrence exists; exclusions are applied on every path.
    pub fn pick_exact(
        &self,
        excluded: Option<&V>,
        seed: u64,
        max_values: u128,
    ) -> Result<Option<V>, CommonValueError> {
        self.exact(excluded, &mut SplitMix64::new(seed), max_values)
    }
    fn exact(
        &self,
        excluded: Option<&V>,
        random: &mut SplitMix64,
        max_values: u128,
    ) -> Result<Option<V>, CommonValueError> {
        let source = &self.ranges[self.smallest];
        let size = source.len();
        if size > max_values {
            return Err(CommonValueError::ScanLimit {
                size,
                limit: max_values,
            });
        }
        let mut chosen = None;
        let mut admitted = 0;
        for index in 0..size {
            let value = source
                .get(index)
                .expect("range must supply every in-range index");
            if self.legal(&value, excluded) {
                admitted += 1;
                if random.below_wide(admitted) == 0 {
                    chosen = Some(value);
                }
            }
        }
        Ok(chosen)
    }
    /// Bounded rejection sampling, then an exact fallback. Neither a failed
    /// probe budget nor an oversized fallback is reported as an empty intersection.
    /// Both paths weight legal occurrences of the same smallest source.
    pub fn find_target(
        &self,
        excluded: Option<&V>,
        seed: u64,
        budget: CommonValueBudget,
    ) -> Result<Option<V>, CommonValueError> {
        let source = &self.ranges[self.smallest];
        let size = source.len();
        if size == 0 {
            return Ok(None);
        }
        let mut random = SplitMix64::new(seed);
        for _ in 0..budget.random_draws.min(intersection_probe_budget(size)) {
            let value = source
                .get(random.below_wide(size))
                .expect("range must supply every in-range index");
            if self.legal(&value, excluded) {
                return Ok(Some(value));
            }
        }
        self.exact(excluded, &mut random, budget.max_exact_values)
    }
    /// Borrow each selected variable's domain. Content-equal slices in the same
    /// order share one native range handle. Missing/duplicate member IDs are errors.
    /// Optional variables still contribute their assigned-value domains only.
    pub fn for_variables(
        problem: &'a FiniteProblem<V>,
        members: &[usize],
    ) -> Result<Self, CommonValueError> {
        if members.is_empty() {
            return Err(CommonValueError::EmptyGroup);
        }
        let mut slices: Vec<&[V]> = Vec::new();
        for (at, &member) in members.iter().enumerate() {
            if members[..at].contains(&member) {
                return Err(CommonValueError::DuplicateVariable { variable: member });
            }
            let variable =
                problem
                    .variables()
                    .get(member)
                    .ok_or(CommonValueError::VariableIndex {
                        variable: member,
                        count: problem.variables().len(),
                    })?;
            if !slices.contains(&variable.domain()) {
                slices.push(variable.domain());
            }
        }
        Self::new(
            slices
                .into_iter()
                .map(|values| Arc::new(BorrowedValues(values)) as SharedValueRange<'a, V>),
        )
    }
}
struct BorrowedValues<'a, V>(&'a [V]);
impl<V: Clone + Eq> ValueRange for BorrowedValues<'_, V> {
    type Value = V;
    fn len(&self) -> u128 {
        self.0.len() as u128
    }
    fn get(&self, index: u128) -> Option<V> {
        usize::try_from(index)
            .ok()
            .and_then(|i| self.0.get(i))
            .cloned()
    }
    fn contains(&self, value: &V) -> bool {
        self.0.contains(value)
    }
}

/// Build one common assigned-destination move for an explicit group. Members can
/// start at different values, including unassigned. Whole-group no-ops are excluded
/// by excluding a shared current value; pinned members are rejected. A singleton
/// yields Change, larger groups yield Composite, preserving caller member order.
/// Invalid models/IDs and exhausted search budgets are errors. Ok(None) means no
/// changing common destination exists. Apply/score/undo use the normal native move
/// protocol; rebuild against the current snapshot at every provider step.
pub fn sample_common_change<V: Clone + Eq>(
    problem: &FiniteProblem<V>,
    assignment: &Assignment<V>,
    members: &[usize],
    seed: u64,
    budget: CommonValueBudget,
) -> Result<Option<Move<V>>, CommonValueError> {
    problem
        .validate(assignment)
        .map_err(CommonValueError::Model)?;
    let common = CommonValueRanges::for_variables(problem, members)?;
    for &variable in members {
        if problem.variables()[variable].is_pinned() {
            return Err(CommonValueError::PinnedVariable { variable });
        }
    }
    let first = assignment.values()[members[0]].as_ref();
    let excluded = if members
        .iter()
        .all(|&i| assignment.values()[i].as_ref() == first)
    {
        first
    } else {
        None
    };
    Ok(common.find_target(excluded, seed, budget)?.map(|value| {
        let moves: Vec<_> = members
            .iter()
            .map(|&variable| Move::Change {
                variable,
                value: Some(value.clone()),
            })
            .collect();
        if moves.len() == 1 {
            moves.into_iter().next().unwrap()
        } else {
            Move::Composite(moves)
        }
    }))
}
