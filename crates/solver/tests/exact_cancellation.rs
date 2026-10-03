//! Original exact-search cancellation checks; no upstream method credit.
use timetuck::*;

#[derive(Clone, Debug, PartialEq)]
struct State {
    basic: Vec<Option<usize>>,
    lists: Vec<Vec<usize>>,
}
fn score(a: &State) -> SimpleScore {
    let n = a.basic.iter().flatten().sum::<usize>()
        + a.lists
            .iter()
            .enumerate()
            .map(|(owner, values)| (owner + 1) * values.iter().sum::<usize>())
            .sum::<usize>();
    SimpleScore::new(n as i64).with_structural(if n % 3 == 0 { -1 } else { 0 })
}
#[derive(Debug, PartialEq)]
struct Outcome {
    best: Option<(State, SimpleScore)>,
    evaluations: u64,
    reason: Termination,
    optimal: bool,
}
fn run(
    mode: usize,
    config: ExactSearchConfig<'_>,
    mut evaluate: impl FnMut(&State) -> SimpleScore,
) -> Outcome {
    let basic = FiniteProblem::new(vec![
        Variable::new(vec![0usize, 1]).allowing_unassigned(),
        Variable::new(vec![9]).with_initial(9).with_pinned(true),
    ])
    .unwrap();
    let lists = ListProblem::new(
        vec![7usize, 8],
        vec![
            ListEntity::new(vec![7]).with_pinned_prefix(1),
            ListEntity::new(vec![]),
        ],
    )
    .unwrap()
    .allowing_unassigned();
    macro_rules! outcome {
        ($r:expr,$p:expr,$state:expr) => {{
            let r = $r;
            if let Some(best) = &r.best {
                $p.validate(&best.assignment).unwrap();
                assert!($p.is_complete(&best.assignment));
            }
            let optimal = r.is_optimal();
            Outcome {
                best: r.best.map(|b| ($state(&b.assignment), b.score)),
                evaluations: r.evaluations,
                reason: r.termination,
                optimal,
            }
        }};
    }
    if mode == 0 {
        let state = |a: &Assignment<usize>| State {
            basic: a.values().to_vec(),
            lists: vec![],
        };
        return outcome!(
            exhaustive_search_with_config(&basic, |a| evaluate(&state(a)), config),
            basic,
            state
        );
    }
    if mode == 1 {
        let state = |a: &ListAssignment<usize>| State {
            basic: vec![],
            lists: a.lists().to_vec(),
        };
        return outcome!(
            exhaustive_list_search_with_config(&lists, |a| evaluate(&state(a)), config),
            lists,
            state
        );
    }
    let p = MixedProblem::new(basic, lists);
    let state = |a: &MixedAssignment<usize, usize>| State {
        basic: a.basic().values().to_vec(),
        lists: a.lists().lists().to_vec(),
    };
    outcome!(
        exhaustive_mixed_search_with_config(&p, |a| evaluate(&state(a)), config),
        p,
        state
    )
}
#[test]
fn every_cancelled_exact_prefix_retains_eligible_best_without_claiming_optimality() {
    for mode in 0..3 {
        let mut states = vec![];
        let full = run(mode, ExactSearchConfig::default(), |a| {
            states.push(a.clone());
            score(a)
        });
        let total = states.len() as u64;
        assert_eq!(total, if mode < 2 { 3 } else { 9 });
        assert_eq!(full.reason, Termination::Exhausted);
        for budget in 0..=total + 1 {
            for cancel_at in 0..=total + 1 {
                let token = CancellationToken::new();
                if cancel_at == 0 {
                    token.cancel();
                }
                let mut calls = 0;
                let r = run(
                    mode,
                    ExactSearchConfig {
                        max_evaluations: budget,
                        cancellation: Some(&token),
                    },
                    |a| {
                        assert_eq!(a, &states[calls as usize]);
                        calls += 1;
                        if calls == cancel_at {
                            token.cancel();
                        }
                        score(a)
                    },
                );
                let count = total.min(budget).min(cancel_at);
                assert_eq!(r.evaluations, count);
                assert_eq!(calls, count);
                let reason = if count == total {
                    Termination::Exhausted
                } else if count == budget {
                    Termination::EvaluationLimit
                } else {
                    Termination::Cancelled
                };
                assert_eq!(
                    r.reason, reason,
                    "mode {mode} budget {budget} cancel {cancel_at}"
                );
                let mut best = None;
                for a in &states[..count as usize] {
                    let s = score(a);
                    if s.structural() >= 0 && best.as_ref().is_none_or(|(_, prior)| s > *prior) {
                        best = Some((a.clone(), s));
                    }
                }
                assert_eq!(
                    r.optimal,
                    reason == Termination::Exhausted && best.is_some()
                );
                assert_eq!(r.best, best);
            }
        }
    }
}
#[test]
fn empty_spaces_are_exhausted_but_empty_assignments_still_need_evaluation() {
    let token = CancellationToken::new();
    token.cancel();
    for max_evaluations in [0, 1] {
        let c = ExactSearchConfig {
            max_evaluations,
            cancellation: Some(&token),
        };
        let basic = FiniteProblem::<usize>::new(vec![Variable::new(vec![])]).unwrap();
        let lists = ListProblem::<usize>::new(vec![7], vec![]).unwrap();
        let b =
            exhaustive_search_with_config(&basic, |_| -> SimpleScore { panic!("empty space") }, c);
        let l = exhaustive_list_search_with_config(
            &lists,
            |_| -> SimpleScore { panic!("empty space") },
            c,
        );
        let p = MixedProblem::new(basic, lists);
        let m = exhaustive_mixed_search_with_config(
            &p,
            |_| -> SimpleScore { panic!("empty space") },
            c,
        );
        for (reason, best, optimal) in [
            (b.termination, b.best.is_some(), b.is_optimal()),
            (l.termination, l.best.is_some(), l.is_optimal()),
            (m.termination, m.best.is_some(), m.is_optimal()),
        ] {
            assert_eq!(reason, Termination::Exhausted);
            assert!(!best);
            assert!(!optimal);
        }
        let basic = FiniteProblem::<usize>::new(vec![]).unwrap();
        let lists = ListProblem::<usize>::new(vec![], vec![]).unwrap();
        let b = exhaustive_search_with_config(
            &basic,
            |_| -> SimpleScore { panic!("cancelled empty assignment") },
            c,
        );
        let l = exhaustive_list_search_with_config(
            &lists,
            |_| -> SimpleScore { panic!("cancelled empty assignment") },
            c,
        );
        let p = MixedProblem::new(basic, lists);
        let m = exhaustive_mixed_search_with_config(
            &p,
            |_| -> SimpleScore { panic!("cancelled empty assignment") },
            c,
        );
        for reason in [b.termination, l.termination, m.termination] {
            assert_eq!(
                reason,
                if max_evaluations == 0 {
                    Termination::EvaluationLimit
                } else {
                    Termination::Cancelled
                }
            );
        }
        // Without the request, one empty assignment must be scored, then exhausted.
        let r = exhaustive_mixed_search_with_config(
            &p,
            |_| SimpleScore::ZERO,
            ExactSearchConfig {
                max_evaluations: 1,
                ..Default::default()
            },
        );
        assert_eq!(r.evaluations, 1);
        assert!(r.is_optimal());
    }
}
#[test]
fn exact_callback_cancellation_can_be_requested_from_a_controller_thread() {
    use std::{sync::mpsc, time::Duration};
    for mode in 0..3 {
        let token = CancellationToken::new();
        let worker_token = token.clone();
        let (entered_tx, entered_rx) = mpsc::sync_channel(0);
        let (release_tx, release_rx) = mpsc::sync_channel(0);
        let worker = std::thread::spawn(move || {
            run(
                mode,
                ExactSearchConfig {
                    cancellation: Some(&worker_token),
                    ..Default::default()
                },
                |a| {
                    entered_tx.send(()).unwrap();
                    release_rx.recv_timeout(Duration::from_secs(10)).unwrap();
                    score(a)
                },
            )
        });
        entered_rx.recv_timeout(Duration::from_secs(10)).unwrap();
        token.cancel();
        release_tx.send(()).unwrap();
        let r = worker.join().unwrap();
        assert_eq!(r.reason, Termination::Cancelled);
        assert_eq!(r.evaluations, 1);
        assert!(!r.optimal);
    }
}
