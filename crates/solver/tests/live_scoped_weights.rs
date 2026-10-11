//! Original live scoped-weight transition regressions; no upstream-method credit.
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
fn s(n: i64) -> SimpleScore {
    SimpleScore::new(n)
}
fn weights(items: &[(&str, SimpleScore)]) -> ConstraintWeightOverrides<SimpleScore> {
    items.iter().map(|(n, w)| (n.to_string(), *w)).collect()
}
fn basic() -> FiniteProblem<i64> {
    FiniteProblem::new(vec![
        Variable::new(vec![1, 2]).with_initial(1),
        Variable::new(vec![3]),
    ])
    .unwrap()
}
fn lists() -> ListProblem<i64> {
    ListProblem::new_indexed(
        vec![7, 9],
        vec![ListEntity::new(vec![7]), ListEntity::new(vec![])],
    )
    .unwrap()
}
fn movement() -> ListMove<i64> {
    ListMove::Change {
        source: ListPosition {
            entity: 0,
            index: 0,
        },
        destination: ListPosition {
            entity: 1,
            index: 0,
        },
    }
}
#[test]
fn effective_weights_follow_commits_rollbacks_explanations_and_shared_network_isolation() {
    let calls = RefCell::new(Vec::new());
    macro_rules! check {
        ($network:expr,$problem:expr,$scope:expr,$director:ident,$movement:expr,$factor:expr) => {{
            let p = $problem;
            let make = || {
                let mut n = $network;
                // Names deliberately oppose registration order.
                n.add_weighted_constraint("z", vec![$scope, $scope], s(2), |f| {
                    calls.borrow_mut().push("z");
                    ($factor)(f)
                })
                .unwrap();
                n.add_weighted_constraint("a", vec![$scope], s(0), |f| {
                    calls.borrow_mut().push("a");
                    ($factor)(f)
                })
                .unwrap();
                n.add_weighted_constraint("live-zero", vec![], s(1), |_| 0)
                    .unwrap();
                n.add_constraint("direct", vec![$scope], |_| s(5)).unwrap();
                n
            };
            let n = make();
            let mut reference_network = make();
            let initial = p.initial_assignment();
            let mut d = $director::new(&p, &n, initial.clone()).unwrap();
            let sibling = $director::new(&p, &n, initial.clone()).unwrap();
            let original_score = d.score();
            for (updates, expected_calls) in [
                (weights(&[("z", s(3)), ("a", s(-4))]), vec!["z", "a"]),
                (weights(&[("z", s(3)), ("a", s(-4))]), vec![]),
                (weights(&[("z", s(0)), ("a", s(0))]), vec![]),
                (weights(&[("a", s(6))]), vec!["a"]),
                (weights(&[("z", s(-2))]), vec!["z"]),
            ] {
                calls.borrow_mut().clear();
                assert_eq!(
                    d.apply_weight_overrides(&updates).unwrap(),
                    expected_calls.len() as u128
                );
                assert_eq!(*calls.borrow(), expected_calls);
                reference_network.apply_weight_overrides(&updates).unwrap();
                let fresh = $director::new(&p, &reference_network, d.assignment().clone()).unwrap();
                assert_eq!(d.score(), fresh.score());
                assert_eq!(
                    d.score().structural(),
                    initial_structural(&initial, original_score)
                );
                assert_eq!(
                    d.constraint_weights().collect::<Vec<_>>(),
                    reference_network.constraint_weights().collect::<Vec<_>>()
                );
                let explanation = d.explain(|name, scope, _, score| {
                    (
                        (name.to_string(), scope.len(), *score),
                        vec!["same", "same"],
                    )
                });
                assert_eq!(
                    explanation,
                    fresh.explain(|name, scope, _, score| (
                        (name.to_string(), scope.len(), *score),
                        vec!["same", "same"]
                    ))
                );
                assert_eq!(explanation.score, d.score());
                assert_eq!(
                    explanation.matches.constraints["live-zero"].matches.len(),
                    1
                );
                for (name, weight) in d.constraint_weights() {
                    assert_eq!(
                        explanation.matches.constraints[name].matches.len(),
                        usize::from(*weight != s(0))
                    );
                }
                assert_eq!(
                    explanation.matches.indictments["same"].score,
                    explanation.matches.score
                );
                assert_eq!(sibling.score(), original_score);
                sibling.assert_consistent().unwrap();
                d.assert_consistent().unwrap();
                let before = d.score();
                {
                    let trial = d.apply(&$movement).unwrap();
                    let fresh_trial =
                        $director::new(&p, &reference_network, trial.assignment().clone()).unwrap();
                    assert_eq!(trial.score(), fresh_trial.score());
                }
                assert_eq!(d.score(), before);
            }
            d.apply(&$movement).unwrap().commit();
            let current = d.assignment().clone();
            d.apply_weight_overrides(&weights(&[("a", s(11))])).unwrap();
            reference_network
                .apply_weight_overrides(&weights(&[("a", s(11))]))
                .unwrap();
            assert_eq!(
                d.score(),
                reference_network.calculate(&p, &current).unwrap()
            );
            {
                let mut session = d.edit_session().unwrap();
                session.nested().unwrap().commit();
                session.assert_consistent().unwrap();
            }
            assert_eq!(d.assignment(), &current);
            d.assert_consistent().unwrap();
        }};
    }
    fn initial_structural<A>(_: &A, score: SimpleScore) -> i64 {
        score.structural()
    }
    check!(
        ConstraintNetwork::new(2),
        basic(),
        0,
        ScoreDirector,
        Move::Change {
            variable: 0,
            value: Some(2)
        },
        |f: &[Option<&i64>]| f[0].copied().unwrap_or(0)
    );
    check!(
        ListConstraintNetwork::new(2, 2),
        lists(),
        ListScope::Value(0),
        ListScoreDirector,
        movement(),
        |f: &[ListFact<'_, i64>]| match &f[0] {
            ListFact::Value { element, .. } =>
                element.as_ref().map_or(0, |e| e.position.entity as i64 + 1),
            _ => unreachable!(),
        }
    );
    check!(
        MixedConstraintNetwork::new(2, 2, 2),
        MixedProblem::new(basic(), lists()),
        MixedScope::ListValue(0),
        MixedScoreDirector,
        MixedMove {
            basic: Move::Change {
                variable: 0,
                value: Some(2)
            },
            lists: movement()
        },
        |f: &[MixedFact<'_, i64, i64>]| match &f[0] {
            MixedFact::ListValue { element, .. } =>
                element.as_ref().map_or(0, |e| e.position.entity as i64 + 1),
            _ => unreachable!(),
        }
    );
}

