// Original selection-reference, failure-atomicity and solver integration checks.
use std::{
    cell::Cell,
    collections::BTreeSet,
    num::NonZeroU64,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
const MODES: [MoveSelection; 4] = [
    MoveSelection::FirstAccepted,
    MoveSelection::BestAccepted,
    MoveSelection::FirstBestScoreImproving,
    MoveSelection::FirstLastStepScoreImproving,
];
fn config(
    mode: MoveSelection,
    limit: Option<u64>,
    ties: TieBreaking,
    fallback: RejectedMovePolicy,
) -> ForagerConfig<HardSoftScore> {
    ForagerConfig {
        selection: mode,
        ranking: FinalistRanking::HighestScore,
        current_score: HardSoftScore::new(0, -3),
        best_score: HardSoftScore::new(0, 1),
        accepted_candidate_limit: limit.and_then(NonZeroU64::new),
        tie_breaking: ties,
        rejected_moves: fallback,
    }
}
#[test]
fn generated_prefixes_match_independent_priority_ties_and_stop_reference() {
    let mut seed = 20261218_u64;
    for _ in 0..256 {
        let rows: Vec<_> = (0..12)
            .map(|id| {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                (
                    id,
                    HardSoftScore::new(((seed >> 8) % 3) as i64 - 1, ((seed >> 16) % 9) as i64 - 5)
                        .with_structural(if seed.is_multiple_of(7) { -1 } else { 0 }),
                    seed & 1 == 0,
                )
            })
            .collect();
        for mode in MODES {
            for limit in [None, Some(1), Some(3), Some(u64::MAX)] {
                for fallback in [RejectedMovePolicy::Stop, RejectedMovePolicy::BestScore] {
                    let cfg = config(mode, limit, TieBreaking::Random { seed: 31 }, fallback);
                    let mut prefix = vec![];
                    let mut accepted = 0;
                    let mut stops = vec![];
                    for &(id, score, ok) in &rows {
                        if ok {
                            accepted += 1;
                        }
                        if ok
                            || (fallback == RejectedMovePolicy::BestScore
                                && score.structural() >= 0)
                        {
                            prefix.push((id, score, ok));
                        }
                        let quit = ok
                            && (limit.is_some_and(|n| accepted >= n)
                                || match mode {
                                    MoveSelection::FirstAccepted => true,
                                    MoveSelection::BestAccepted => false,
                                    MoveSelection::FirstBestScoreImproving => {
                                        score > cfg.best_score
                                    }
                                    MoveSelection::FirstLastStepScoreImproving => {
                                        score > cfg.current_score
                                    }
                                });
                        stops.push(quit);
                        if quit {
                            break;
                        }
                    }
                    let rank = prefix.iter().map(|(_, s, a)| (*a, *s)).max();
                    let expected: Vec<_> = prefix
                        .iter()
                        .filter(|(_, s, a)| Some((*a, *s)) == rank)
                        .map(|(id, _, _)| *id)
                        .collect();
                    for index in 0..expected.len().max(1) {
                        let mut f = MoveForager::new(cfg);
                        for (&(id, s, a), &quit) in rows.iter().zip(&stops) {
                            f.add(s, a, id);
                            assert_eq!(f.is_quit_early(), quit);
                        }
                        assert_eq!(f.accepted_candidates(), u128::from(accepted));
                        assert_eq!(f.finalist_count(), expected.len());
                        assert_eq!(
                            f.pick_with(|count| {
                                assert_eq!(count, expected.len());
                                index
                            })
                            .unwrap(),
                            expected.get(index).copied()
                        );
                    }
                }
            }
        }
    }
}
#[test]
fn finalists_are_lazy_and_save_failure_preserves_selection_and_stop_state() {
    let mut f = MoveForager::new(config(
        MoveSelection::BestAccepted,
        Some(2),
        TieBreaking::Random { seed: 0 },
        RejectedMovePolicy::BestScore,
    ));
    f.add(HardSoftScore::ZERO, true, Box::new(1));
    let saved = Cell::new(0);
    let result = f.try_add_with(HardSoftScore::ZERO, true, || {
        saved.set(saved.get() + 1);
        Err::<Box<i32>, _>("save failure")
    });
    assert_eq!(result, Err("save failure"));
    assert_eq!(f.accepted_candidates(), 1);
    assert!(!f.is_quit_early());
    assert_eq!(f.finalist_count(), 1);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _: Result<(), ()> =
                f.try_add_with(HardSoftScore::new(1, 0), true, || panic!("save panic"));
        }))
        .is_err()
    );
    assert_eq!(f.accepted_candidates(), 1);
    assert!(!f.is_quit_early());
    f.try_add_with(HardSoftScore::new(-1, 0), true, || {
        saved.set(saved.get() + 1);
        Ok::<_, ()>(Box::new(2))
    })
    .unwrap();
    assert_eq!(saved.get(), 1, "inferior candidate must not be captured");
    assert!(f.is_quit_early());
    assert_eq!(f.pick().map(|v| *v), Some(1));
}
#[test]
fn seeded_ties_repeat_cover_finalists_and_validate_supplied_indices() {
    fn make(ties: TieBreaking) -> MoveForager<usize, HardSoftScore> {
        let mut f = MoveForager::new(config(
            MoveSelection::BestAccepted,
            None,
            ties,
            RejectedMovePolicy::BestScore,
        ));
        // Accepted values replace all better-scoring rejected finalists.
        for i in 0..3 {
            f.add(HardSoftScore::new(2, 0), false, i + 100);
        }
        for i in 0..4 {
            f.add(HardSoftScore::ZERO, true, i);
        }
        f
    }
    let mut seen = BTreeSet::new();
    for seed in 0..128 {
        let a = make(TieBreaking::Random { seed }).pick();
        assert_eq!(a, make(TieBreaking::Random { seed }).pick());
        seen.insert(a.unwrap());
    }
    assert_eq!(seen, BTreeSet::from([0, 1, 2, 3]));
    assert_eq!(make(TieBreaking::First).finalist_count(), 1);
    assert_eq!(
        make(TieBreaking::First)
            .pick_with(|_| panic!("unused draw"))
            .unwrap(),
        Some(0)
    );
    assert_eq!(
        make(TieBreaking::Random { seed: 0 }).pick_with(|_| 4),
        Err(InvalidFinalistIndex {
            index: 4,
            finalist_count: 4
        })
    );
    let empty = MoveForager::<usize, _>::new(config(
        MoveSelection::BestAccepted,
        None,
        TieBreaking::Random { seed: 0 },
        RejectedMovePolicy::Stop,
    ));
    assert_eq!(empty.pick_with(|_| panic!("empty draw")).unwrap(), None);
}
fn reward(v: usize) -> i64 {
    [0, 1, 2, 2][v]
}
fn basic(config: LocalSearchConfig) -> LocalSearchResult<usize, SimpleScore> {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2, 3]).with_initial(0)]).unwrap();
    let mut n = ConstraintNetwork::new(1);
    n.add_constraint("reward", vec![0], |v| {
        SimpleScore::new(reward(*v[0].unwrap()))
    })
    .unwrap();
    let a = local_search_from(&p, &n, p.initial_assignment(), config).unwrap();
    let mut b = StreamGraphBuilder::new();
    let s = b.source::<usize, (usize, Option<usize>)>();
    b.score("reward", s.port(), |(_, v)| {
        SimpleScore::new(reward(v.unwrap()))
    })
    .unwrap();
    let c = stream_local_search_from(&p, b.build(), s, p.initial_assignment(), config).unwrap();
    assert_eq!(a.best, c.best);
    assert_eq!(a.evaluations, c.evaluations);
    assert_eq!(a.completed_steps, c.completed_steps);
    assert_eq!(a.accepted_moves, c.accepted_moves);
    assert_eq!(a.termination, c.termination);
    if let Some(best) = &a.best {
        assert_eq!(
            best.score,
            SimpleScore::new(reward(best.assignment.values()[0].unwrap()))
        );
    }
    a
}
fn lists() -> ListProblem<usize> {
    ListProblem::new(
        vec![7],
        vec![
            ListEntity::new(vec![7]),
            ListEntity::new(vec![]),
            ListEntity::new(vec![]),
            ListEntity::new(vec![]),
        ],
    )
    .unwrap()
}
fn list(config: LocalSearchConfig) -> ListLocalSearchResult<usize, SimpleScore> {
    let p = lists();
    let mut n = ListConstraintNetwork::new(4, 1);
    n.add_constraint("reward", vec![ListScope::Value(0)], |f| {
        let ListFact::Value {
            element: Some(v), ..
        } = &f[0]
        else {
            panic!()
        };
        SimpleScore::new(reward(v.position.entity))
    })
    .unwrap();
    let a = list_local_search_from(&p, &n, p.initial_assignment(), config).unwrap();
    let mut b = StreamGraphBuilder::new();
    let s = ListStreamSources::new(&mut b);
    b.score("reward", s.values.port(), |v| {
        SimpleScore::new(v.position.map_or(0, |p| reward(p.entity)))
    })
    .unwrap();
    let c =
        list_stream_local_search_from(&p, b.build(), s, p.initial_assignment(), config).unwrap();
    assert_eq!(a.best, c.best);
    assert_eq!(a.evaluations, c.evaluations);
    assert_eq!(a.completed_steps, c.completed_steps);
    assert_eq!(a.accepted_moves, c.accepted_moves);
    assert_eq!(a.termination, c.termination);
    if let Some(best) = &a.best {
        assert_eq!(
            best.score,
            SimpleScore::new(reward(best.assignment.location(&7).unwrap().entity))
        );
    }
    a
}
fn mixed(config: LocalSearchConfig) -> MixedLocalSearchResult<usize, usize, SimpleScore> {
    let p = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![0, 1, 2, 3]).with_initial(0)]).unwrap(),
        lists(),
    );
    let mut n = MixedConstraintNetwork::new(1, 4, 1);
    n.add_constraint("basic", vec![MixedScope::Basic(0)], |f| {
        let MixedFact::Basic(v) = f[0] else { panic!() };
        SimpleScore::new(reward(*v.unwrap()))
    })
    .unwrap();
    n.add_constraint("list", vec![MixedScope::ListValue(0)], |f| {
        let MixedFact::ListValue {
            element: Some(v), ..
        } = &f[0]
        else {
            panic!()
        };
        SimpleScore::new(reward(v.position.entity))
    })
    .unwrap();
    let cfg = MixedLocalSearchConfig {
        search: config,
        joint_moves: true,
    };
    let a = mixed_local_search_from(&p, &n, p.initial_assignment(), cfg).unwrap();
    let mut b = StreamGraphBuilder::new();
    let s = MixedStreamSources::new(&mut b);
    b.score("basic", s.basic.port(), |(_, v)| {
        SimpleScore::new(reward(v.unwrap()))
    })
    .unwrap();
    b.score("list", s.lists.values.port(), |v| {
        SimpleScore::new(v.position.map_or(0, |p| reward(p.entity)))
    })
    .unwrap();
    let c = mixed_stream_local_search_from(&p, b.build(), s, p.initial_assignment(), cfg).unwrap();
    assert_eq!(a.best, c.best);
    assert_eq!(a.evaluations, c.evaluations);
    assert_eq!(a.completed_steps, c.completed_steps);
    assert_eq!(a.accepted_moves, c.accepted_moves);
    assert_eq!(a.termination, c.termination);
    if let Some(best) = &a.best {
        assert_eq!(
            best.score,
            SimpleScore::new(
                reward(best.assignment.basic().values()[0].unwrap())
                    + reward(best.assignment.lists().location(&7).unwrap().entity)
            )
        );
    }
    a
}
#[test]
fn all_six_backends_match_across_limits_modes_orders_and_rejected_fallback() {
    for selection in MODES {
        for limit in [None, NonZeroU64::new(1), NonZeroU64::new(2)] {
            for ties in [TieBreaking::First, TieBreaking::Random { seed: 29 }] {
                for budget in [0, 1, 2, 4, 100] {
                    for order in [
                        MoveOrder::Original,
                        MoveOrder::ShuffledMoves {
                            seed: 11,
                            window: 3.try_into().unwrap(),
                        },
                    ] {
                        let config = LocalSearchConfig {
                            selection,
                            accepted_candidate_limit: limit,
                            tie_breaking: ties,
                            max_evaluations: budget,
                            max_steps: 3,
                            move_order: order,
                            acceptance: Acceptance::VariableTabu {
                                tenure: 2,
                                aspiration: true,
                            },
                            rejected_moves: RejectedMovePolicy::BestScore,
                            assert_scores: true,
                            ..Default::default()
                        };
                        assert!(basic(config).evaluations <= budget);
                        assert!(list(config).evaluations <= budget);
                        assert!(mixed(config).evaluations <= budget);
                    }
                }
            }
        }
    }
}
#[test]
fn early_limits_change_actual_selected_candidates_and_do_not_replay_scores() {
    let base = LocalSearchConfig {
        max_steps: 1,
        max_evaluations: 100,
        acceptance: Acceptance::HillClimbing,
        selection: MoveSelection::BestAccepted,
        assert_scores: false,
        ..Default::default()
    };
    for selection in MODES {
        for limit in [None, NonZeroU64::new(1), NonZeroU64::new(2)] {
            let config = LocalSearchConfig {
                selection,
                accepted_candidate_limit: limit,
                ..base
            };
            let early = selection != MoveSelection::BestAccepted || limit == NonZeroU64::new(1);
            let expected = if early { 1 } else { 2 };
            let a = basic(config);
            let b = list(config);
            let c = mixed(config);
            assert_eq!(a.best.unwrap().score, SimpleScore::new(expected));
            assert_eq!(b.best.unwrap().score, SimpleScore::new(expected));
            assert_eq!(
                c.best.unwrap().score,
                SimpleScore::new(if !early && limit.is_none() {
                    4
                } else {
                    expected
                })
            );
            let evaluations = if early {
                2
            } else if limit.is_some() {
                3
            } else {
                4
            };
            assert_eq!(a.evaluations, evaluations);
            assert_eq!(b.evaluations, evaluations);
            assert_eq!(a.constraint_evaluations, u128::from(evaluations));
            assert_eq!(a.completed_steps, 1);
            assert_eq!(a.accepted_moves, 1);
        }
    }
    let mut selected = BTreeSet::new();
    for seed in 0..32 {
        let config = LocalSearchConfig {
            tie_breaking: TieBreaking::Random { seed },
            ..base
        };
        let a = basic(config);
        let b = list(config);
        let c = mixed(config);
        selected.insert(a.best.unwrap().assignment.values()[0].unwrap());
        assert_eq!(b.best.unwrap().score, SimpleScore::new(2));
        assert_eq!(c.best.unwrap().score, SimpleScore::new(4));
        assert_eq!(
            a.constraint_evaluations, 4,
            "saving and selecting ties must not rescore"
        );
    }
    assert_eq!(selected, BTreeSet::from([2, 3]));
}

