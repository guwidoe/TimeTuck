//! Original live first-accepted selection controls; no upstream method credit.
use super::*;
use crate::search_scoring::SearchDirector;
use crate::{
    ForagerConfig, MoveSelection, RejectedMovePolicy, SimpleScore, StreamConstraint,
    StreamGraphBuilder, Variable,
};
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

type Trace = RefCell<Vec<String>>;
type Constraint = StreamConstraint<usize, (usize, usize)>;
fn problem() -> FiniteProblem<usize> {
    FiniteProblem::new(vec![
        Variable::new(vec![0usize, 1, 2, 3, 4]).with_initial(0),
        Variable::new(vec![0usize, 1, 2, 3, 4])
            .with_initial(1)
            .with_pinned(true),
    ])
    .unwrap()
}
fn change(value: usize) -> Move<usize> {
    Move::Change {
        variable: 0,
        value: Some(value),
    }
}
fn director<'p, 'a>(
    p: &'p FiniteProblem<usize>,
    trace: &'a Trace,
    force_legacy: bool,
) -> (StreamScoreDirector<'p, 'a, usize, SimpleScore>, Constraint) {
    let mut b = StreamGraphBuilder::new();
    let source = b.source::<usize, (usize, Option<usize>)>();
    let assigned = b.filter(source.port(), |(_, v)| v.is_some()).unwrap();
    let mapped = b.map(&assigned, |&(id, v)| (id, v.unwrap())).unwrap();
    let constraint = b
        .score("value", &mapped, |&(id, v)| {
            trace.borrow_mut().push(format!("score:{id}:{v}"));
            SimpleScore::new(v as i64)
        })
        .unwrap();
    let mut d = StreamScoreDirector::new(p, b.build(), source, p.initial_assignment()).unwrap();
    assert!(
        d.graph.supports_live_commit(),
        "borrowed callbacks and primitive state should be eligible"
    );
    if force_legacy {
        d.graph.disable_live_commit_for_tests();
        assert!(!d.graph.supports_live_commit());
    }
    trace.borrow_mut().clear();
    (d, constraint)
}
type Matches = Vec<(usize, ((usize, usize), SimpleScore))>;

#[derive(Debug, PartialEq, Eq)]
struct State {
    assignment: Vec<Option<usize>>,
    score: SimpleScore,
    source: Vec<(usize, (usize, Option<usize>))>,
    matches: Matches,
}
fn state(d: &StreamScoreDirector<'_, '_, usize, SimpleScore>, c: &Constraint) -> State {
    State {
        assignment: d.assignment.values().to_vec(),
        score: d.score(),
        source: d
            .graph
            .source_rows(&d.source)
            .unwrap()
            .iter()
            .map(|(k, v)| (*k, *v))
            .collect(),
        matches: d
            .graph
            .constraint_matches(c)
            .unwrap()
            .iter()
            .map(|(k, v)| (*k, *v))
            .collect(),
    }
}
#[derive(Debug, PartialEq, Eq)]
struct Summary {
    selected: Option<Move<usize>>,
    accepted: bool,
    evaluations: u64,
    replays: u64,
    constraints: u128,
    limit: bool,
}
impl From<Selection<usize>> for Summary {
    fn from(s: Selection<usize>) -> Self {
        Self {
            selected: s.selected,
            accepted: s.selected_accepted,
            evaluations: s.evaluations,
            replays: s.replay_evaluations,
            constraints: s.constraint_evaluations,
            limit: s.evaluation_limit,
        }
    }
}
fn config(fallback: RejectedMovePolicy) -> ForagerConfig<SimpleScore> {
    ForagerConfig {
        selection: MoveSelection::FirstAccepted,
        rejected_moves: fallback,
        ..Default::default()
    }
}

