//! Original shared-control checks across all six repair entry points.
use std::time::Duration;
use timetuck::*;
#[derive(Clone, Copy, Debug, PartialEq)]
enum Kind {
    Basic,
    Lists,
    Mixed,
}
const KINDS: [Kind; 3] = [Kind::Basic, Kind::Lists, Kind::Mixed];
fn problem(kind: Kind, optional: bool) -> MixedProblem<i32, i32> {
    let nb = match kind {
        Kind::Basic => 2,
        Kind::Lists => 0,
        Kind::Mixed => 1,
    };
    let nl = 2 - nb;
    let basics = (0..nb)
        .map(|_| {
            let v = Variable::new(vec![0, 1]).with_initial(0);
            if optional { v.allowing_unassigned() } else { v }
        })
        .collect();
    let lists = ListProblem::new(
        (0..nl).collect(),
        if nl == 0 {
            vec![]
        } else {
            vec![ListEntity::new((0..nl).collect()), ListEntity::new(vec![])]
        },
    )
    .unwrap();
    MixedProblem::new(
        FiniteProblem::new(basics).unwrap(),
        if optional {
            lists.allowing_unassigned()
        } else {
            lists
        },
    )
}
fn project(kind: Kind, basic: &[Option<i32>], lists: &[Vec<i32>]) -> Vec<Option<i32>> {
    let mut out = basic.to_vec();
    let nl = if kind == Kind::Basic {
        0
    } else if kind == Kind::Lists {
        2
    } else {
        1
    };
    for value in 0..nl {
        out.push(
            lists
                .iter()
                .position(|l| l.contains(&value))
                .map(|i| i as i32),
        );
    }
    out
}
fn map<T, U>(r: RuinRecreateOutcome<T>, f: impl FnOnce(T) -> U) -> RuinRecreateOutcome<U> {
    RuinRecreateOutcome {
        best: r.best.map(f),
        evaluations: r.evaluations,
        repair_move_evaluations: r.repair_move_evaluations,
        fine_move_attempts: r.fine_move_attempts,
        fine_move_evaluations: r.fine_move_evaluations,
        constraint_evaluations: r.constraint_evaluations,
        attempts: r.attempts,
        complete_reconstructions: r.complete_reconstructions,
        accepted_moves: r.accepted_moves,
        termination: r.termination,
    }
}
fn graph<'a, S: AdditiveScore + 'static>(
    kind: Kind,
    score: &'a impl Fn(&[Option<i32>]) -> S,
) -> Result<(StreamGraph<'a, S>, MixedStreamSources<i32, i32>), GraphError> {
    let mut b = StreamGraphBuilder::new();
    let sources = MixedStreamSources::new(&mut b);
    let bs = b.collect_list(sources.basic.port(), |r| *r)?;
    let ls = b.collect_list(sources.lists.entities.port(), |r| r.clone())?;
    match kind {
        Kind::Basic => {
            b.score("quality", &bs, |rows| {
                let mut values = vec![None; rows.len()];
                for (i, v) in rows.iter() {
                    values[*i] = *v;
                }
                score(&values)
            })?;
        }
        Kind::Lists => {
            b.score("quality", &ls, |rows| {
                let mut lists = vec![vec![]; rows.len()];
                for (i, l) in rows.iter() {
                    lists[*i] = l.clone();
                }
                score(&project(Kind::Lists, &[], &lists))
            })?;
        }
        Kind::Mixed => {
            let all = b.join_equal(&bs, &ls, |_| 0, |_| 0)?;
            b.score("quality", &all, move |(bs, ls)| {
                let mut basics = vec![None; bs.len()];
                let mut lists = vec![vec![]; ls.len()];
                for (i, v) in bs.iter() {
                    basics[*i] = *v;
                }
                for (i, l) in ls.iter() {
                    lists[*i] = l.clone();
                }
                score(&project(kind, &basics, &lists))
            })?;
        }
    }
    Ok((b.build(), sources))
}
fn run<S: AdditiveScore + 'static>(
    kind: Kind,
    stream: bool,
    optional: bool,
    c: RuinRecreateConfig<S>,
    score: impl Fn(&[Option<i32>]) -> S,
) -> RuinRecreateOutcome<(Vec<Option<i32>>, S)> {
    run_observed(kind, stream, optional, c, score, None)
}
fn run_observed<S: AdditiveScore + 'static>(
    kind: Kind,
    stream: bool,
    optional: bool,
    c: RuinRecreateConfig<S>,
    score: impl Fn(&[Option<i32>]) -> S,
    mut observer: Option<&mut dyn FnMut(Vec<Option<i32>>, S)>,
) -> RuinRecreateOutcome<(Vec<Option<i32>>, S)> {
    macro_rules! repair {
        ($plain:path, $observed:path, ($($arg:expr),* $(,)?), $project:expr) => {
            if let Some(observer) = observer.as_deref_mut() {
                $observed($($arg),*, |u| {
                    assert!(u.complete);
                    observer(($project)(u.assignment), u.score);
                })
            } else {
                $plain($($arg),*)
            }
        };
    }
    let p = problem(kind, optional);
    let result = match (kind, stream) {
        (Kind::Basic, false) => {
            let mut n = ConstraintNetwork::new(2);
            n.add_constraint("quality", vec![0, 1], |v| {
                score(&v.iter().map(|v| v.copied()).collect::<Vec<_>>())
            })
            .unwrap();
            map(
                repair!(
                    ruin_recreate_search_from,
                    ruin_recreate_search_from_with_observer,
                    (p.basic(), &n, p.basic().initial_assignment(), c),
                    |a: &Assignment<i32>| a.values().to_vec()
                )
                .unwrap(),
                |b| (b.assignment.values().to_vec(), b.score),
            )
        }
        (Kind::Basic, true) => map(
            repair!(
                ruin_recreate_stream_search_from,
                ruin_recreate_stream_search_from_with_observer,
                (
                    p.basic(),
                    || graph(kind, &score).map(|(g, s)| (g, s.basic)),
                    p.basic().initial_assignment(),
                    c,
                ),
                |a: &Assignment<i32>| a.values().to_vec()
            )
            .unwrap(),
            |b| (b.assignment.values().to_vec(), b.score),
        ),
        (Kind::Lists, false) => {
            let mut n = ListConstraintNetwork::new(2, 2);
            n.add_constraint(
                "quality",
                vec![ListScope::Entity(0), ListScope::Entity(1)],
                |f| {
                    let l: Vec<_> = f
                        .iter()
                        .map(|v| {
                            let ListFact::Entity(l) = v else {
                                unreachable!()
                            };
                            l.to_vec()
                        })
                        .collect();
                    score(&project(kind, &[], &l))
                },
            )
            .unwrap();
            map(
                repair!(
                    ruin_recreate_list_search_from,
                    ruin_recreate_list_search_from_with_observer,
                    (p.lists(), &n, p.lists().initial_assignment(), c),
                    |a: &ListAssignment<i32>| project(kind, &[], a.lists())
                )
                .unwrap(),
                |b| (project(kind, &[], b.assignment.lists()), b.score),
            )
        }
        (Kind::Lists, true) => map(
            repair!(
                ruin_recreate_list_stream_search_from,
                ruin_recreate_list_stream_search_from_with_observer,
                (
                    p.lists(),
                    || graph(kind, &score).map(|(g, s)| (g, s.lists)),
                    p.lists().initial_assignment(),
                    c,
                ),
                |a: &ListAssignment<i32>| project(kind, &[], a.lists())
            )
            .unwrap(),
            |b| (project(kind, &[], b.assignment.lists()), b.score),
        ),
        (Kind::Mixed, false) => {
            let mut n = MixedConstraintNetwork::new(1, 2, 1);
            n.add_constraint(
                "quality",
                vec![
                    MixedScope::Basic(0),
                    MixedScope::ListEntity(0),
                    MixedScope::ListEntity(1),
                ],
                |f| {
                    let (MixedFact::Basic(b), MixedFact::ListEntity(a), MixedFact::ListEntity(z)) =
                        (&f[0], &f[1], &f[2])
                    else {
                        unreachable!()
                    };
                    score(&project(kind, &[b.copied()], &[a.to_vec(), z.to_vec()]))
                },
            )
            .unwrap();
            map(
                repair!(
                    ruin_recreate_mixed_search_from,
                    ruin_recreate_mixed_search_from_with_observer,
                    (
                        &p,
                        &n,
                        p.initial_assignment(),
                        MixedRuinRecreateConfig {
                            search: c,
                            ..Default::default()
                        },
                    ),
                    |a: &MixedAssignment<i32, i32>| project(
                        kind,
                        a.basic().values(),
                        a.lists().lists()
                    )
                )
                .unwrap(),
                |b| {
                    (
                        project(
                            kind,
                            b.assignment.basic().values(),
                            b.assignment.lists().lists(),
                        ),
                        b.score,
                    )
                },
            )
        }
        (Kind::Mixed, true) => map(
            repair!(
                ruin_recreate_mixed_stream_search_from,
                ruin_recreate_mixed_stream_search_from_with_observer,
                (
                    &p,
                    || graph(kind, &score),
                    p.initial_assignment(),
                    MixedRuinRecreateConfig {
                        search: c,
                        ..Default::default()
                    },
                ),
                |a: &MixedAssignment<i32, i32>| project(
                    kind,
                    a.basic().values(),
                    a.lists().lists()
                )
            )
            .unwrap(),
            |b| {
                (
                    project(
                        kind,
                        b.assignment.basic().values(),
                        b.assignment.lists().lists(),
                    ),
                    b.score,
                )
            },
        ),
    };
    if let Some((values, best)) = &result.best {
        assert_eq!(
            *best,
            score(values),
            "independent final score for {kind:?} stream={stream}"
        );
    }
    result
}

