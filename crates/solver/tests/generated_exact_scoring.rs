//! Original invariants for basic exact candidates scored after entry validation.
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use timetuck::*;

fn plan(strategy: ExactSearchStrategy, budget: u64) -> SolverSequence<'static> {
    SolverSequence {
        phases: vec![SolverPhase::Exact(ExactPhaseConfig {
            strategy,
            max_evaluations: budget,
            ..Default::default()
        })],
        ..Default::default()
    }
}
fn strategies() -> Vec<ExactSearchStrategy> {
    let mut out = vec![
        ExactSearchStrategy::Enumerate,
        ExactSearchStrategy::Complete,
    ];
    for exploration in [
        NodeExploration::DepthFirst,
        NodeExploration::BreadthFirst,
        NodeExploration::ScoreFirst,
        NodeExploration::OptimisticBoundFirst,
        NodeExploration::InputOrderDepthFirst,
    ] {
        let c = BoundedPhaseConfig {
            exploration,
            ..Default::default()
        };
        out.push(ExactSearchStrategy::BranchAndBound(c.clone()));
        out.push(ExactSearchStrategy::CompleteBranchAndBound(c));
    }
    out
}
#[test]
fn generated_candidates_validate_and_match_public_scoring_with_pins_and_optionals() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![7]).with_initial(7).with_pinned(true),
        Variable::new(vec![0, 2]),
        Variable::new(vec![3, -1]).allowing_unassigned(),
        Variable::new(vec![]).allowing_unassigned(),
    ])
    .unwrap();
    let mut reference = ConstraintNetwork::new(4);
    reference
        .add_constraint("score", vec![0, 1, 2, 3], |v| {
            SimpleScore::new(v.iter().flatten().map(|v| **v).sum())
        })
        .unwrap();
    for strategy in strategies() {
        for budget in (0..=15).chain([100]) {
            let seen = RefCell::new(vec![]);
            let mut n = ConstraintNetwork::new(4);
            n.add_constraint("score", vec![0, 1, 2, 3], |v| {
                let a = Assignment::new(v.iter().map(|v| v.copied()).collect());
                p.validate(&a).unwrap();
                let expected = reference.calculate(&p, &a).unwrap();
                seen.borrow_mut().push(a);
                expected.with_structural(0)
            })
            .unwrap();
            let r = solve(&p, &n, plan(strategy.clone(), budget)).unwrap();
            assert_eq!(r.work.evaluations as usize, seen.borrow().len());
            assert_eq!(r.work.constraint_evaluations, r.work.evaluations as u128);
            let mut reference_seen = vec![];
            let (expected_best, expected_evaluations) = match &strategy {
                ExactSearchStrategy::BranchAndBound(c)
                | ExactSearchStrategy::CompleteBranchAndBound(c) => {
                    let expected = branch_and_bound(
                        &p,
                        |a, _| {
                            reference_seen.push(a.clone());
                            BranchEvaluation::unbounded(reference.calculate(&p, a).unwrap())
                        },
                        BranchAndBoundConfig {
                            exploration: c.exploration,
                            max_evaluations: budget,
                            ..Default::default()
                        },
                    )
                    .unwrap();
                    (expected.best, expected.evaluations)
                }
                _ => {
                    let expected = exhaustive_search(
                        &p,
                        |a| {
                            reference_seen.push(a.clone());
                            reference.calculate(&p, a).unwrap()
                        },
                        budget,
                    );
                    (expected.best, expected.evaluations)
                }
            };
            assert_eq!(r.work.evaluations, expected_evaluations);
            assert_eq!(*seen.borrow(), reference_seen);
            assert_eq!(r.score, expected_best.as_ref().map(|b| b.score));
            if let Some(best) = expected_best {
                assert_eq!(r.assignment, best.assignment);
            }
            if budget == 100 {
                assert_eq!(r.score, Some(SimpleScore::new(12)), "{strategy:?}");
            }
        }
    }
}