#[test]
fn live_commit_matches_legacy_candidate_order_budgets_empty_and_rejected_fallback() {
    let p = problem();
    for (moves, budget, fallback, threshold, evals, accepted, selected, limit) in [
        (
            vec![change(1), change(2), change(3), change(4)],
            10,
            RejectedMovePolicy::Stop,
            4,
            3,
            true,
            Some(change(3)),
            false,
        ),
        (
            vec![change(1), change(2), change(1)],
            10,
            RejectedMovePolicy::BestScore,
            10,
            3,
            false,
            Some(change(2)),
            false,
        ),
        (
            vec![change(1), change(2)],
            0,
            RejectedMovePolicy::Stop,
            0,
            0,
            false,
            None,
            true,
        ),
        (
            vec![change(3), change(4)],
            1,
            RejectedMovePolicy::Stop,
            4,
            1,
            true,
            Some(change(3)),
            false,
        ),
        (
            vec![change(1), change(3)],
            1,
            RejectedMovePolicy::BestScore,
            4,
            1,
            false,
            Some(change(1)),
            true,
        ),
        (
            vec![change(1), change(3)],
            2,
            RejectedMovePolicy::Stop,
            4,
            2,
            true,
            Some(change(3)),
            false,
        ),
        (
            vec![],
            10,
            RejectedMovePolicy::Stop,
            0,
            0,
            false,
            None,
            false,
        ),
        (
            vec![],
            0,
            RejectedMovePolicy::Stop,
            0,
            0,
            false,
            None,
            false,
        ),
    ] {
        let mut outcomes = Vec::new();
        for legacy in [false, true] {
            let trace = Trace::default();
            let (mut d, c) = director(&p, &trace, legacy);
            let result = d
                .select_best(
                    moves.clone(),
                    budget,
                    config(fallback),
                    |movement, score| {
                        trace
                            .borrow_mut()
                            .push(format!("accept:{movement:?}:{}", score.score()));
                        score >= SimpleScore::new(threshold)
                    },
                    |_| false,
                    false,
                )
                .unwrap();
            let result = Summary::from(result);
            assert_eq!(
                (result.evaluations, result.constraints, result.replays),
                (evals, evals as u128, 0)
            );
            assert_eq!(
                (result.accepted, result.selected.clone(), result.limit),
                (accepted, selected.clone(), limit)
            );
            let log = trace.borrow().clone();
            let final_state = state(&d, &c);
            d.assert_consistent().unwrap();
            outcomes.push((result, final_state, log));
        }
        assert_eq!(outcomes[0], outcomes[1]);
    }
}

struct ObservedMoves<'a> {
    moves: std::vec::IntoIter<Move<usize>>,
    trace: &'a Trace,
    dropped: &'a Cell<bool>,
    panic_drop: bool,
}
impl Iterator for ObservedMoves<'_> {
    type Item = Move<usize>;
    fn next(&mut self) -> Option<Self::Item> {
        let next = self.moves.next();
        self.trace.borrow_mut().push(format!("next:{next:?}"));
        next
    }
}
impl Drop for ObservedMoves<'_> {
    fn drop(&mut self) {
        self.trace.borrow_mut().push("iterator-drop".into());
        self.dropped.set(true);
        assert!(!self.panic_drop, "iterator drop failure");
    }
}
struct Policy<'a> {
    trace: &'a Trace,
    dropped: &'a Cell<bool>,
    mode: u8,
}
impl crate::construction::SelectionPolicy<SimpleScore> for Policy<'_> {
    fn should_pick(&self, _: SimpleScore) -> bool {
        self.trace.borrow_mut().push("early-pick".into());
        false
    }
    fn can_commit(&self, limit: bool) -> bool {
        self.trace.borrow_mut().push(format!("can-commit:{limit}"));
        assert!(
            self.dropped.get(),
            "iterator must be dropped before can_commit"
        );
        assert_ne!(self.mode, 2, "commit policy failure");
        self.mode == 0
    }
}

#[test]
fn live_commit_iterator_destruction_commit_rejection_and_panics_keep_base_and_callback_order() {
    let p = problem();
    // 0=accepted, 1=can_commit rejects, 2=can_commit panics,
    // 3=iterator Drop panics, 4=acceptance panics on the accepted candidate.
    for mode in 0..5 {
        let mut outcomes = Vec::new();
        for legacy in [false, true] {
            let trace = Trace::default();
            let (mut d, c) = director(&p, &trace, legacy);
            let base = state(&d, &c);
            let dropped = Cell::new(false);
            let iter = ObservedMoves {
                moves: vec![change(1), change(3), change(4)].into_iter(),
                trace: &trace,
                dropped: &dropped,
                panic_drop: mode == 3,
            };
            let outcome = catch_unwind(AssertUnwindSafe(|| {
                d.select_best(
                    iter,
                    10,
                    config(RejectedMovePolicy::Stop),
                    |_, score| {
                        trace.borrow_mut().push(format!("accept:{}", score.score()));
                        assert!(
                            !(mode == 4 && score >= SimpleScore::new(4)),
                            "acceptance failure"
                        );
                        score >= SimpleScore::new(4)
                    },
                    Policy {
                        trace: &trace,
                        dropped: &dropped,
                        mode: if mode == 1 || mode == 2 { mode } else { 0 },
                    },
                    false,
                )
            }));
            let summary = match outcome {
                Ok(result) => Some(Summary::from(result.unwrap())),
                Err(_) => None,
            };
            assert_eq!(summary.is_some(), mode < 2);
            assert!(dropped.get());
            let final_state = state(&d, &c);
            let log = trace.borrow().clone();
            if mode != 0 {
                assert_eq!(final_state, base);
            } else {
                assert_eq!(final_state.assignment[0], Some(3));
            }
            assert_eq!(
                log.iter().filter(|line| line.starts_with("score:")).count(),
                2
            );
            d.assert_consistent().unwrap();
            outcomes.push((summary, final_state, log));
        }
        assert_eq!(outcomes[0], outcomes[1], "mode {mode}");
    }
}

