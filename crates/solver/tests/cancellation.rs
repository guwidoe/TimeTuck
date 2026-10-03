//! Original cancellation checks; no upstream method credit.
use std::{
    cell::Cell,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    time::Duration,
};
use timetuck::*;
#[derive(Debug, PartialEq)]
struct Outcome {
    score: Option<SimpleScore>,
    evaluations: u64,
    steps: u64,
    reason: LocalSearchTermination,
}
fn config(token: &CancellationToken) -> LocalSearchConfig<'_> {
    LocalSearchConfig {
        cancellation: Some(token),
        max_evaluations: 100,
        max_steps: 20,
        selection: MoveSelection::BestAccepted,
        ..Default::default()
    }
}
trait LandscapeValue {
    fn value(&self) -> usize;
}
impl LandscapeValue for Assignment<usize> {
    fn value(&self) -> usize {
        self.values()[0].unwrap()
    }
}
impl LandscapeValue for ListAssignment<i32> {
    fn value(&self) -> usize {
        self.location(&7).unwrap().entity
    }
}
impl LandscapeValue for MixedAssignment<usize, i32> {
    fn value(&self) -> usize {
        self.basic().values()[0].unwrap()
    }
}

// A value 0 -> 1 -> 2 landscape in all six model/backend paths. In the list
// models the value is its owner index; mixed models score their basic variable.
fn run(mode: usize, c: LocalSearchConfig<'_>, score: impl Fn(usize) -> SimpleScore) -> Outcome {
    macro_rules! out {
        ($r:expr,$p:expr) => {{
            let r = $r.unwrap();
            if let Some(best) = &r.best {
                $p.validate(&best.assignment).unwrap();
                assert!($p.is_complete(&best.assignment));
                assert_eq!(best.score, SimpleScore::new(best.assignment.value() as i64));
            }
            Outcome {
                score: r.best.map(|b| b.score),
                evaluations: r.evaluations,
                steps: r.completed_steps,
                reason: r.termination,
            }
        }};
    }
    if mode < 2 {
        let p =
            FiniteProblem::new(vec![Variable::new(vec![0usize, 1, 2]).with_initial(0)]).unwrap();
        if mode == 0 {
            let mut n = ConstraintNetwork::new(1);
            n.add_constraint("value", vec![0], |v| score(*v[0].unwrap()))
                .unwrap();
            return out!(local_search_from(&p, &n, p.initial_assignment(), c), p);
        }
        let mut b = StreamGraphBuilder::new();
        let s = b.source::<usize, (usize, Option<usize>)>();
        b.score("value", s.port(), |(_, v)| score(v.unwrap()))
            .unwrap();
        return out!(
            stream_local_search_from(&p, b.build(), s, p.initial_assignment(), c),
            p
        );
    }
    let p = ListProblem::new(
        vec![7],
        vec![
            ListEntity::new(vec![7]),
            ListEntity::new(vec![]),
            ListEntity::new(vec![]),
        ],
    )
    .unwrap();
    if mode < 4 {
        if mode == 2 {
            let mut n = ListConstraintNetwork::new(3, 1);
            n.add_constraint("value", vec![ListScope::Value(0)], |v| {
                let ListFact::Value { element, .. } = &v[0] else {
                    unreachable!()
                };
                score(element.as_ref().unwrap().position.entity)
            })
            .unwrap();
            return out!(list_local_search_from(&p, &n, p.initial_assignment(), c), p);
        }
        let mut b = StreamGraphBuilder::new();
        let s = ListStreamSources::<i32>::new(&mut b);
        b.score("value", s.values.port(), |v| {
            score(v.position.unwrap().entity)
        })
        .unwrap();
        return out!(
            list_stream_local_search_from(&p, b.build(), s, p.initial_assignment(), c),
            p
        );
    }
    let p = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![0usize, 1, 2]).with_initial(0)]).unwrap(),
        p,
    );
    let c = MixedLocalSearchConfig {
        search: c,
        joint_moves: true,
    };
    if mode == 4 {
        let mut n = MixedConstraintNetwork::new(1, 3, 1);
        n.add_constraint("value", vec![MixedScope::Basic(0)], |v| {
            let MixedFact::Basic(v) = v[0] else {
                unreachable!()
            };
            score(*v.unwrap())
        })
        .unwrap();
        return out!(
            mixed_local_search_from(&p, &n, p.initial_assignment(), c),
            p
        );
    }
    let mut b = StreamGraphBuilder::new();
    let s = MixedStreamSources::<usize, i32>::new(&mut b);
    b.score("value", s.basic.port(), |(_, v)| score(v.unwrap()))
        .unwrap();
    out!(
        mixed_stream_local_search_from(&p, b.build(), s, p.initial_assignment(), c),
        p
    )
}
#[test]
fn token_identity_is_shared_and_exactly_one_concurrent_request_is_first() {
    let token = CancellationToken::new();
    assert!(!token.is_cancelled());
    assert_eq!(token, token.clone());
    assert_ne!(token, CancellationToken::new());
    let first = Arc::new(AtomicUsize::new(0));
    std::thread::scope(|scope| {
        for _ in 0..8 {
            let token = token.clone();
            let first = first.clone();
            scope.spawn(move || {
                if token.cancel() {
                    first.fetch_add(1, Ordering::Relaxed);
                }
            });
        }
    });
    assert_eq!(first.load(Ordering::Relaxed), 1);
    assert!(token.is_cancelled());
    assert!(!token.cancel());
}
#[test]
fn pre_cancelled_search_never_scores_and_does_not_reset_the_request() {
    for mode in 0..6 {
        let token = CancellationToken::new();
        token.cancel();
        for _ in 0..2 {
            let r = run(mode, config(&token), |_| panic!("pre-cancelled callback"));
            assert_eq!(
                r,
                Outcome {
                    score: None,
                    evaluations: 0,
                    steps: 0,
                    reason: LocalSearchTermination::Cancelled
                }
            );
        }
        let r = run(
            mode,
            LocalSearchConfig {
                max_evaluations: 0,
                ..config(&token)
            },
            |_| panic!("zero budget callback"),
        );
        assert_eq!(r.reason, LocalSearchTermination::EvaluationLimit);
        let fresh = CancellationToken::new();
        assert!(!fresh.is_cancelled());
        assert_eq!(
            run(mode, config(&fresh), |v| SimpleScore::new(v as i64)).score,
            Some(SimpleScore::new(2))
        );
    }
}
#[test]
fn cancellation_during_initial_or_candidate_scoring_preserves_the_scored_prefix() {
    for mode in 0..6 {
        for at in [0, 1] {
            for selection in [MoveSelection::FirstAccepted, MoveSelection::BestAccepted] {
                let token = CancellationToken::new();
                let calls = Cell::new(0);
                let c = LocalSearchConfig {
                    selection,
                    max_evaluations: 2,
                    move_order: MoveOrder::ShuffledMoves {
                        seed: 9,
                        window: std::num::NonZeroUsize::new(1).unwrap(),
                    },
                    ..config(&token)
                };
                let r = run(mode, c, |v| {
                    calls.set(calls.get() + 1);
                    if v == at {
                        token.cancel();
                    }
                    SimpleScore::new(v as i64)
                });
                assert_eq!(
                    r,
                    Outcome {
                        score: Some(SimpleScore::new(at as i64)),
                        evaluations: at as u64 + 1,
                        steps: at as u64,
                        reason: LocalSearchTermination::Cancelled
                    }
                );
                assert_eq!(calls.get(), at + 1);
            }
        }
    }
}
#[test]
fn another_thread_can_cancel_an_active_callback_without_discarding_its_candidate() {
    for mode in 0..6 {
        let token = CancellationToken::new();
        let worker_token = token.clone();
        let (entered_tx, entered_rx) = mpsc::sync_channel(0);
        let (release_tx, release_rx) = mpsc::sync_channel(0);
        let worker = std::thread::spawn(move || {
            run(mode, config(&worker_token), |v| {
                if v == 1 {
                    entered_tx.send(()).unwrap();
                    release_rx.recv_timeout(Duration::from_secs(10)).unwrap();
                }
                SimpleScore::new(v as i64)
            })
        });
        entered_rx.recv_timeout(Duration::from_secs(10)).unwrap();
        assert!(token.cancel());
        release_tx.send(()).unwrap();
        let r = worker.join().unwrap();
        assert_eq!(
            r,
            Outcome {
                score: Some(SimpleScore::new(1)),
                evaluations: 2,
                steps: 1,
                reason: LocalSearchTermination::Cancelled
            }
        );
    }
}
#[test]
fn cancellation_does_not_mask_invalid_initial_assignments_or_acceptance() {
    let token = CancellationToken::new();
    token.cancel();
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1])]).unwrap();
    let n = ConstraintNetwork::<i32, SimpleScore>::new(1);
    assert!(matches!(
        local_search(
            &p,
            &n,
            LocalSearchConfig {
                acceptance: Acceptance::LateAcceptance {
                    history_size: 0,
                    hill_climbing: false
                },
                ..config(&token)
            }
        ),
        Err(LocalSearchError::Acceptance(AcceptanceError::EmptyHistory))
    ));
    assert!(local_search_from(&p, &n, Assignment::new(vec![None]), config(&token)).is_err());
    let p = ListProblem::new(vec![7], vec![ListEntity::new(vec![])]).unwrap();
    let n = ListConstraintNetwork::<i32, SimpleScore>::new(1, 1);
    assert!(list_local_search_from(&p, &n, p.initial_assignment(), config(&token)).is_err());
}

#[test]
fn rejected_candidate_cancellation_keeps_global_best_with_or_without_fallback() {
    for mode in 0..6 {
        for rejected_moves in [RejectedMovePolicy::Stop, RejectedMovePolicy::BestScore] {
            let token = CancellationToken::new();
            let r = run(
                mode,
                LocalSearchConfig {
                    rejected_moves,
                    ..config(&token)
                },
                |v| {
                    if v == 1 {
                        token.cancel();
                    }
                    SimpleScore::new(-(v as i64))
                },
            );
            assert_eq!(
                r,
                Outcome {
                    score: Some(SimpleScore::new(0)),
                    evaluations: 2,
                    steps: u64::from(rejected_moves == RejectedMovePolicy::BestScore),
                    reason: LocalSearchTermination::Cancelled
                }
            );
        }
    }
}