#[derive(Clone, Debug)]
struct Value {
    id: i64,
    payload: i64,
    comparisons: Rc<Cell<usize>>,
}
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        self.comparisons.set(self.comparisons.get() + 1);
        self.id == other.id
    }
}
impl Eq for Value {}
#[test]
fn completion_preserves_actual_payload_and_domain_checks_do_not_scale_with_leaves() {
    let comparisons = Rc::new(Cell::new(0));
    let value = |id, payload| Value {
        id,
        payload,
        comparisons: comparisons.clone(),
    };
    let p = FiniteProblem::new(vec![
        Variable::new(vec![value(0, 1)]).with_initial(value(0, 99)),
        Variable::new((0..64).map(|i| value(i, i)).collect()),
    ])
    .unwrap();
    let mut n = ConstraintNetwork::<Value, SimpleScore>::new(2);
    n.add_constraint("payload", vec![0, 1], |v| {
        SimpleScore::new(v.iter().flatten().map(|v| v.payload).sum())
    })
    .unwrap();
    for strategy in [
        ExactSearchStrategy::Complete,
        ExactSearchStrategy::CompleteBranchAndBound(BoundedPhaseConfig::default()),
    ] {
        comparisons.set(0);
        let r = solve(&p, &n, plan(strategy, 1000)).unwrap();
        let actual_comparisons = comparisons.get();
        assert_eq!(r.score, Some(SimpleScore::new(162)));
        assert_eq!(r.assignment.values()[0].as_ref().unwrap().payload, 99);
        // Per-candidate range validation would do 1+...+64 comparisons alone.
        // Entry validation and retained-best/result equality remain permitted.
        assert!(actual_comparisons < 512, "{actual_comparisons}");
        assert_eq!(n.calculate(&p, &r.assignment).unwrap(), r.score.unwrap());
    }
}
#[test]
fn public_validation_and_zero_budget_network_validation_precede_callbacks() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![0, 1]).with_initial(0).with_pinned(true),
    ])
    .unwrap();
    let calls = Cell::new(0);
    let mut n = ConstraintNetwork::new(1);
    n.add_constraint("never", vec![0], |_| {
        calls.set(calls.get() + 1);
        SimpleScore::ZERO
    })
    .unwrap();
    for a in [
        Assignment::new(vec![]),
        Assignment::new(vec![Some(9)]),
        Assignment::new(vec![Some(1)]),
    ] {
        assert!(matches!(n.calculate(&p, &a), Err(ScoringError::Model(_))));
    }
    assert_eq!(calls.get(), 0);
    let wrong = ConstraintNetwork::<i64, SimpleScore>::new(2);
    for strategy in strategies() {
        assert!(matches!(
            solve(&p, &wrong, plan(strategy, 0)),
            Err(SolverError::Phase {
                error: LocalSearchError::Scoring(ScoringError::VariableCount { .. }),
                ..
            })
        ));
    }
}
#[test]
fn impossible_required_space_is_not_scored_and_partial_structure_is_still_counted() {
    for pinned in [false, true] {
        let p = FiniteProblem::new(vec![Variable::new(vec![]).with_pinned(pinned)]).unwrap();
        let mut n = ConstraintNetwork::<i64, SimpleScore>::new(1);
        n.add_constraint("never", vec![0], |_| panic!("empty space"))
            .unwrap();
        for strategy in strategies() {
            let r = solve(&p, &n, plan(strategy, 100)).unwrap();
            assert_eq!(r.work.evaluations, 0);
            assert_eq!(r.score, None);
        }
    }
    let p = FiniteProblem::new(vec![Variable::new(vec![0]); 2]).unwrap();
    let n = ConstraintNetwork::<_, SimpleScore>::new(2);
    for budget in [1, 2] {
        let r = solve(
            &p,
            &n,
            plan(
                ExactSearchStrategy::BranchAndBound(BoundedPhaseConfig::default()),
                budget,
            ),
        )
        .unwrap();
        assert_eq!(r.score, None); // Partial structural scores never become a best.
        assert_eq!(r.work.evaluations, budget);
    }
}
#[test]
fn contribution_failures_preserve_order_and_stop_before_later_callbacks() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0])]).unwrap();
    for structural in [true, false] {
        for strategy in strategies() {
            let seen = RefCell::new(vec![]);
            let mut n = ConstraintNetwork::new(1);
            n.add_constraint("first", vec![0], |_| {
                seen.borrow_mut().push(0);
                if structural {
                    SimpleScore::ZERO.with_structural(-1)
                } else {
                    SimpleScore::new(i64::MAX)
                }
            })
            .unwrap();
            n.add_constraint("second", vec![0], |_| {
                seen.borrow_mut().push(1);
                SimpleScore::new(1)
            })
            .unwrap();
            let expected = n.calculate(&p, &p.initial_assignment()).unwrap_err();
            let expected_seen = seen.take();
            let e = solve(&p, &n, plan(strategy, 100)).unwrap_err();
            assert!(
                matches!(e,SolverError::Phase { error: LocalSearchError::Scoring(ref error), .. } if *error==expected)
            );
            assert_eq!(*seen.borrow(), expected_seen);
            assert_eq!(expected_seen, if structural { vec![0] } else { vec![0, 1] });
        }
    }
}