#[test]
fn live_commit_composite_duplicate_writes_pins_and_invalid_moves_match_legacy() {
    let p = problem();
    let duplicate = Move::Composite(vec![change(1), change(3), change(2), change(3)]);
    let invalids = [
        Move::Change {
            variable: 1,
            value: Some(2usize),
        },
        change(99),
        Move::Composite(vec![
            change(2),
            Move::Change {
                variable: 99,
                value: Some(3usize),
            },
        ]),
    ];
    for movement in std::iter::once(duplicate)
        .chain(std::iter::once(Move::Composite(vec![])))
        .chain(invalids)
    {
        let mut outcomes = Vec::new();
        for legacy in [false, true] {
            let trace = Trace::default();
            let (mut d, c) = director(&p, &trace, legacy);
            let base = state(&d, &c);
            let result = d.select_best(
                [movement.clone()],
                1,
                config(RejectedMovePolicy::Stop),
                |_, _| true,
                |_| false,
                false,
            );
            let result = match result {
                Ok(result) => Ok(Summary::from(result)),
                Err(error) => Err(format!("{error:?}")),
            };
            let final_state = state(&d, &c);
            if result.is_err() {
                assert_eq!(final_state, base);
            }
            let log = trace.borrow().clone();
            d.assert_consistent().unwrap();
            outcomes.push((result, final_state, log));
        }
        assert_eq!(outcomes[0], outcomes[1]);
    }
}

#[test]
fn live_commit_phase_installs_retained_best_after_accepting_worse_steps() {
    let p = problem();
    let mut outcomes = Vec::new();
    for legacy in [false, true] {
        let trace = Trace::default();
        let (mut d, c) = director(&p, &trace, legacy);
        let result = d
            .run_local_search(LocalSearchConfig {
                max_evaluations: 20,
                max_steps: 4,
                max_accepted_moves: 4,
                neighborhood: crate::Neighborhood::Change,
                acceptance: crate::Acceptance::SimulatedAnnealing {
                    starting_temperature: SimpleScore::new(1_000_000_000),
                    cooling_steps: 1000,
                    seed: 7,
                },
                selection: MoveSelection::FirstAccepted,
                assert_scores: false,
                ..Default::default()
            })
            .unwrap();
        let best = result.best.unwrap();
        assert_eq!(best.score, d.score());
        assert_eq!(&best.assignment, d.assignment());
        assert_eq!(result.accepted_moves, 4);
        assert_eq!(d.assignment.values()[0], Some(1));
        let output = (
            best.score,
            best.assignment.values().to_vec(),
            result.evaluations,
            result.constraint_evaluations,
            result.accepted_moves,
            result.completed_steps,
            state(&d, &c),
            trace.borrow().clone(),
        );
        d.assert_consistent().unwrap();
        outcomes.push(output);
    }
    assert_eq!(outcomes[0], outcomes[1]);
}

#[test]
fn live_commit_diagnostic_mode_and_drop_callbacks_keep_reference_behavior() {
    let p = problem();
    for diagnostic in [false, true] {
        let mut outputs = Vec::new();
        for legacy in [false, true] {
            let trace = Trace::default();
            let (mut d, c) = director(&p, &trace, legacy);
            let result = d
                .select_best(
                    [change(1), change(3)],
                    2,
                    config(RejectedMovePolicy::Stop),
                    |_, score| score >= SimpleScore::new(4),
                    |_| false,
                    diagnostic,
                )
                .unwrap();
            let result = Summary::from(result);
            let log = trace.borrow().clone();
            let final_state = state(&d, &c);
            d.assert_consistent().unwrap();
            outputs.push((result, final_state, log));
        }
        assert_eq!(outputs[0], outputs[1]);
    }
    // Capturing Rc adds destruction effects and must retain the reference route.
    let calls = Rc::new(Cell::new(0));
    let mut b = StreamGraphBuilder::new();
    let source = b.source::<usize, (usize, Option<usize>)>();
    let counter = calls.clone();
    b.score("drop callback", source.port(), move |(_, value)| {
        counter.set(counter.get() + 1);
        SimpleScore::new(value.unwrap_or(0) as i64)
    })
    .unwrap();
    let mut d = StreamScoreDirector::new(&p, b.build(), source, p.initial_assignment()).unwrap();
    assert!(!d.graph.supports_live_commit());
    let before = calls.get();
    let result = d
        .select_best(
            [change(3)],
            1,
            config(RejectedMovePolicy::Stop),
            |_, _| true,
            |_| false,
            false,
        )
        .unwrap();
    assert_eq!(result.constraint_evaluations, 1);
    assert_eq!(calls.get(), before + 1);
    d.assert_consistent().unwrap();
}

