//! Iterative exact enumeration of disjoint ordered lists.

use crate::{ListAssignment, ListPosition, ListProblem, Score, Termination};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScoredListAssignment<V, S> {
    pub assignment: ListAssignment<V>,
    pub score: S,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ListSearchResult<V, S> {
    pub best: Option<ScoredListAssignment<V, S>>,
    pub evaluations: u64,
    pub termination: Termination,
}
impl<V, S> ListSearchResult<V, S> {
    pub fn is_optimal(&self) -> bool {
        self.termination == Termination::Exhausted && self.best.is_some()
    }
}

struct Frame {
    choices: Vec<Option<ListPosition>>,
    next: usize,
    applied: Option<ListPosition>,
}

fn choices<V: Eq + Clone>(
    problem: &ListProblem<V>,
    assignment: &ListAssignment<V>,
    value: &V,
) -> Frame {
    let mut choices = Vec::new();
    for (entity, meta) in problem.entities().iter().enumerate() {
        if !meta.is_pinned() && problem.accepts(entity, value) {
            for index in meta.pinned_prefix()..=assignment.lists[entity].len() {
                choices.push(Some(ListPosition { entity, index }));
            }
        }
    }
    if problem.allows_unassigned() {
        choices.push(None);
    }
    Frame {
        choices,
        next: 0,
        applied: None,
    }
}

fn record<V: Clone, S: Score>(
    result: &mut ListSearchResult<V, S>,
    assignment: &ListAssignment<V>,
    score: S,
) {
    result.evaluations += 1;
    if score.structural() >= 0 && result.best.as_ref().is_none_or(|best| score > best.score) {
        result.best = Some(ScoredListAssignment {
            assignment: assignment.clone(),
            score,
        });
    }
}

/// Maximize a deterministic score over complete ordered-list assignments.
///
/// Frozen lists and pinned prefixes retain their exact initial values. All other
/// initial values are reset, then considered in value-range order. Each value
/// is inserted in entity/index order, or left unassigned last when optional.
/// This produces each distinct assignment exactly once. Ties retain the first
/// assignment. The scoring callback must not mutate shared facts or identities.
///
/// Negative structural scores reject a candidate; hard-infeasible candidates
/// remain eligible. A zero budget performs no scoring. Exhaustion, including
/// exhaustion on the last budgeted evaluation, is required to prove optimality.
/// This factorial baseline uses an explicit stack and full recomputation; it
/// does not provide heuristic or incremental list scoring performance.
pub fn exhaustive_list_search<V, S, F>(
    problem: &ListProblem<V>,
    evaluate: F,
    max_evaluations: u64,
) -> ListSearchResult<V, S>
where
    V: Eq + Clone,
    S: Score,
    F: FnMut(&ListAssignment<V>) -> S,
{
    exhaustive_list_search_with_config(
        problem,
        evaluate,
        crate::ExactSearchConfig {
            max_evaluations,
            ..Default::default()
        },
    )
}

/// Exact enumeration with optional cooperative cancellation. See
/// [`crate::ExactSearchConfig`] for polling and termination precedence.
pub fn exhaustive_list_search_with_config<V, S, F>(
    problem: &ListProblem<V>,
    mut evaluate: F,
    config: crate::ExactSearchConfig<'_>,
) -> ListSearchResult<V, S>
where
    V: Eq + Clone,
    S: Score,
    F: FnMut(&ListAssignment<V>) -> S,
{
    let mut result = ListSearchResult {
        best: None,
        evaluations: 0,
        termination: Termination::Exhausted,
    };
    visit_list_assignments(problem, |working| {
        if let Some(reason) = config.stop_before_next(result.evaluations) {
            result.termination = reason;
            return false;
        }
        record(&mut result, working, evaluate(working));
        true
    });
    result
}

// As for basic enumeration, false stops immediately without further callbacks.
pub(crate) fn visit_list_assignments<V: Eq + Clone>(
    problem: &ListProblem<V>,
    mut visit: impl FnMut(&ListAssignment<V>) -> bool,
) -> bool {
    let mut working = ListAssignment::new(
        problem
            .entities()
            .iter()
            .map(|meta| {
                if meta.is_pinned() {
                    meta.initial().to_vec()
                } else {
                    meta.initial()[..meta.pinned_prefix()].to_vec()
                }
            })
            .collect(),
    );
    if !problem.can_complete(&working) {
        return true;
    }
    let remaining: Vec<_> = problem
        .values()
        .iter()
        .filter(|v| working.location(v).is_none())
        .cloned()
        .collect();
    if remaining.is_empty() {
        return visit(&working);
    }
    let mut stack = vec![choices(problem, &working, &remaining[0])];
    while !stack.is_empty() {
        let depth = stack.len() - 1;
        let frame = stack.last_mut().expect("nonempty enumeration stack");
        if let Some(position) = frame.applied.take() {
            working.lists[position.entity].remove(position.index);
        }
        let Some(position) = frame.choices.get(frame.next).copied() else {
            stack.pop();
            continue;
        };
        frame.next += 1;
        if let Some(position) = position {
            working.lists[position.entity].insert(position.index, remaining[depth].clone());
            frame.applied = Some(position);
        }
        if depth + 1 == remaining.len() {
            if !visit(&working) {
                return false;
            }
        } else {
            stack.push(choices(problem, &working, &remaining[depth + 1]));
        }
    }
    true
}