#[test]
fn invalid_batches_overflow_and_callback_panics_publish_nothing() {
    let fail = Cell::new(false);
    let calls = RefCell::new(Vec::new());
    macro_rules! check {
        ($network:expr,$problem:expr,$director:ident,$error:ident) => {{
            let p = $problem;
            let mut n = $network;
            n.add_weighted_constraint("z-first", vec![], s(1), |_| {
                calls.borrow_mut().push(0);
                2
            })
            .unwrap();
            n.add_weighted_constraint("a-last", vec![], s(1), |_| {
                calls.borrow_mut().push(1);
                assert!(!fail.get(), "factor panic");
                1
            })
            .unwrap();
            n.add_constraint("direct", vec![], |_| s(0)).unwrap();
            let mut d = $director::new(&p, &n, p.initial_assignment()).unwrap();
            let original = d.score();
            for (name, weight, error) in [
                (
                    "missing",
                    s(1),
                    ConstraintWeightError::UnknownConstraint("missing".into()),
                ),
                (
                    "direct",
                    s(1),
                    ConstraintWeightError::UnweightedConstraint("direct".into()),
                ),
                (
                    "a-last",
                    s(1).with_structural(-1),
                    ConstraintWeightError::StructuralWeight("a-last".into()),
                ),
            ] {
                calls.borrow_mut().clear();
                assert_eq!(
                    d.apply_weight_overrides(&weights(&[("z-first", s(5)), (name, weight)])),
                    Err($error::InvalidWeight(error))
                );
                assert!(calls.borrow().is_empty());
            }
            assert_eq!(
                d.apply_weight_overrides(&weights(&[("z-first", s(i64::MAX))])),
                Err($error::ScoreOverflow {
                    constraint: Some(0)
                })
            );
            assert_eq!(
                d.apply_weight_overrides(&weights(&[("a-last", s(i64::MAX))])),
                Err($error::ScoreOverflow { constraint: None })
            );
            calls.borrow_mut().clear();
            fail.set(true);
            assert!(
                catch_unwind(AssertUnwindSafe(|| d.apply_weight_overrides(&weights(&[
                    ("z-first", s(2)),
                    ("a-last", s(3))
                ]))))
                .is_err()
            );
            assert_eq!(*calls.borrow(), vec![0, 1]);
            fail.set(false);
            assert_eq!(d.score(), original);
            assert_eq!(
                d.constraint_weights()
                    .map(|(n, w)| (n, *w))
                    .collect::<Vec<_>>(),
                vec![("z-first", s(1)), ("a-last", s(1))]
            );
            d.assert_consistent().unwrap();
            // Final total fits despite an overflowing intermediate ordinary sum.
            d.apply_weight_overrides(&weights(&[
                ("z-first", s(i64::MAX / 2)),
                ("a-last", s(-i64::MAX)),
            ]))
            .unwrap();
            assert_eq!(d.score(), s(-1).with_structural(original.structural()));
            d.assert_consistent().unwrap();
        }};
    }
    check!(
        ConstraintNetwork::<i64, SimpleScore>::new(2),
        basic(),
        ScoreDirector,
        ScoringError
    );
    check!(
        ListConstraintNetwork::<i64, SimpleScore>::new(2, 2),
        lists(),
        ListScoreDirector,
        ListScoringError
    );
    check!(
        MixedConstraintNetwork::<i64, i64, SimpleScore>::new(2, 2, 2),
        MixedProblem::new(basic(), lists()),
        MixedScoreDirector,
        MixedScoringError
    );
}