fn config<S>() -> RuinRecreateConfig<'static, S> {
    RuinRecreateConfig {
        minimum_ruined: usize::MAX,
        maximum_ruined: usize::MAX,
        max_attempts: 3,
        assert_scores: true,
        ..Default::default()
    }
}
fn quality(v: &[Option<i32>]) -> HardSoftScore {
    HardSoftScore::new(
        -2 + v.iter().flatten().map(|v| i64::from(*v)).sum::<i64>(),
        0,
    )
}

#[test]
fn independent_score_and_candidate_budget_prefixes_hold_in_all_six_paths() {
    for kind in KINDS {
        for stream in [false, true] {
            for scores in 0..=24 {
                for moves in 0..=16 {
                    let c = RuinRecreateConfig {
                        max_evaluations: scores,
                        max_move_evaluations: Some(moves),
                        ..config()
                    };
                    let r = run(kind, stream, false, c, quality);
                    // Each full repair scores one partial initial state, then four basic/mixed
                    // candidates or five list insertions. Only the third and later candidates
                    // finish both required assignments; the fourth reaches hard score zero.
                    let per = if kind == Kind::Lists { 5 } else { 4 };
                    let mut used = u64::from(scores > 0);
                    let mut trials = 0;
                    let mut attempts = 0;
                    let mut complete = 0;
                    let mut hard = -2;
                    while used < scores && trials < moves && attempts < 3 {
                        attempts += 1;
                        used += 1;
                        let n = (scores - used).min(moves - trials).min(per);
                        used += n;
                        trials += n;
                        if n >= 3 {
                            complete += 1;
                            hard = hard.max(if n >= 4 { 0 } else { -1 });
                        }
                    }
                    let reason = if used >= scores {
                        RuinRecreateTermination::EvaluationLimit
                    } else if trials >= moves {
                        RuinRecreateTermination::MoveEvaluationLimit
                    } else {
                        RuinRecreateTermination::AttemptLimit
                    };
                    assert_eq!(
                        (
                            r.evaluations,
                            r.repair_move_evaluations,
                            r.attempts,
                            r.complete_reconstructions,
                            r.termination
                        ),
                        (used, trials, attempts, complete, reason),
                        "{kind:?} stream={stream} scores={scores} moves={moves}"
                    );
                    assert_eq!(
                        r.best.as_ref().map(|b| b.1),
                        (scores > 0).then_some(HardSoftScore::new(hard, 0))
                    );
                }
            }
        }
    }
}