#[test]
fn live_commit_custom_value_fallback_keeps_owned_values_and_selection_results() {
    #[derive(Clone, Debug, PartialEq, Eq)]
    struct Value {
        number: usize,
        owner: Rc<()>,
    }
    let owner = Rc::new(());
    let zero = Value {
        number: 0,
        owner: owner.clone(),
    };
    let one = Value {
        number: 1,
        owner: owner.clone(),
    };
    let p = FiniteProblem::new(vec![
        Variable::new(vec![zero.clone(), one.clone()]).with_initial(zero),
    ])
    .unwrap();
    let mut outputs = Vec::new();
    for force in [false, true] {
        let calls = Cell::new(0);
        let mut b = StreamGraphBuilder::new();
        let source = b.source::<usize, (usize, Option<Value>)>();
        b.score("custom", source.port(), |(_, value)| {
            calls.set(calls.get() + 1);
            SimpleScore::new(value.as_ref().unwrap().number as i64)
        })
        .unwrap();
        let mut d =
            StreamScoreDirector::new(&p, b.build(), source, p.initial_assignment()).unwrap();
        assert!(!d.graph.supports_live_commit());
        if force {
            d.graph.disable_live_commit_for_tests();
        }
        let before = calls.get();
        let selected = d
            .select_best(
                [Move::Change {
                    variable: 0,
                    value: Some(one.clone()),
                }],
                1,
                config(RejectedMovePolicy::Stop),
                |_, _| true,
                |_| false,
                false,
            )
            .unwrap();
        assert!(selected.selected_accepted);
        assert_eq!(calls.get(), before + 1);
        assert!(Rc::ptr_eq(
            &d.assignment.values()[0].as_ref().unwrap().owner,
            &owner
        ));
        outputs.push((
            d.score(),
            d.assignment.values()[0].as_ref().unwrap().number,
            selected.evaluations,
            selected.constraint_evaluations,
        ));
        d.assert_consistent().unwrap();
    }
    assert_eq!(outputs[0], outputs[1]);
}

#[test]
fn live_commit_graph_errors_and_scoring_panics_after_rejections_restore_the_original_graph() {
    let p = problem();
    for panic_score in [false, true] {
        let mut outputs = Vec::new();
        for legacy in [false, true] {
            let trace = Trace::default();
            let mut b = StreamGraphBuilder::new();
            let source = b.source::<usize, (usize, Option<usize>)>();
            let constraint = b
                .score("failing", source.port(), |&(id, value)| {
                    trace.borrow_mut().push(format!("score:{id}:{value:?}"));
                    if value == Some(3usize) {
                        assert!(!panic_score, "scoring callback failure");
                        return SimpleScore::ZERO.with_structural(1);
                    }
                    SimpleScore::new(value.unwrap_or(0usize) as i64)
                })
                .unwrap();
            let mut d =
                StreamScoreDirector::new(&p, b.build(), source, p.initial_assignment()).unwrap();
            assert!(d.graph.supports_live_commit());
            if legacy {
                d.graph.disable_live_commit_for_tests();
            }
            let assignment = d.assignment.clone();
            let score = d.score();
            let sources = d.graph.source_rows(&d.source).unwrap().clone();
            let matches = d.graph.constraint_matches(&constraint).unwrap().clone();
            trace.borrow_mut().clear();
            let outcome = catch_unwind(AssertUnwindSafe(|| {
                d.select_best(
                    [change(1), change(3)],
                    2,
                    config(RejectedMovePolicy::BestScore),
                    |_, _| false,
                    |_| false,
                    false,
                )
            }));
            let error = match outcome {
                Ok(Err(error)) => Some(format!("{error:?}")),
                Err(_) => None,
                Ok(Ok(_)) => panic!("failure must prevent selection"),
            };
            assert_eq!(error.is_none(), panic_score);
            assert_eq!(d.assignment, assignment);
            assert_eq!(d.score(), score);
            assert_eq!(d.graph.source_rows(&d.source).unwrap(), &sources);
            assert_eq!(d.graph.constraint_matches(&constraint).unwrap(), &matches);
            let log = trace.borrow().clone();
            assert_eq!(log.len(), 2);
            d.assert_consistent().unwrap();
            outputs.push((error, log));
        }
        assert_eq!(outputs[0], outputs[1]);
    }
}
