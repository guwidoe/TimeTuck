//! Native model size and managed-job telemetry, independent of score calculation.
use crate::{FiniteProblem, ListProblem, MixedProblem, SolverWork};
use num_bigint::BigUint;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

/// Number of structurally complete assignments before scoring constraints.
/// Restricted list ranges and dependency-constrained lists use a labelled upper bound.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssignmentSpace {
    pub count: BigUint,
    pub exact: bool,
}
impl AssignmentSpace {
    /// Full decimal representation, without locale-dependent rounding or grouping.
    pub fn decimal(&self) -> String {
        self.count.to_string()
    }
}
/// Counts describe native model slots, including pinned ones. A basic slot is one
/// entity/variable; a list owner is one entity/list variable. Value identities are
/// deduplicated across basic domains; mixed components have distinct namespaces.
/// Empty finite models have no separate fact registry and therefore zero values.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProblemSizeStatistics {
    pub entity_count: usize,
    pub variable_count: usize,
    pub value_count: usize,
    pub assignment_space: AssignmentSpace,
}
/// Floating logarithm of a count, or of a labelled upper bound when list ranges
/// or dependencies restrict it. This metadata does not prove score feasibility.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AssignmentScale {
    pub scale: crate::ProblemScale,
    pub upper_bound: bool,
}
/// Inspect model metadata without scoring or enumerating assignments.
pub trait ProblemStatistics {
    fn problem_size_statistics(&self) -> ProblemSizeStatistics;
    /// Logarithmic assignment count. Native models override this to avoid building
    /// an arbitrary-precision integer. Zero possibilities use negative infinity.
    fn assignment_scale(&self) -> AssignmentScale {
        let space = self.problem_size_statistics().assignment_space;
        AssignmentScale {
            scale: crate::ProblemScale::from_count(&space.count),
            upper_bound: !space.exact,
        }
    }
}
impl<V: Eq> ProblemStatistics for FiniteProblem<V> {
    fn assignment_scale(&self) -> AssignmentScale {
        let mut sum = crate::combinatorics::LogSum::default();
        for variable in self.variables() {
            let choices = if variable.is_pinned() {
                usize::from(variable.initial().is_some() || variable.allows_unassigned()) as f64
            } else {
                variable.domain().len() as f64 + f64::from(variable.allows_unassigned())
            };
            if choices == 0.0 {
                return AssignmentScale {
                    scale: crate::ProblemScale::from_log10(f64::NEG_INFINITY),
                    upper_bound: false,
                };
            }
            sum.add(choices.ln());
        }
        AssignmentScale {
            scale: crate::ProblemScale::from_log10(sum.value() * std::f64::consts::LOG10_E),
            upper_bound: false,
        }
    }
    fn problem_size_statistics(&self) -> ProblemSizeStatistics {
        let mut values = vec![];
        let mut count = BigUint::from(1u8);
        for variable in self.variables() {
            for value in variable.domain() {
                if !values.contains(&value) {
                    values.push(value);
                }
            }
            let choices = if variable.is_pinned() {
                BigUint::from(u8::from(
                    variable.initial().is_some() || variable.allows_unassigned(),
                ))
            } else {
                BigUint::from(variable.domain().len())
                    + BigUint::from(u8::from(variable.allows_unassigned()))
            };
            count *= choices;
        }
        ProblemSizeStatistics {
            entity_count: self.variables().len(),
            variable_count: self.variables().len(),
            value_count: values.len(),
            assignment_space: AssignmentSpace { count, exact: true },
        }
    }
}
fn list_dimensions<V: Eq + Clone>(problem: &ListProblem<V>) -> (usize, usize, bool) {
    let fixed: Vec<_> = problem
        .entities()
        .iter()
        .flat_map(|e| {
            let length = if e.is_pinned() {
                e.initial().len()
            } else {
                e.pinned_prefix()
            };
            e.initial()[..length].iter()
        })
        .collect();
    let free: Vec<_> = problem
        .values()
        .iter()
        .filter(|v| !fixed.contains(v))
        .collect();
    let owners: Vec<_> = problem
        .entities()
        .iter()
        .filter(|e| !e.is_pinned())
        .collect();
    let restricted = owners.iter().any(|e| {
        e.value_range()
            .is_some_and(|r| free.iter().any(|v| !r.contains(v)))
    });
    (free.len(), owners.len(), restricted)
}
impl<V: Eq + Clone> ProblemStatistics for ListProblem<V> {
    fn assignment_scale(&self) -> AssignmentScale {
        let (free, owners, restricted) = list_dimensions(self);
        AssignmentScale {
            scale: crate::ProblemScale::from_log10(
                crate::combinatorics::list_arrangements_ln(free, owners, self.allows_unassigned())
                    * std::f64::consts::LOG10_E,
            ),
            upper_bound: restricted || self.dependencies.is_some(),
        }
    }
    fn problem_size_statistics(&self) -> ProblemSizeStatistics {
        let (free, owners, restricted) = list_dimensions(self);
        // k selected values can be ordered and split among m labelled suffixes
        // in m*(m+1)*...*(m+k-1) ways. Optional values sum over every subset size.
        let mut arrangements = BigUint::from(1u8);
        let mut choose = BigUint::from(1u8);
        let mut count = BigUint::from(1u8);
        for k in 1..=free {
            arrangements *= BigUint::from(owners) + BigUint::from(k - 1);
            if self.allows_unassigned() {
                choose *= BigUint::from(free - k + 1);
                choose /= BigUint::from(k);
                count += &choose * &arrangements;
            }
        }
        if !self.allows_unassigned() {
            count = arrangements;
        }
        ProblemSizeStatistics {
            entity_count: self.entities().len(),
            variable_count: self.entities().len(),
            value_count: self.values().len(),
            assignment_space: AssignmentSpace {
                count,
                exact: !restricted && self.dependencies.is_none(),
            },
        }
    }
}
impl<B: Eq + Clone, L: Eq + Clone> ProblemStatistics for MixedProblem<B, L> {
    fn assignment_scale(&self) -> AssignmentScale {
        let basic = self.basic().assignment_scale();
        let lists = self.lists().assignment_scale();
        AssignmentScale {
            scale: crate::ProblemScale::from_log10(basic.scale.log10() + lists.scale.log10()),
            upper_bound: basic.upper_bound || lists.upper_bound,
        }
    }
    fn problem_size_statistics(&self) -> ProblemSizeStatistics {
        let a = self.basic().problem_size_statistics();
        let b = self.lists().problem_size_statistics();
        ProblemSizeStatistics {
            entity_count: a.entity_count + b.entity_count,
            variable_count: a.variable_count + b.variable_count,
            value_count: a.value_count + b.value_count,
            assignment_space: AssignmentSpace {
                count: a.assignment_space.count * b.assignment_space.count,
                exact: a.assignment_space.exact && b.assignment_space.exact,
            },
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RevisionProblemSize {
    pub revision: u64,
    pub size: Arc<ProblemSizeStatistics>,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SolverJobStatistics {
    /// None means no work counters were reported, not zero work.
    pub work: Option<SolverWork>,
    /// Counters saturate on overflow; this flag prevents claiming exact totals.
    pub work_overflowed: bool,
    /// True while measured solver work is active; false while queued, idle or delivering.
    pub solving: bool,
    pub solving_duration: Duration,
    pub problem_size: Option<RevisionProblemSize>,
}
impl SolverJobStatistics {
    fn speed(&self, count: u64) -> u64 {
        let nanos = self.solving_duration.as_nanos();
        if nanos == 0 {
            return 0;
        }
        ((u128::from(count) * 1_000_000_000) / nanos).min(u128::from(u64::MAX)) as u64
    }
    pub fn score_calculation_speed(&self) -> Option<u64> {
        self.work.map(|w| self.speed(w.evaluations))
    }
    pub fn move_evaluation_speed(&self) -> Option<u64> {
        self.work.map(|w| self.speed(w.move_evaluations))
    }
}
pub(crate) enum StatisticsUpdate {
    Work { work: SolverWork, add: bool },
    Size(RevisionProblemSize),
    Timing(bool),
}
pub(crate) struct StatisticsState {
    snapshot: SolverJobStatistics,
    since: Option<Instant>,
    closed: bool,
}
impl StatisticsState {
    pub(crate) fn new(size: Option<ProblemSizeStatistics>) -> Self {
        Self {
            snapshot: SolverJobStatistics {
                problem_size: size.map(|size| RevisionProblemSize {
                    revision: 0,
                    size: Arc::new(size),
                }),
                ..Default::default()
            },
            since: None,
            closed: false,
        }
    }
    pub(crate) fn start(&mut self) {
        if !self.closed && self.since.is_none() {
            self.since = Some(Instant::now());
        }
    }
    fn pause(&mut self) {
        if let Some(since) = self.since.take() {
            self.snapshot.solving_duration = self
                .snapshot
                .solving_duration
                .saturating_add(since.elapsed());
        }
    }
    pub(crate) fn finish(&mut self) {
        self.pause();
        self.closed = true;
    }
    pub(crate) fn read(&self) -> SolverJobStatistics {
        let mut out = self.snapshot.clone();
        out.solving = self.since.is_some();
        if let Some(since) = self.since {
            out.solving_duration = out.solving_duration.saturating_add(since.elapsed());
        }
        out
    }
    pub(crate) fn update(&mut self, update: StatisticsUpdate) -> bool {
        if self.closed {
            return false;
        }
        match update {
            StatisticsUpdate::Timing(running) => {
                if running {
                    self.start();
                } else {
                    self.pause();
                }
            }
            StatisticsUpdate::Size(size) => {
                if self
                    .snapshot
                    .problem_size
                    .as_ref()
                    .is_some_and(|old| size.revision < old.revision)
                {
                    return false;
                }
                self.snapshot.problem_size = Some(size);
            }
            StatisticsUpdate::Work { work, add } => {
                let old = self.snapshot.work.unwrap_or_default();
                if add {
                    let mut total = old;
                    macro_rules! sum { ($($field:ident),*) => { $(
                        total.$field = match old.$field.checked_add(work.$field) {
                            Some(n) => n, None => { self.snapshot.work_overflowed = true; old.$field.saturating_add(work.$field) }
                        };
                    )* }; }
                    sum!(
                        evaluations,
                        move_evaluations,
                        constraint_evaluations,
                        completed_steps,
                        construction_steps,
                        accepted_moves,
                        repair_attempts,
                        fine_move_attempts,
                        complete_reconstructions
                    );
                    self.snapshot.work = Some(total);
                } else {
                    if work.evaluations < old.evaluations
                        || work.move_evaluations < old.move_evaluations
                        || work.constraint_evaluations < old.constraint_evaluations
                        || work.completed_steps < old.completed_steps
                        || work.construction_steps < old.construction_steps
                        || work.accepted_moves < old.accepted_moves
                        || work.repair_attempts < old.repair_attempts
                        || work.fine_move_attempts < old.fine_move_attempts
                        || work.complete_reconstructions < old.complete_reconstructions
                    {
                        return false;
                    }
                    self.snapshot.work = Some(work);
                }
            }
        }
        true
    }
}