#[test]
fn targets_and_feasibility_stop_at_complete_attempt_boundaries() {
    for kind in KINDS {
        for stream in [false, true] {
            for target in [-3, -2, -1, 0, 1] {
                let r = run(
                    kind,
                    stream,
                    false,
                    RuinRecreateConfig {
                        best_score_limit: Some(HardSoftScore::new(target, 0)),
                        ..config()
                    },
                    quality,
                );
                assert_eq!(
                    r.attempts,
                    if target <= -2 {
                        0
                    } else if target <= 0 {
                        1
                    } else {
                        3
                    }
                );
                assert_eq!(
                    r.termination,
                    if target <= 0 {
                        RuinRecreateTermination::BestScoreLimit
                    } else {
                        RuinRecreateTermination::AttemptLimit
                    }
                );
                assert_eq!(
                    r.best.unwrap().1,
                    HardSoftScore::new(if target <= -2 { -2 } else { 0 }, 0)
                );
            }
            let r = run(
                kind,
                stream,
                false,
                RuinRecreateConfig {
                    stop_when_feasible: true,
                    ..config()
                },
                quality,
            );
            assert_eq!(r.termination, RuinRecreateTermination::FeasibleScore);
            assert_eq!(r.attempts, 1);
            assert_eq!(r.best.unwrap().1, HardSoftScore::ZERO);
            let r = run(
                kind,
                stream,
                false,
                RuinRecreateConfig {
                    stop_when_feasible: true,
                    ..config()
                },
                |_| HardSoftScore::new(0, -999),
            );
            assert_eq!(r.termination, RuinRecreateTermination::FeasibleScore);
            assert_eq!(r.attempts, 0);
        }
    }
}

