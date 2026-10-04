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
    let p = problem(kind, optional);
    let result = match (kind, stream) {
        (Kind::Basic, false) => {
            let mut n = ConstraintNetwork::new(2);
            n.add_constraint("quality", vec![0, 1], |v| {
                score(&v.iter().map(|v| v.copied()).collect::<Vec<_>>())
            })
            .unwrap();
            map(
                ruin_recreate_search_from(p.basic(), &n, p.basic().initial_assignment(), c)
                    .unwrap(),
                |b| (b.assignment.values().to_vec(), b.score),
            )
        }
        (Kind::Basic, true) => map(
            ruin_recreate_stream_search_from(
                p.basic(),
                || graph(kind, &score).map(|(g, s)| (g, s.basic)),
                p.basic().initial_assignment(),
                c,
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
                ruin_recreate_list_search_from(p.lists(), &n, p.lists().initial_assignment(), c)
                    .unwrap(),
                |b| (project(kind, &[], b.assignment.lists()), b.score),
            )
        }
        (Kind::Lists, true) => map(
            ruin_recreate_list_stream_search_from(
                p.lists(),
                || graph(kind, &score).map(|(g, s)| (g, s.lists)),
                p.lists().initial_assignment(),
                c,
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
                ruin_recreate_mixed_search_from(
                    &p,
                    &n,
                    p.initial_assignment(),
                    MixedRuinRecreateConfig {
                        search: c,
                        ..Default::default()
                    },
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
            ruin_recreate_mixed_stream_search_from(
                &p,
                || graph(kind, &score),
                p.initial_assignment(),
                MixedRuinRecreateConfig {
                    search: c,
                    ..Default::default()
                },
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
