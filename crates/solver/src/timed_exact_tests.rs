//! Original controlled-clock exact-search checks; no upstream method credit.
use crate::*;
use std::{cell::Cell, time::Duration};

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
    clock: impl FnMut() -> Duration,
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
            crate::search::exhaustive_search_with_clock(
                &basic,
                |a| evaluate(&state(a)),
                config,
                clock
            ),
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
            crate::list_search::exhaustive_list_search_with_clock(
                &lists,
                |a| evaluate(&state(a)),
                config,
                clock
            ),
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
        crate::mixed_search::exhaustive_mixed_search_with_clock(
            &p,
            |a| evaluate(&state(a)),
            config,
            clock
        ),
        p,
        state
    )
}

#[test]
fn every_deadline_count_and_cancellation_prefix_preserves_eligible_best() {
    for mode in 0..3 {
        let mut states = vec![];
        let full = run(
            mode,
            ExactSearchConfig::default(),
            |a| {
                states.push(a.clone());
                score(a)
            },
            || panic!("disabled clock"),
        );
        let total = states.len() as u64;
        assert_eq!(total, if mode < 2 { 3 } else { 9 });
        assert!(full.optimal);
        for budget in 0..=total + 1 {
            for cancel_at in 0..=total + 1 {
                for deadline in 0..=total + 1 {
                    let token = CancellationToken::new();
                    if cancel_at == 0 {
                        token.cancel();
                    }
                    let elapsed = Cell::new(0u64);
                    let mut calls = 0;
                    let r = run(
                        mode,
                        ExactSearchConfig {
                            max_evaluations: budget,
                            cancellation: Some(&token),
                            max_time: Some(Duration::from_millis(deadline)),
                        },
                        |a| {
                            assert_eq!(a, &states[calls as usize]);
                            calls += 1;
                            elapsed.set(calls);
                            if calls == cancel_at {
                                token.cancel();
                            }
                            score(a)
                        },
                        || Duration::from_millis(elapsed.get()),
                    );
                    let count = total.min(budget).min(cancel_at).min(deadline);
                    let reason = if count == total {
                        Termination::Exhausted
                    } else if count == budget {
                        Termination::EvaluationLimit
                    } else if count == cancel_at {
                        Termination::Cancelled
                    } else {
                        Termination::TimeLimit
                    };
                    assert_eq!(
                        (r.evaluations, calls, r.reason),
                        (count, count, reason),
                        "mode {mode}, budget {budget}, cancel {cancel_at}, deadline {deadline}"
                    );
                    let mut best = None;
                    for a in &states[..count as usize] {
                        let s = score(a);
                        if s.structural() >= 0 && best.as_ref().is_none_or(|(_, old)| s > *old) {
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
}
#[test]
fn late_callbacks_finish_and_ties_or_structural_rejection_keep_correct_results() {
    for mode in 0..3 {
        for rejected in [false, true] {
            let elapsed = Cell::new(Duration::ZERO);
            let mut visited = vec![];
            let r = run(
                mode,
                ExactSearchConfig {
                    max_time: Some(Duration::from_nanos(1)),
                    ..Default::default()
                },
                |a| {
                    visited.push(a.clone());
                    elapsed.set(Duration::from_secs(100));
                    SimpleScore::new(-9).with_structural(if rejected { -1 } else { 0 })
                },
                || elapsed.get(),
            );
            assert_eq!(r.evaluations, 1);
            assert_eq!(r.reason, Termination::TimeLimit);
            assert!(!r.optimal);
            assert_eq!(
                r.best,
                if rejected {
                    None
                } else {
                    Some((visited[0].clone(), SimpleScore::new(-9)))
                }
            );
        }
        let elapsed = Cell::new(0);
        let mut first = None;
        let r = run(
            mode,
            ExactSearchConfig {
                max_time: Some(Duration::from_nanos(2)),
                ..Default::default()
            },
            |a| {
                first.get_or_insert(a.clone());
                elapsed.set(elapsed.get() + 1);
                SimpleScore::new(7)
            },
            || Duration::from_nanos(elapsed.get()),
        );
        assert_eq!(r.best, Some((first.unwrap(), SimpleScore::new(7))));
        assert_eq!(r.evaluations, 2);
        assert_eq!(r.reason, Termination::TimeLimit);
    }
}
#[test]
fn disabled_and_backwards_clocks_preserve_enumeration_and_latch_expiry() {
    for mode in 0..3 {
        let plain = run(mode, ExactSearchConfig::default(), score, || {
            panic!("disabled clock")
        });
        let mut samples = [4, 2, 5, 0].into_iter();
        let r = run(
            mode,
            ExactSearchConfig {
                max_time: Some(Duration::from_nanos(5)),
                ..Default::default()
            },
            score,
            || Duration::from_nanos(samples.next().expect("must not resume after expiry")),
        );
        assert_eq!(r.evaluations, 2);
        assert_eq!(r.reason, Termination::TimeLimit);
        assert!(!r.optimal);
        let huge = run(
            mode,
            ExactSearchConfig {
                max_time: Some(Duration::MAX),
                ..Default::default()
            },
            score,
            || Duration::from_secs(100),
        );
        assert_eq!(huge, plain);
    }
}
#[test]
fn empty_spaces_do_not_poll_time_but_singleton_empty_assignments_do() {
    let c = ExactSearchConfig {
        max_time: Some(Duration::ZERO),
        ..Default::default()
    };
    let basic = FiniteProblem::<usize>::new(vec![Variable::new(vec![])]).unwrap();
    let lists = ListProblem::<usize>::new(vec![1], vec![]).unwrap();
    let b = crate::search::exhaustive_search_with_clock(
        &basic,
        |_| -> SimpleScore { panic!("impossible") },
        c,
        || panic!("empty clock"),
    );
    let l = crate::list_search::exhaustive_list_search_with_clock(
        &lists,
        |_| -> SimpleScore { panic!("impossible") },
        c,
        || panic!("empty clock"),
    );
    let m = crate::mixed_search::exhaustive_mixed_search_with_clock(
        &MixedProblem::new(basic, lists),
        |_| -> SimpleScore { panic!("impossible") },
        c,
        || panic!("empty clock"),
    );
    for (reason, count) in [
        (b.termination, b.evaluations),
        (l.termination, l.evaluations),
        (m.termination, m.evaluations),
    ] {
        assert_eq!((reason, count), (Termination::Exhausted, 0));
    }
    let b = FiniteProblem::<usize>::new(vec![]).unwrap();
    let l = ListProblem::<usize>::new(vec![], vec![]).unwrap();
    let r = crate::mixed_search::exhaustive_mixed_search_with_clock(
        &MixedProblem::new(b, l),
        |_| -> SimpleScore { panic!("zero deadline") },
        c,
        || Duration::ZERO,
    );
    assert_eq!(r.evaluations, 0);
    assert_eq!(r.termination, Termination::TimeLimit);
    assert!(!r.is_optimal());
}

#[derive(Debug)]
struct Costly<'a> {
    value: usize,
    elapsed: &'a Cell<u64>,
}
impl Clone for Costly<'_> {
    fn clone(&self) -> Self {
        self.elapsed.set(self.elapsed.get() + 1);
        Self {
            value: self.value,
            elapsed: self.elapsed,
        }
    }
}
impl PartialEq for Costly<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}
impl Eq for Costly<'_> {}
#[test]
fn model_preparation_and_mixed_component_cloning_consume_the_same_budget() {
    let elapsed = Cell::new(0);
    let key = |value| Costly {
        value,
        elapsed: &elapsed,
    };
    let basic = FiniteProblem::new(vec![Variable::new(vec![key(0), key(1)])]).unwrap();
    let lists = ListProblem::new(vec![key(7), key(8)], vec![ListEntity::new(vec![])]).unwrap();
    let c = ExactSearchConfig {
        max_time: Some(Duration::from_nanos(1)),
        ..Default::default()
    };
    elapsed.set(0);
    let b = crate::search::exhaustive_search_with_clock(
        &basic,
        |_| -> SimpleScore { panic!("setup spent time") },
        c,
        || Duration::from_nanos(elapsed.get()),
    );
    assert_eq!(b.termination, Termination::TimeLimit);
    assert!(elapsed.get() > 0);
    elapsed.set(0);
    let l = crate::list_search::exhaustive_list_search_with_clock(
        &lists,
        |_| -> SimpleScore { panic!("setup spent time") },
        c,
        || Duration::from_nanos(elapsed.get()),
    );
    assert_eq!(l.termination, Termination::TimeLimit);
    assert!(elapsed.get() > 0);
    let mixed = MixedProblem::new(basic, lists);
    elapsed.set(0);
    let m = crate::mixed_search::exhaustive_mixed_search_with_clock(
        &mixed,
        |_| -> SimpleScore { panic!("setup spent time") },
        c,
        || Duration::from_nanos(elapsed.get()),
    );
    assert_eq!(m.termination, Termination::TimeLimit);
    assert!(elapsed.get() > 0);
    assert_eq!((b.evaluations, l.evaluations, m.evaluations), (0, 0, 0));
}

#[test]
fn mixed_owned_candidate_preparation_is_checked_before_its_scorer() {
    let elapsed = Cell::new(0);
    let key = |value| Costly {
        value,
        elapsed: &elapsed,
    };
    let p = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![key(0), key(1)])]).unwrap(),
        ListProblem::new(vec![key(7)], vec![ListEntity::new(vec![])]).unwrap(),
    );
    elapsed.set(0);
    let mut first = 0;
    crate::mixed_search::exhaustive_mixed_search_with_clock(
        &p,
        |_| {
            first = elapsed.get();
            SimpleScore::ZERO
        },
        ExactSearchConfig {
            max_evaluations: 1,
            ..Default::default()
        },
        || panic!("disabled"),
    );
    assert!(first >= 2);
    elapsed.set(0);
    let r = crate::mixed_search::exhaustive_mixed_search_with_clock(
        &p,
        |_| -> SimpleScore { panic!("joint clone spent time") },
        ExactSearchConfig {
            max_time: Some(Duration::from_nanos(first)),
            ..Default::default()
        },
        || Duration::from_nanos(elapsed.get()),
    );
    assert_eq!(r.evaluations, 0);
    assert_eq!(r.termination, Termination::TimeLimit);
    assert!(!r.is_optimal());
}