#[test]
fn reached_goals_do_not_override_existing_resource_precedence() {
    for kind in KINDS {
        for stream in [false, true] {
            let base = RuinRecreateConfig {
                best_score_limit: Some(HardSoftScore::new(-2, 0)),
                stop_when_feasible: true,
                ..config()
            };
            for (c, reason) in [
                (
                    RuinRecreateConfig {
                        max_evaluations: 0,
                        ..base.clone()
                    },
                    RuinRecreateTermination::EvaluationLimit,
                ),
                (
                    RuinRecreateConfig {
                        max_time: Some(Duration::ZERO),
                        max_evaluations: 1,
                        ..base.clone()
                    },
                    RuinRecreateTermination::TimeLimit,
                ),
                (
                    RuinRecreateConfig {
                        max_evaluations: 1,
                        max_move_evaluations: Some(0),
                        ..base.clone()
                    },
                    RuinRecreateTermination::EvaluationLimit,
                ),
                (
                    RuinRecreateConfig {
                        max_move_evaluations: Some(0),
                        max_attempts: 0,
                        ..base.clone()
                    },
                    RuinRecreateTermination::MoveEvaluationLimit,
                ),
                (
                    RuinRecreateConfig {
                        max_attempts: 0,
                        ..base.clone()
                    },
                    RuinRecreateTermination::AttemptLimit,
                ),
                (base.clone(), RuinRecreateTermination::BestScoreLimit),
            ] {
                let r = run(kind, stream, false, c, quality);
                assert_eq!(r.termination, reason);
                assert_eq!(r.attempts, 0);
            }
            let token = CancellationToken::new();
            token.cancel();
            let r = run(
                kind,
                stream,
                false,
                RuinRecreateConfig {
                    cancellation: Some(&token),
                    max_time: Some(Duration::ZERO),
                    ..base
                },
                quality,
            );
            assert_eq!(r.termination, RuinRecreateTermination::Cancelled);
            assert!(r.best.is_none());
            assert_eq!(r.evaluations, 0);
        }
    }
}

