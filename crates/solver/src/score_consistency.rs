//! Detached evidence from one complete score recomputation.
//!
//! ```
//! use std::cell::Cell;
//! use timetuck::*;
//! let factor = Cell::new(1_i64);
//! let p = FiniteProblem::new(vec![Variable::new(vec![2_i64]).with_initial(2)])?;
//! let mut n = ConstraintNetwork::new(1);
//! n.add_constraint("cost", vec![0], |v| SimpleScore::new(*v[0].unwrap() * factor.get()))?;
//! let d = ScoreDirector::new(&p, &n, p.initial_assignment())?;
//! factor.set(3); // Deliberately violate the immutable-fact contract.
//! let report = d.consistency_report()?;
//! assert_eq!(report.working_score, SimpleScore::new(2));
//! assert_eq!(report.recomputed_score, SimpleScore::new(6));
//! assert_eq!(report.constraints[0].name, "cost");
//! assert_eq!(d.score(), SimpleScore::new(2)); // Diagnostics never repair the cache.
//! # Ok::<(),Box<dyn std::error::Error>>(())
//! ```
use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConstraintScoreDifference<S> {
    pub constraint: usize,
    pub name: String,
    pub working_score: S,
    pub recomputed_score: S,
}
/// One fresh evaluation of every scoped constraint. Differences remain visible
/// even when they cancel in the total. Score values retain their original type;
/// equality never depends on their Debug representation. This report does not
/// repair caches or establish that an application's objective is correct.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScoreConsistencyReport<S> {
    pub working_score: S,
    pub recomputed_score: S,
    pub constraints: Vec<ConstraintScoreDifference<S>>,
}
impl<S: Eq> ScoreConsistencyReport<S> {
    pub fn is_consistent(&self) -> bool {
        self.working_score == self.recomputed_score && self.constraints.is_empty()
    }
}
impl<S: fmt::Debug> ScoreConsistencyReport<S> {
    /// Detach formatted evidence for nongeneric error types. Typed equality must
    /// be checked first; two unequal custom scores may share a Debug rendering.
    pub fn diagnostic(&self) -> ScoreCorruption {
        ScoreCorruption {
            working_score: format!("{:?}", self.working_score),
            recomputed_score: format!("{:?}", self.recomputed_score),
            constraints: self
                .constraints
                .iter()
                .map(|c| ConstraintScoreDifference {
                    constraint: c.constraint,
                    name: c.name.clone(),
                    working_score: format!("{:?}", c.working_score),
                    recomputed_score: format!("{:?}", c.recomputed_score),
                })
                .collect(),
        }
    }
}
/// Exact observations captured by a failing score check. Display shows at most
/// five constraint differences; the structured data retains every difference.
/// Fresh values can themselves be wrong if callbacks are impure or mis-specified.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScoreCorruption {
    pub working_score: String,
    pub recomputed_score: String,
    pub constraints: Vec<ConstraintScoreDifference<String>>,
}
impl fmt::Display for ScoreCorruption {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Score corruption: working score {}; recomputed score {}",
            self.working_score, self.recomputed_score
        )?;
        for c in self.constraints.iter().take(5) {
            write!(
                f,
                "; constraint {} ({:?}): cached {}, recomputed {}",
                c.constraint, c.name, c.working_score, c.recomputed_score
            )?;
        }
        if self.constraints.len() > 5 {
            write!(
                f,
                "; {} more constraint differences",
                self.constraints.len() - 5
            )?;
        }
        Ok(())
    }
}
impl std::error::Error for ScoreCorruption {}

pub(crate) fn compare<'a, S: Eq + Clone + 'a, E>(
    working_score: &S,
    cached: impl Iterator<Item = (&'a str, &'a S)>,
    mut recompute: impl FnMut(usize) -> Result<S, E>,
    total: impl FnOnce(&[S]) -> Result<S, E>,
) -> Result<ScoreConsistencyReport<S>, E> {
    let mut fresh = Vec::new();
    let mut constraints = Vec::new();
    for (constraint, (name, working)) in cached.enumerate() {
        let actual = recompute(constraint)?;
        if working != &actual {
            constraints.push(ConstraintScoreDifference {
                constraint,
                name: name.to_owned(),
                working_score: working.clone(),
                recomputed_score: actual.clone(),
            });
        }
        fresh.push(actual);
    }
    Ok(ScoreConsistencyReport {
        working_score: working_score.clone(),
        recomputed_score: total(&fresh)?,
        constraints,
    })
}