#[test]
fn retained_search_and_exact_replacements_use_local_weights_after_each_reactivation() {
    let calls = Cell::new(0u128);
    macro_rules! check {
        ($network:expr,$problem:expr,$scope:expr,$director:ident,$factor:expr,$config:expr,$diagnostics:expr) => {{
            let p = $problem;
            let make = || {
                let mut n = $network;
                n.add_weighted_constraint("value", vec![$scope], s(0), |facts| {
                    calls.set(calls.get() + 1);
                    ($factor)(facts)
                })
                .unwrap();
                n
            };
            let n = make();
            let mut reference = make();
            let mut d = $director::new(&p, &n, p.initial_assignment()).unwrap();
            // Each run starts from the previous run's best, not the model's initial assignment.
            for weight in [3, 0, -2, 0, 5] {
                let overrides = weights(&[("value", s(weight))]);
                calls.set(0);
                assert_eq!(
                    d.apply_weight_overrides(&overrides).unwrap(),
                    u128::from(weight != 0)
                );
                assert_eq!(calls.get(), u128::from(weight != 0));
                reference.apply_weight_overrides(&overrides).unwrap();
                let mut fresh = $director::new(&p, &reference, d.assignment().clone()).unwrap();
                calls.set(0);
                let result = d.run_solver($config).unwrap();
                if $diagnostics {
                    assert!(calls.get() >= result.work.constraint_evaluations);
                } else {
                    assert_eq!(calls.get(), result.work.constraint_evaluations);
                }
                let expected = fresh.run_solver($config).unwrap();
                assert_eq!(result, expected);
                assert_eq!(d.assignment(), fresh.assignment());
                assert_eq!(d.score(), fresh.score());
                if weight == 0 {
                    assert_eq!(result.work.constraint_evaluations, 0);
                } else {
                    assert!(result.work.constraint_evaluations > 0);
                }
                d.assert_consistent().unwrap();
            }
        }};
    }
    for phase in 0..4 {
        let diagnostics = phase == 3;
        let config = || SolverSequence {
            phases: vec![if phase == 1 {
                SolverPhase::Exact(ExactPhaseConfig::default())
            } else if phase == 2 {
                SolverPhase::RuinRecreate(RuinRecreateConfig {
                    minimum_ruined: 1,
                    maximum_ruined: 1,
                    max_attempts: 3,
                    seed: 9,
                    ..Default::default()
                })
            } else {
                SolverPhase::LocalSearch(LocalSearchConfig {
                    max_steps: 4,
                    assert_scores: diagnostics,
                    ..Default::default()
                })
            }],
            ..Default::default()
        };
        let mixed_config = || MixedSolverSequence {
            phases: vec![if phase == 1 {
                MixedSolverPhase::Exact(ExactPhaseConfig::default())
            } else if phase == 2 {
                MixedSolverPhase::RuinRecreate(MixedRuinRecreateConfig {
                    search: RuinRecreateConfig {
                        minimum_ruined: 1,
                        maximum_ruined: 1,
                        max_attempts: 3,
                        seed: 9,
                        ..Default::default()
                    },
                    ..Default::default()
                })
            } else {
                MixedSolverPhase::LocalSearch(MixedLocalSearchConfig {
                    search: LocalSearchConfig {
                        max_steps: 4,
                        assert_scores: diagnostics,
                        ..Default::default()
                    },
                    ..Default::default()
                })
            }],
            ..Default::default()
        };
        let bp = || FiniteProblem::new(vec![Variable::new(vec![1, 2]).with_initial(1)]).unwrap();
        let lp = || {
            ListProblem::new_indexed(
                vec![7],
                vec![ListEntity::new(vec![7]), ListEntity::new(vec![])],
            )
            .unwrap()
        };
        check!(
            ConstraintNetwork::new(1),
            bp(),
            0,
            ScoreDirector,
            |f: &[Option<&i64>]| f[0].copied().unwrap_or(0),
            config(),
            diagnostics
        );
        check!(
            ListConstraintNetwork::new(2, 1),
            lp(),
            ListScope::Value(0),
            ListScoreDirector,
            |f: &[ListFact<'_, i64>]| match &f[0] {
                ListFact::Value { element, .. } =>
                    element.as_ref().map_or(0, |e| e.position.entity as i64),
                _ => unreachable!(),
            },
            config(),
            diagnostics
        );
        check!(
            MixedConstraintNetwork::new(1, 2, 1),
            MixedProblem::new(bp(), lp()),
            MixedScope::ListValue(0),
            MixedScoreDirector,
            |f: &[MixedFact<'_, i64, i64>]| match &f[0] {
                MixedFact::ListValue { element, .. } =>
                    element.as_ref().map_or(0, |e| e.position.entity as i64),
                _ => unreachable!(),
            },
            mixed_config(),
            diagnostics
        );
    }
}