#[test]
fn optional_complete_repairs_survive_move_budget_exhaustion() {
    for kind in KINDS {
        for stream in [false, true] {
            for moves in 1..=6 {
                let r = run(
                    kind,
                    stream,
                    true,
                    RuinRecreateConfig {
                        max_move_evaluations: Some(moves),
                        ..config()
                    },
                    |v| SimpleScore::new(-(v.iter().flatten().count() as i64)),
                );
                assert_eq!(r.termination, RuinRecreateTermination::MoveEvaluationLimit);
                assert_eq!(r.repair_move_evaluations, moves);
                assert_eq!(r.best.unwrap().1, SimpleScore::ZERO);
                assert!(r.complete_reconstructions > 0);
            }
        }
    }
}

#[test]
fn owned_decimal_targets_use_exact_order_and_hard_feasibility() {
    let old: HardSoftDecimalScore = "-0.000000000000000002hard/9007199254740992soft"
        .parse()
        .unwrap();
    let new: HardSoftDecimalScore = "-0.000000000000000001hard/9007199254740992soft"
        .parse()
        .unwrap();
    for kind in KINDS {
        for stream in [false, true] {
            let r = run(
                kind,
                stream,
                false,
                RuinRecreateConfig {
                    best_score_limit: Some(new.clone()),
                    stop_when_feasible: true,
                    ..config()
                },
                |v| {
                    if v.iter().all(|v| *v == Some(1)) {
                        new.clone()
                    } else {
                        old.clone()
                    }
                },
            );
            // Each first assignment ties and takes zero, so reaching this target is
            // impossible for this greedy reconstruction; negative hard stays infeasible.
            assert_eq!(r.termination, RuinRecreateTermination::AttemptLimit);
            assert_eq!(r.best.unwrap().1, old);
            let r = run(
                kind,
                stream,
                false,
                RuinRecreateConfig {
                    best_score_limit: Some(new.clone()),
                    stop_when_feasible: true,
                    ..config()
                },
                |v| {
                    if v.contains(&Some(1)) {
                        new.clone()
                    } else {
                        old.clone()
                    }
                },
            );
            assert_eq!(r.termination, RuinRecreateTermination::BestScoreLimit);
            assert_eq!(r.attempts, 1);
            assert_eq!(r.best.unwrap().1, new);
        }
    }
}

#[test]
fn completed_goal_candidates_remain_retained_when_resource_limits_win() {
    for kind in KINDS {
        for stream in [false, true] {
            for moves in [3, 4] {
                let r = run(
                    kind,
                    stream,
                    false,
                    RuinRecreateConfig {
                        max_move_evaluations: Some(moves),
                        best_score_limit: Some(HardSoftScore::new(-1, 0)),
                        stop_when_feasible: true,
                        ..config()
                    },
                    quality,
                );
                assert_eq!(r.termination, RuinRecreateTermination::MoveEvaluationLimit);
                assert_eq!(r.attempts, 1);
                assert_eq!(r.complete_reconstructions, 1);
                assert_eq!(
                    r.best.unwrap().1,
                    HardSoftScore::new(if moves == 3 { -1 } else { 0 }, 0)
                );
            }
        }
    }
}