#[cfg(test)]
mod tests {
    use crate::*;
    use std::cell::Cell;
    #[test]
    fn candidate_assertions_detect_cancelling_scope_errors_and_roll_back_all_models() {
        // Deliberately invalid dependency declaration: two constant-scope callbacks
        // read a fact changed by a third evaluator. Total-only checking misses this.
        let fact = Cell::new(0);
        let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap();
        let mut n = ConstraintNetwork::new(1);
        n.add_constraint("trigger", vec![0], |v| {
            fact.set(*v[0].unwrap());
            SimpleScore::ZERO
        })
        .unwrap();
        n.add_constraint("positive", vec![], |_| SimpleScore::new(fact.get()))
            .unwrap();
        n.add_constraint("negative", vec![], |_| SimpleScore::new(-fact.get()))
            .unwrap();
        let mut d = ScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
        let result = d.select_best(
            [Move::Change {
                variable: 0,
                value: Some(1),
            }],
            1,
            false,
            |_, _| true,
            |_| false,
            true,
        );
        let Err(ScoringError::ScoreCorruption(report)) = result else {
            panic!("expected basic corruption")
        };
        assert_eq!(
            report
                .constraints
                .iter()
                .map(|c| c.constraint)
                .collect::<Vec<_>>(),
            [1, 2]
        );
        assert_eq!(report.working_score, report.recomputed_score);
        assert_eq!(d.assignment(), &p.initial_assignment());
        assert_eq!(d.score(), SimpleScore::ZERO);

        let p = ListProblem::new(vec![0, 1], vec![ListEntity::new(vec![0])]).unwrap();
        let mut n = ListConstraintNetwork::new(1, 2);
        n.add_constraint("trigger", vec![ListScope::Entity(0)], |v| {
            let ListFact::Entity(list) = v[0] else {
                panic!()
            };
            fact.set(list.len() as i64);
            SimpleScore::ZERO
        })
        .unwrap();
        n.add_constraint("positive", vec![], |_| SimpleScore::new(fact.get()))
            .unwrap();
        n.add_constraint("negative", vec![], |_| SimpleScore::new(-fact.get()))
            .unwrap();
        let mut d = ListScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
        let initial_score = d.score();
        let result = d.select_best(
            [ListMove::Assign {
                value: 1,
                destination: ListPosition {
                    entity: 0,
                    index: 1,
                },
            }],
            1,
            false,
            |_, _| true,
            |_| false,
            true,
        );
        let Err(ListScoringError::ScoreCorruption(report)) = result else {
            panic!("expected list corruption")
        };
        assert_eq!(report.constraints.len(), 2);
        assert_eq!(d.assignment(), &p.initial_assignment());
        assert_eq!(d.score(), initial_score);

        let p = MixedProblem::new(
            FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap(),
            p,
        );
        let mut n = MixedConstraintNetwork::new(1, 1, 2);
        n.add_constraint("trigger", vec![MixedScope::Basic(0)], |v| {
            let MixedFact::Basic(value) = v[0] else {
                panic!()
            };
            fact.set(*value.unwrap());
            SimpleScore::ZERO
        })
        .unwrap();
        n.add_constraint("positive", vec![], |_| SimpleScore::new(fact.get()))
            .unwrap();
        n.add_constraint("negative", vec![], |_| SimpleScore::new(-fact.get()))
            .unwrap();
        let mut d = MixedScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
        let initial_score = d.score();
        let result = d.select_best(
            [MixedMove {
                basic: Move::Change {
                    variable: 0,
                    value: Some(1),
                },
                lists: ListMove::Composite(vec![]),
            }],
            1,
            false,
            |_, _| true,
            |_| false,
            true,
        );
        let Err(MixedScoringError::ScoreCorruption(report)) = result else {
            panic!("expected mixed corruption")
        };
        assert_eq!(report.constraints.len(), 2);
        assert_eq!(d.assignment(), &p.initial_assignment());
        assert_eq!(d.score(), initial_score);
    }
}