#[test]
fn per_step_early_stop_and_phase_evaluation_limits_have_distinct_boundaries() {
    let base = LocalSearchConfig {
        max_steps: 1,
        max_evaluations: 3,
        selection: MoveSelection::BestAccepted,
        accepted_candidate_limit: NonZeroU64::new(2),
        acceptance: Acceptance::HillClimbing,
        ..Default::default()
    };
    // The second accepted candidate ends selection without probing a third.
    assert_eq!(basic(base).termination, LocalSearchTermination::StepLimit);
    assert_eq!(list(base).termination, LocalSearchTermination::StepLimit);
    assert_eq!(mixed(base).termination, LocalSearchTermination::StepLimit);
    let short = LocalSearchConfig {
        max_evaluations: 2,
        ..base
    };
    assert_eq!(
        basic(short).termination,
        LocalSearchTermination::EvaluationLimit
    );
    assert_eq!(
        list(short).termination,
        LocalSearchTermination::EvaluationLimit
    );
    assert_eq!(
        mixed(short).termination,
        LocalSearchTermination::EvaluationLimit
    );
    let per_move = LocalSearchConfig {
        max_evaluations: 100,
        max_move_evaluations: Some(1),
        ..base
    };
    assert_eq!(
        basic(per_move).termination,
        LocalSearchTermination::MoveEvaluationLimit
    );
    assert_eq!(
        list(per_move).termination,
        LocalSearchTermination::MoveEvaluationLimit
    );
    assert_eq!(
        mixed(per_move).termination,
        LocalSearchTermination::MoveEvaluationLimit
    );
}
