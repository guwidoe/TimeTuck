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

fn choices<V: Eq + Clone>(problem: &ListProblem<V>, assignment: &ListAssignment<V>) -> Frame {
    let mut choices = Vec::new();
    for (entity, meta) in problem.entities().iter().enumerate() {
        if !meta.is_pinned() {
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
    mut evaluate: F,
    max_evaluations: u64,
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
    let remaining: Vec<_> = problem
        .values()
        .iter()
        .filter(|v| working.location(v).is_none())
        .cloned()
        .collect();
    if remaining.is_empty() {
        if max_evaluations == 0 {
            result.termination = Termination::EvaluationLimit;
        } else {
            record(&mut result, &working, evaluate(&working));
        }
        return result;
    }
    let mut stack = vec![choices(problem, &working)];
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
            if result.evaluations == max_evaluations {
                result.termination = Termination::EvaluationLimit;
                return result;
            }
            record(&mut result, &working, evaluate(&working));
        } else {
            stack.push(choices(problem, &working));
        }
    }
    result
}