#[test]
fn reactivated_dense_value_scopes_keep_positions_neighbors_and_actual_payloads() {
    #[derive(Clone, Debug)]
    struct Value {
        id: usize,
        payload: i64,
    }
    impl PartialEq for Value {
        fn eq(&self, other: &Self) -> bool {
            self.id == other.id
        }
    }
    impl Eq for Value {}
    impl std::hash::Hash for Value {
        fn hash<H: std::hash::Hasher>(&self, h: &mut H) {
            self.id.hash(h)
        }
    }
    let count = 129;
    let p = ListProblem::new_indexed(
        (0..count).map(|id| Value { id, payload: 0 }).collect(),
        vec![
            ListEntity::new(
                (0..count - 1)
                    .map(|id| Value {
                        id,
                        payload: 1000 + id as i64,
                    })
                    .collect(),
            ),
            ListEntity::new(vec![]),
        ],
    )
    .unwrap();
    let make = || {
        let mut n = ListConstraintNetwork::<Value, SimpleScore>::new(2, count);
        for i in 0..count {
            n.add_weighted_constraint(
                format!("v{i}"),
                vec![
                    ListScope::Value(i),
                    ListScope::Value((i + 1) % count),
                    ListScope::Value(i),
                ],
                s(0),
                |facts| {
                    facts
                        .iter()
                        .map(|f| match f {
                            ListFact::Value { element, .. } => element.as_ref().map_or(-3, |e| {
                                e.position.index as i64 + e.previous.map_or(0, |v| v.payload)
                                    - e.next.map_or(0, |v| v.payload)
                            }),
                            _ => unreachable!(),
                        })
                        .sum()
                },
            )
            .unwrap();
        }
        n
    };
    let n = make();
    let mut reference = make();
    let mut d = ListScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
    for weight in [2, 0, -3] {
        let overrides = (0..count).map(|i| (format!("v{i}"), s(weight))).collect();
        assert_eq!(
            d.apply_weight_overrides(&overrides).unwrap(),
            if weight == 0 { 0 } else { count as u128 }
        );
        reference.apply_weight_overrides(&overrides).unwrap();
        assert_eq!(d.score(), reference.calculate(&p, d.assignment()).unwrap());
        d.apply(&ListMove::Change {
            source: ListPosition {
                entity: 0,
                index: 0,
            },
            destination: ListPosition {
                entity: 0,
                index: 127,
            },
        })
        .unwrap()
        .commit();
        assert_eq!(d.score(), reference.calculate(&p, d.assignment()).unwrap());
        d.assert_consistent().unwrap();
    }
}

#[test]
fn exact_decimal_transition_errors_leave_effective_configuration_unchanged() {
    let fail = Cell::new(false);
    let p = basic();
    let mut n = ConstraintNetwork::new(2);
    n.add_decimal_weighted_constraint(
        "fraction",
        vec![0],
        "0.1".parse::<SimpleDecimalScore>().unwrap(),
        |_| {
            if fail.get() {
                Decimal::new(1.into(), i32::MAX)
            } else {
                "0.025".parse().unwrap()
            }
        },
    )
    .unwrap();
    let mut d = ScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
    let overrides = [("fraction".to_string(), "0.01".parse().unwrap())]
        .into_iter()
        .collect();
    let score = d.score();
    fail.set(true);
    assert!(matches!(
        d.apply_weight_overrides(&overrides),
        Err(ScoringError::DecimalWeightArithmetic {
            constraint: 0,
            error: DecimalArithmeticError::ScaleOverflow
        })
    ));
    assert_eq!(d.score(), score);
    assert_eq!(
        d.constraint_weights().next().unwrap().1,
        &"0.1".parse::<SimpleDecimalScore>().unwrap()
    );
    fail.set(false);
    assert_eq!(d.apply_weight_overrides(&overrides).unwrap(), 1);
    assert_eq!(
        d.score(),
        "0.00025"
            .parse::<SimpleDecimalScore>()
            .unwrap()
            .with_structural(-1)
    );
    d.assert_consistent().unwrap();
}