#[test]
fn every_placement_and_early_pick_obeys_the_shared_candidate_cap() {
    for kind in KINDS {
        for stream in [false, true] {
            for placement in [
                ConstructionPlacement::Sequential,
                ConstructionPlacement::Pooled,
                ConstructionPlacement::ValueQueue,
            ] {
                for pick_early in [
                    ConstructionPickEarly::Never,
                    ConstructionPickEarly::FirstNonDeterioratingScore,
                    ConstructionPickEarly::FirstFeasibleScore,
                ] {
                    for moves in 0..=12 {
                        let r = run(
                            kind,
                            stream,
                            false,
                            RuinRecreateConfig {
                                max_move_evaluations: Some(moves),
                                placement,
                                pick_early,
                                ..config()
                            },
                            quality,
                        );
                        assert!(r.repair_move_evaluations <= moves);
                        assert_eq!(r.evaluations, 1 + r.attempts + r.repair_move_evaluations);
                        assert_eq!(
                            r.termination,
                            if r.repair_move_evaluations == moves {
                                RuinRecreateTermination::MoveEvaluationLimit
                            } else {
                                RuinRecreateTermination::AttemptLimit
                            }
                        );
                        assert!(r.best.unwrap().1 >= quality(&[Some(0), Some(0)]));
                    }
                }
            }
        }
    }
}

