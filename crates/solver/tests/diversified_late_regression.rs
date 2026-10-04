// Original invariants and independently represented history/search references.
use SimpleScore as S;
use timetuck::*;

#[test]
fn trial_history_matches_rotating_reference_across_many_score_levels() {
    for size in [1, 2, 3, 9] {
        let initial = HardMediumSoftScore::new(-3, -2, -1);
        let mut a = DiversifiedLateAcceptance::new(size, initial).unwrap();
        let mut ring = std::collections::VecDeque::from(vec![initial; size]);
        for step in 0..100 {
            let current = HardMediumSoftScore::new(step % 5 - 3, step % 9 - 4, step % 7 - 2);
            for trial in 0..17 {
                let next = HardMediumSoftScore::new(trial % 7 - 4, trial % 5 - 2, trial % 11 - 6);
                let accepted = next == current || ring.iter().any(|s| next > *s);
                if accepted {
                    let old = ring[0];
                    if next < old || (next > old && next > current) {
                        ring[0] = next;
                    }
                }
                assert_eq!(a.accepts(current, next), accepted);
                let native: Vec<_> = a
                    .history()
                    .iter()
                    .cycle()
                    .skip(a.cursor())
                    .take(size)
                    .copied()
                    .collect();
                assert_eq!(native, ring.iter().copied().collect::<Vec<_>>());
                let worst = ring.iter().min().unwrap();
                assert_eq!(a.worst_score(), worst);
                assert_eq!(
                    a.worst_occurrences(),
                    ring.iter().filter(|s| *s == worst).count()
                );
            }
            a.step_ended();
            ring.rotate_left(1);
        }
    }
}

#[test]
fn strict_worst_boundary_ties_and_rejected_trials_preserve_history() {
    let mut a = DiversifiedLateAcceptance::new(3, S::new(-10)).unwrap();
    assert!(a.accepts(S::new(-10), S::new(-5)));
    assert_eq!(a.history(), &[S::new(-5), S::new(-10), S::new(-10)]);
    assert_eq!(a.worst_occurrences(), 2);
    assert!(!a.accepts(S::new(-5), S::new(-10)));
    assert!(!a.accepts(S::new(-5), S::new(-20)));
    assert_eq!(a.cursor(), 0);
    assert!(a.accepts(S::new(-5), S::new(-7)));
    assert_eq!(a.history()[0], S::new(-7));
    a.step_ended();
    assert!(a.accepts(S::new(-7), S::new(-7)));
    assert_eq!(a.history()[1], S::new(-10)); // Plateau does not fill history.
    assert_eq!(a.worst_occurrences(), 2);
}

#[test]
fn ordered_conjunction_retains_queried_trial_updates_and_skips_later_children() {
    let d = Acceptance::DiversifiedLateAcceptance { history_size: 2 };
    let t = Acceptance::VariableTabu {
        tenure: 2,
        aspiration: false,
    };
    for before in [false, true] {
        let policies = if before { [t, d] } else { [d, t] };
        let mut a = Acceptor::from_policies(&policies, S::new(-10)).unwrap();
        a.step_ended(S::new(-10), &[0]);
        assert!(!a.accepts(S::new(-10), S::new(-5), &[0]));
        let Acceptor::All(children) = &a else {
            panic!()
        };
        let Acceptor::DiversifiedLateAcceptance(history) = &children[usize::from(before)] else {
            panic!()
        };
        assert_eq!(history.cursor(), 1);
        assert_eq!(history.history()[1], S::new(if before { -10 } else { -5 }));
        a.step_ended(S::new(-50), &[0]); // Rejected fallback advances only.
        let Acceptor::All(children) = &a else {
            panic!()
        };
        let Acceptor::DiversifiedLateAcceptance(history) = &children[usize::from(before)] else {
            panic!()
        };
        assert_eq!(history.cursor(), 0);
        assert!(!history.history().contains(&S::new(-50)));
    }
}

#[test]
fn invalid_history_and_fresh_sessions_validate_before_scoring() {
    assert!(matches!(
        DiversifiedLateAcceptance::new(0, S::ZERO),
        Err(AcceptanceError::EmptyHistory)
    ));
    assert!(matches!(
        DiversifiedLateAcceptance::new(usize::MAX, S::ZERO),
        Err(AcceptanceError::HistoryAllocation)
    ));
    let p: Acceptance<S> = Acceptance::DiversifiedLateAcceptance { history_size: 0 };
    assert_eq!(p.validate(), Err(AcceptanceError::EmptyHistory));
    let p = Acceptance::DiversifiedLateAcceptance { history_size: 2 };
    let mut first = p.start(S::new(-10)).unwrap();
    assert!(first.accepts(S::new(-10), S::new(10), &[]));
    first.step_ended(S::new(10), &[]);
    let Acceptor::DiversifiedLateAcceptance(fresh) = p.start(S::new(-20)).unwrap() else {
        panic!()
    };
    assert_eq!(fresh.history(), &[S::new(-20); 2]);
    let extra = [Acceptance::DiversifiedLateAcceptance { history_size: 0 }];
    let problem = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap();
    let mut network = ConstraintNetwork::new(1);
    network
        .add_constraint("never", vec![0], |_| -> S { panic!("must validate first") })
        .unwrap();
    let result = local_search(
        &problem,
        &network,
        LocalSearchConfig {
            acceptance: p,
            additional_acceptance: &extra,
            ..Default::default()
        },
    );
    assert!(result.is_err());
}