thread_local! {
    static CLONES: Cell<usize> = const { Cell::new(0) };
    static PANIC_AT: Cell<usize> = const { Cell::new(usize::MAX) };
}
#[derive(Debug, Eq, PartialEq, Ord, PartialOrd)]
struct PanicScore(SimpleScore);
impl Clone for PanicScore {
    fn clone(&self) -> Self {
        CLONES.with(|n| {
            n.set(n.get() + 1);
            PANIC_AT.with(|at| assert_ne!(n.get(), at.get(), "score clone panic"));
        });
        Self(self.0)
    }
}
impl Score for PanicScore {
    type Level = i64;
    const HARD_LEVEL_COUNT: usize = 0;
    fn ordinary_levels(&self) -> impl Iterator<Item = i64> {
        self.0.ordinary_levels()
    }
    fn structural(&self) -> i64 {
        self.0.structural()
    }
    fn hard_cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.hard_cmp(&other.0)
    }
    fn is_feasible(&self) -> bool {
        self.0.is_feasible()
    }
}
impl std::ops::Add for PanicScore {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self(self.0 + rhs.0)
    }
}
impl std::ops::Sub for PanicScore {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self(self.0 - rhs.0)
    }
}
impl AdditiveScore for PanicScore {
    const ZERO: Self = Self(SimpleScore::ZERO);
    fn with_structural(self, structural: i64) -> Self {
        Self(self.0.with_structural(structural))
    }
}
#[test]
fn every_score_clone_failure_during_reweight_preserves_the_complete_director() {
    macro_rules! check {
        ($network:expr,$problem:expr,$director:ident) => {{
            let p = $problem;
            let mut n = $network;
            n.add_weighted_constraint("first", vec![], PanicScore(s(1)), |_| 3)
                .unwrap();
            n.add_weighted_constraint("last", vec![], PanicScore(s(2)), |_| 5)
                .unwrap();
            let updates = [
                ("first".to_owned(), PanicScore(s(4))),
                ("last".to_owned(), PanicScore(s(6))),
            ]
            .into_iter()
            .collect();
            let mut d = $director::new(&p, &n, p.initial_assignment()).unwrap();
            CLONES.set(0);
            d.apply_weight_overrides(&updates).unwrap();
            let total = CLONES.get();
            assert!(total > 4);
            // Also test updating an already overridden director (shared Arc snapshot).
            for prior_override in [false, true] {
                for at in 1..=total {
                    let mut d = $director::new(&p, &n, p.initial_assignment()).unwrap();
                    if prior_override {
                        d.apply_weight_overrides(
                            &[("first".to_owned(), PanicScore(s(7)))]
                                .into_iter()
                                .collect(),
                        )
                        .unwrap();
                    }
                    let before = d.score();
                    let old_weights: Vec<_> = d
                        .constraint_weights()
                        .map(|(name, w)| (name.to_owned(), w.clone()))
                        .collect();
                    let assignment = d.assignment().clone();
                    CLONES.set(0);
                    PANIC_AT.set(at);
                    let result =
                        catch_unwind(AssertUnwindSafe(|| d.apply_weight_overrides(&updates)));
                    PANIC_AT.set(usize::MAX);
                    assert!(result.is_err(), "clone {at} did not execute");
                    assert_eq!(d.score(), before);
                    assert_eq!(d.assignment(), &assignment);
                    assert_eq!(
                        d.constraint_weights()
                            .map(|(name, w)| (name.to_owned(), w.clone()))
                            .collect::<Vec<_>>(),
                        old_weights
                    );
                    d.assert_consistent().unwrap();
                }
            }
        }};
    }
    check!(
        ConstraintNetwork::<i64, PanicScore>::new(2),
        basic(),
        ScoreDirector
    );
    check!(
        ListConstraintNetwork::<i64, PanicScore>::new(2, 2),
        lists(),
        ListScoreDirector
    );
    check!(
        MixedConstraintNetwork::<i64, i64, PanicScore>::new(2, 2, 2),
        MixedProblem::new(basic(), lists()),
        MixedScoreDirector
    );
}