#[test]
fn repair_observers_preserve_results_counters_and_strict_event_prefixes() {
    for kind in KINDS {
        for stream in [false, true] {
            for scores in 0..=20 {
                for moves in 0..=12 {
                    let c = RuinRecreateConfig {
                        max_evaluations: scores,
                        max_move_evaluations: Some(moves),
                        ..config()
                    };
                    let plain = run(kind, stream, false, c.clone(), quality);
                    let mut events = vec![];
                    let observed = run_observed(
                        kind,
                        stream,
                        false,
                        c,
                        quality,
                        Some(&mut |v, s| events.push((v, s))),
                    );
                    assert_eq!(plain, observed);
                    for (v, s) in &events {
                        assert_eq!(*s, quality(v));
                        assert!(v.iter().all(Option::is_some));
                    }
                    assert!(events.windows(2).all(|w| w[0].1 < w[1].1));
                    assert_eq!(events.last(), observed.best.as_ref());
                    assert_eq!(
                        events.len(),
                        if scores == 0 {
                            0
                        } else {
                            1 + usize::from(
                                observed.best.as_ref().unwrap().1 > quality(&[Some(0), Some(0)]),
                            )
                        }
                    );
                    if scores > 0 {
                        assert_eq!(
                            events[0],
                            (vec![Some(0), Some(0)], quality(&[Some(0), Some(0)]))
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn cancelling_from_initial_or_improved_notification_keeps_that_solution() {
    for kind in KINDS {
        for stream in [false, true] {
            for cancel_after in [1, 2] {
                let token = CancellationToken::new();
                let mut events = vec![];
                let r = run_observed(
                    kind,
                    stream,
                    false,
                    RuinRecreateConfig {
                        cancellation: Some(&token),
                        ..config()
                    },
                    quality,
                    Some(&mut |v, s| {
                        events.push((v, s));
                        if events.len() == cancel_after {
                            token.cancel();
                        }
                    }),
                );
                assert_eq!(r.termination, RuinRecreateTermination::Cancelled);
                assert_eq!(r.attempts, if cancel_after == 1 { 0 } else { 1 });
                assert_eq!(events.len(), cancel_after);
                assert_eq!(events.last(), r.best.as_ref());
                assert_eq!(
                    r.best.unwrap().1,
                    HardSoftScore::new(if cancel_after == 1 { -2 } else { 0 }, 0)
                );
            }
        }
    }
}

#[test]
fn callback_time_counts_and_owned_decimal_notifications_do_not_round() {
    let old: HardSoftDecimalScore = "-0.000000000000000002hard/0soft".parse().unwrap();
    let new: HardSoftDecimalScore = "-0.000000000000000001hard/0soft".parse().unwrap();
    for kind in KINDS {
        for stream in [false, true] {
            let mut events = vec![];
            let r = run_observed(
                kind,
                stream,
                false,
                config(),
                |v| {
                    if v.contains(&Some(1)) {
                        new.clone()
                    } else {
                        old.clone()
                    }
                },
                Some(&mut |v, s| events.push((v, s))),
            );
            assert_eq!(
                events.iter().map(|e| e.1.clone()).collect::<Vec<_>>(),
                vec![old.clone(), new.clone()]
            );
            assert_eq!(events.last(), r.best.as_ref());
            let mut count = 0;
            let r = run_observed(
                kind,
                stream,
                false,
                RuinRecreateConfig {
                    max_time: Some(Duration::from_millis(1)),
                    ..config()
                },
                quality,
                Some(&mut |_, _| {
                    count += 1;
                    std::thread::sleep(Duration::from_millis(3));
                }),
            );
            assert_eq!(r.termination, RuinRecreateTermination::TimeLimit);
            assert_eq!(count, 1);
            assert_eq!(r.attempts, 0);
            assert_eq!(r.evaluations, 1);
        }
    }
}

#[test]
fn skipped_scoring_emits_nothing_and_observer_panics_propagate() {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    for kind in KINDS {
        for stream in [false, true] {
            let token = CancellationToken::new();
            token.cancel();
            for c in [
                RuinRecreateConfig {
                    max_evaluations: 0,
                    ..config()
                },
                RuinRecreateConfig {
                    cancellation: Some(&token),
                    ..config()
                },
            ] {
                let r = run_observed(
                    kind,
                    stream,
                    false,
                    c,
                    quality,
                    Some(&mut |_, _| panic!("skipped scoring must not notify")),
                );
                assert!(r.best.is_none());
            }
            for panic_after in [1, 2] {
                let mut count = 0;
                let error = catch_unwind(AssertUnwindSafe(|| {
                    run_observed(
                        kind,
                        stream,
                        false,
                        config(),
                        quality,
                        Some(&mut |_, _| {
                            count += 1;
                            if count == panic_after {
                                panic!("observer failure");
                            }
                        }),
                    )
                }));
                assert!(error.is_err());
                assert_eq!(count, panic_after);
            }
            // A callback failure leaves no thread-local search/graph state behind.
            let r = run(kind, stream, false, config(), quality);
            assert_eq!(r.best.unwrap().1, HardSoftScore::ZERO);
        }
    }
}

#[test]
fn sampled_improvements_notify_in_order_and_optional_repairs_notify_complete_nulls() {
    for kind in KINDS {
        for stream in [false, true] {
            for seed in 0..8 {
                let mut events = vec![];
                let r = run_observed(
                    kind,
                    stream,
                    false,
                    RuinRecreateConfig {
                        seed,
                        minimum_ruined: 1,
                        maximum_ruined: 1,
                        max_attempts: 30,
                        ..config()
                    },
                    quality,
                    Some(&mut |v, s| events.push((v, s))),
                );
                assert_eq!(
                    events.iter().map(|v| v.1).collect::<Vec<_>>(),
                    vec![
                        HardSoftScore::new(-2, 0),
                        HardSoftScore::new(-1, 0),
                        HardSoftScore::ZERO
                    ]
                );
                assert_eq!(events.last(), r.best.as_ref());
            }
            let mut events = vec![];
            let r = run_observed(
                kind,
                stream,
                true,
                RuinRecreateConfig {
                    max_move_evaluations: Some(1),
                    ..config()
                },
                |v| SimpleScore::new(-(v.iter().flatten().count() as i64)),
                Some(&mut |v, s| events.push((v, s))),
            );
            assert_eq!(r.termination, RuinRecreateTermination::MoveEvaluationLimit);
            assert_eq!(
                events,
                vec![
                    (vec![Some(0), Some(0)], SimpleScore::new(-2)),
                    (vec![None, None], SimpleScore::ZERO)
                ]
            );
            assert_eq!(events.last(), r.best.as_ref());
        }
    }
}

#[test]
fn weighted_moves_share_budgets_scores_and_seeded_results_in_every_backend() {
    for kind in KINDS {
        for optional in [false, true] {
            for seed in 0..4 {
                for acceptance in [
                    Acceptance::HillClimbing,
                    Acceptance::VariableTabu {
                        tenure: 2,
                        aspiration: true,
                    },
                    Acceptance::MoveTabu {
                        tenure: 2,
                        aspiration: true,
                    },
                    Acceptance::LateAcceptance {
                        history_size: 3,
                        hill_climbing: true,
                    },
                ] {
                    for budget in 0..=12 {
                        let c = RuinRecreateConfig {
                            seed,
                            max_attempts: 8,
                            max_evaluations: budget,
                            max_move_evaluations: Some(budget % 7),
                            acceptance,
                            move_weights: RepairMoveWeights {
                                change: 2,
                                swap: 2,
                                repair: 1,
                            },
                            ..config()
                        };
                        let a = run(kind, false, optional, c.clone(), quality);
                        let b = run(kind, true, optional, c.clone(), quality);
                        assert_eq!(
                            (
                                a.best.clone(),
                                a.evaluations,
                                a.repair_move_evaluations,
                                a.fine_move_evaluations,
                                a.attempts,
                                a.fine_move_attempts,
                                a.complete_reconstructions,
                                a.accepted_moves,
                                a.termination
                            ),
                            (
                                b.best,
                                b.evaluations,
                                b.repair_move_evaluations,
                                b.fine_move_evaluations,
                                b.attempts,
                                b.fine_move_attempts,
                                b.complete_reconstructions,
                                b.accepted_moves,
                                b.termination
                            )
                        );
                        assert!(a.evaluations <= budget);
                        assert!(a.repair_move_evaluations + a.fine_move_evaluations <= budget % 7);
                        assert!(a.attempts + a.fine_move_attempts <= 8);
                        assert_eq!(a.fine_move_attempts, a.fine_move_evaluations);
                        assert_eq!(
                            a.evaluations,
                            u64::from(budget > 0)
                                + a.attempts
                                + a.repair_move_evaluations
                                + a.fine_move_evaluations
                        );
                        assert!(
                            a.accepted_moves <= a.fine_move_attempts + a.complete_reconstructions
                        );
                        if let Some((_, s)) = a.best {
                            assert!(s >= HardSoftScore::new(-2, 0) && s <= HardSoftScore::ZERO);
                        }
                    }
                }
            }
        }
    }
}
#[test]
fn fine_only_sampling_preserves_owned_scores_and_strict_notifications() {
    let low: HardSoftDecimalScore = "-0.000000000000000002hard/9007199254740992soft"
        .parse()
        .unwrap();
    let high: HardSoftDecimalScore = "-0.000000000000000001hard/9007199254740992soft"
        .parse()
        .unwrap();
    for kind in KINDS {
        for stream in [false, true] {
            let mut events = Vec::new();
            let r = run_observed(
                kind,
                stream,
                false,
                RuinRecreateConfig {
                    max_attempts: 100,
                    seed: 42,
                    best_score_limit: Some(high.clone()),
                    move_weights: RepairMoveWeights {
                        change: 1,
                        swap: 0,
                        repair: 0,
                    },
                    ..config()
                },
                |v| {
                    if v.contains(&Some(1)) {
                        high.clone()
                    } else {
                        low.clone()
                    }
                },
                Some(&mut |_, s| events.push(s)),
            );
            assert_eq!(r.termination, RuinRecreateTermination::BestScoreLimit);
            assert_eq!(events, vec![low.clone(), high.clone()]);
            assert_eq!(r.best.unwrap().1, high);
            assert_eq!(
                (
                    r.attempts,
                    r.complete_reconstructions,
                    r.repair_move_evaluations
                ),
                (0, 0, 0)
            );
            assert_eq!(r.evaluations, 1 + r.fine_move_evaluations);
        }
    }
}