fn score(a: &[Option<i32>]) -> S {
    let v: Vec<_> = a.iter().map(|v| v.unwrap()).collect();
    S::new((v[0] * 7 - v[1] * 3 + v[2] * 5 - 8 * i32::from(v[0] == v[2])) as i64)
}
#[test]
fn basic_search_and_streams_match_independent_trial_history_and_selection() {
    let p = FiniteProblem::new(
        (0..3)
            .map(|_| Variable::new(vec![0, 1, 2]).with_initial(0))
            .collect(),
    )
    .unwrap();
    for size in [1, 2, 5] {
        for first in [false, true] {
            for fallback in [false, true] {
                let mut current = vec![Some(0); 3];
                let mut best = (score(&current), current.clone());
                let mut ring = std::collections::VecDeque::from(vec![best.0; size]);
                let mut steps = 0;
                let mut accepted_steps = 0;
                for _ in 0..20 {
                    let old = score(&current);
                    let mut accepted = None;
                    let mut rejected = None;
                    'scan: for variable in 0..3 {
                        for value in 0..3 {
                            if current[variable] == Some(value) {
                                continue;
                            }
                            let mut trial = current.clone();
                            trial[variable] = Some(value);
                            let next = score(&trial);
                            let yes = next == old || ring.iter().any(|s| next > *s);
                            if yes && (next < ring[0] || (next > ring[0] && next > old)) {
                                ring[0] = next;
                            }
                            let slot: &mut Option<(S, Vec<Option<i32>>)> =
                                if yes { &mut accepted } else { &mut rejected };
                            if slot.as_ref().is_none_or(|(s, _)| next > *s) {
                                *slot = Some((next, trial));
                            }
                            if yes && first {
                                break 'scan;
                            }
                        }
                    }
                    let yes = accepted.is_some();
                    let selected = accepted.or(if fallback { rejected } else { None });
                    let Some((s, a)) = selected else {
                        break;
                    };
                    current = a;
                    if s > best.0 {
                        best = (s, current.clone());
                    }
                    steps += 1;
                    accepted_steps += u64::from(yes);
                    ring.rotate_left(1);
                }
                let c = LocalSearchConfig {
                    acceptance: Acceptance::DiversifiedLateAcceptance { history_size: size },
                    max_steps: 20,
                    max_evaluations: 10000,
                    assert_scores: true,
                    neighborhood: Neighborhood::Change,
                    selection: if first {
                        MoveSelection::FirstAccepted
                    } else {
                        MoveSelection::BestAccepted
                    },
                    rejected_moves: if fallback {
                        RejectedMovePolicy::BestScore
                    } else {
                        RejectedMovePolicy::Stop
                    },
                    ..Default::default()
                };
                let mut n = ConstraintNetwork::new(3);
                n.add_constraint("score", vec![0, 1, 2], |v| {
                    score(&v.iter().map(|v| v.copied()).collect::<Vec<_>>())
                })
                .unwrap();
                let a = local_search(&p, &n, c).unwrap();
                let mut b = StreamGraphBuilder::new();
                let source = b.source::<usize, (usize, Option<i32>)>();
                let zero = b.filter(source.port(), |(i, _)| *i == 0).unwrap();
                let one = b.filter(source.port(), |(i, _)| *i == 1).unwrap();
                let two = b.filter(source.port(), |(i, _)| *i == 2).unwrap();
                let pair = b.join_equal(&zero, &one, |_| (), |_| ()).unwrap();
                let triple = b.join_equal(&pair, &two, |_| (), |_| ()).unwrap();
                b.score("score", &triple, |(((_, a), (_, b)), (_, c))| {
                    score(&[*a, *b, *c])
                })
                .unwrap();
                let streamed = stream_local_search(&p, b.build(), source, c).unwrap();
                assert_eq!(streamed.best, a.best);
                assert_eq!(
                    (streamed.completed_steps, streamed.accepted_moves),
                    (steps, accepted_steps)
                );
                let got = a.best.unwrap();
                assert_eq!((got.score, got.assignment.values().to_vec()), best);
                assert_eq!(
                    (a.completed_steps, a.accepted_moves),
                    (steps, accepted_steps)
                );
            }
        }
    }
}
