//! Original ranking, numeric, failure and six-backend integration checks.
use std::{
    cell::RefCell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;
const RANKINGS: [FinalistRanking; 3] = [
    FinalistRanking::HighestScore,
    FinalistRanking::StrategicOscillationLastStep,
    FinalistRanking::StrategicOscillationBest,
];
type S = HardMediumSoftScore;
fn s(a: i64, b: i64, c: i64) -> S {
    S::new(a, b, c)
}

fn choose(rows: &[(S, bool)], cfg: ForagerConfig<S>) -> usize {
    let eligible: Vec<_> = rows
        .iter()
        .enumerate()
        .filter(|(_, (s, a))| {
            *a || (cfg.rejected_moves == RejectedMovePolicy::BestScore && s.structural() >= 0)
        })
        .collect();
    let accepted = eligible.iter().any(|(_, (_, a))| *a);
    let mut pool: Vec<_> = eligible
        .into_iter()
        .filter(|(_, (_, a))| *a == accepted)
        .collect();
    let structural = pool.iter().map(|(_, (s, _))| s.structural()).max().unwrap();
    pool.retain(|(_, (s, _))| s.structural() == structural);
    if accepted && cfg.ranking != FinalistRanking::HighestScore {
        let r = if cfg.ranking == FinalistRanking::StrategicOscillationBest {
            cfg.best_score
        } else {
            cfg.current_score
        };
        if pool.iter().any(|(_, (s, _))| *s > r) {
            pool.retain(|(_, (s, _))| *s > r);
        } else {
            for level in 0..3 {
                if pool
                    .iter()
                    .any(|(_, (s, _))| s.levels()[level] > r.levels()[level])
                {
                    pool.retain(|(_, (s, _))| s.levels()[level] > r.levels()[level]);
                    break;
                }
            }
        }
    }
    let high = pool.iter().map(|(_, (s, _))| *s).max().unwrap();
    pool.into_iter().find(|(_, (s, _))| *s == high).unwrap().0
}
#[test]
fn generated_rankings_preserve_acceptance_structure_and_encounter_ties() {
    let mut seed = 20261220u64;
    for _ in 0..256 {
        let rows: Vec<_> = (0..16)
            .map(|i| {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                (
                    s(
                        (seed % 5) as i64 - 3,
                        ((seed >> 8) % 7) as i64 - 4,
                        ((seed >> 16) % 9) as i64 - 5,
                    )
                    .with_structural(if i % 5 == 0 { -1 } else { 0 }),
                    i % 3 != 0,
                )
            })
            .collect();
        for ranking in RANKINGS {
            let cfg = ForagerConfig {
                ranking,
                current_score: s(0, -3, -3),
                best_score: s(0, -1, -1),
                rejected_moves: RejectedMovePolicy::BestScore,
                ..Default::default()
            };
            for rotation in 0..rows.len() {
                let rows: Vec<_> = rows
                    .iter()
                    .cycle()
                    .skip(rotation)
                    .take(rows.len())
                    .copied()
                    .collect();
                let expected = choose(&rows, cfg);
                let mut f = MoveForager::new(cfg);
                for (i, (score, accepted)) in rows.iter().enumerate() {
                    f.add(*score, *accepted, i);
                }
                assert_eq!(f.pick(), Some(expected));
            }
        }
    }
}
#[test]
fn exact_reference_order_does_not_override_soft_improvement() {
    for ranking in &RANKINGS[1..] {
        for accepted in [false, true] {
            for rows in [[s(6, 0, 0), s(2, 8, -2)], [s(2, 8, -2), s(6, 0, 0)]] {
                let mut f = MoveForager::new(ForagerConfig {
                    ranking: *ranking,
                    current_score: s(6, 0, 0),
                    best_score: s(6, 0, 0),
                    rejected_moves: RejectedMovePolicy::BestScore,
                    ..Default::default()
                });
                for row in rows {
                    f.add(row, accepted, row);
                }
                assert_eq!(
                    f.pick(),
                    Some(if accepted { s(2, 8, -2) } else { s(6, 0, 0) })
                );
            }
        }
    }
}

#[test]
fn early_pick_overrides_strategic_rank_but_count_limit_does_not() {
    for limit_only in [false, true] {
        let mut f = MoveForager::new(ForagerConfig {
            ranking: FinalistRanking::StrategicOscillationBest,
            current_score: s(-5, 0, 0),
            best_score: s(0, 0, 0),
            selection: if limit_only {
                MoveSelection::BestAccepted
            } else {
                MoveSelection::FirstLastStepScoreImproving
            },
            accepted_candidate_limit: 2.try_into().ok(),
            ..Default::default()
        });
        f.add(s(-6, 2, 0), true, 'a');
        assert!(!f.is_quit_early());
        f.add(s(-4, -1, 0), true, 'b');
        assert!(f.is_quit_early());
        assert_eq!(f.pick(), Some(if limit_only { 'a' } else { 'b' }));
    }
}
#[test]
fn exact_decimal_levels_full_width_and_failed_capture_preserve_finalists() {
    let r: HardMediumSoftDecimalScore = "0hard/9007199254740992medium/0soft".parse().unwrap();
    let a: HardMediumSoftDecimalScore = "-1hard/9007199254740992.0medium/1soft".parse().unwrap();
    let b: HardMediumSoftDecimalScore = "-2hard/9007199254740992.00000000000000000001medium/-1soft"
        .parse()
        .unwrap();
    let mut f = MoveForager::new(ForagerConfig {
        ranking: FinalistRanking::StrategicOscillationBest,
        current_score: r.clone(),
        best_score: r,
        tie_breaking: TieBreaking::Random { seed: 0 },
        ..Default::default()
    });
    f.add(a, true, Box::new('a'));
    let result = f.try_add_with(b.clone(), true, || Err::<Box<char>, _>("capture failed"));
    assert_eq!(result, Err("capture failed"));
    assert_eq!(f.accepted_candidates(), 1);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _: Result<(), ()> = f.try_add_with(b.clone(), true, || panic!("capture panic"));
        }))
        .is_err()
    );
    assert_eq!(f.accepted_candidates(), 1);
    assert_eq!(f.finalist_count(), 1);
    f.add(b.clone(), true, Box::new('b'));
    f.add(b, true, Box::new('c'));
    assert_eq!(f.finalist_count(), 2);
    assert_eq!(f.pick_with(|_| 1).unwrap().map(|v| *v), Some('c'));
    let r = s(i64::MAX, i64::MIN, 0);
    let mut f = MoveForager::new(ForagerConfig {
        ranking: FinalistRanking::StrategicOscillationLastStep,
        current_score: r,
        best_score: r,
        ..Default::default()
    });
    f.add(s(i64::MAX - 1, i64::MIN, 1), true, 0);
    f.add(s(i64::MIN, i64::MIN + 1, -1), true, 1);
    assert_eq!(f.pick(), Some(1));
}

// Every model describes one movable choice. Record actual scoring callbacks to
// observe which candidate is absent from each next neighborhood (the saved step).
fn run_backend(mode: usize, cfg: LocalSearchConfig<S>, scores: &[S]) -> Vec<usize> {
    let records = Rc::new(RefCell::new(vec![]));
    let log = records.clone();
    let table = scores.to_vec();
    let score = move |v: usize| {
        log.borrow_mut().push(v);
        table[v]
    };
    let n = scores.len();
    let p = FiniteProblem::new(vec![Variable::new((0..n).collect()).with_initial(0)]).unwrap();
    if mode < 2 {
        let r = if mode == 0 {
            let mut network = ConstraintNetwork::new(1);
            network
                .add_constraint("table", vec![0], |v| score(*v[0].unwrap()))
                .unwrap();
            local_search_from(&p, &network, p.initial_assignment(), cfg.clone()).unwrap()
        } else {
            let mut b = StreamGraphBuilder::new();
            let src = b.source::<usize, (usize, Option<usize>)>();
            b.score("table", src.port(), |(_, v)| score(v.unwrap()))
                .unwrap();
            stream_local_search_from(&p, b.build(), src, p.initial_assignment(), cfg.clone())
                .unwrap()
        };
        assert_eq!(r.completed_steps, cfg.max_steps);
        assert_eq!(r.accepted_moves, cfg.max_steps);
        assert_eq!(
            r.best.as_ref().unwrap().score,
            scores[r.best.as_ref().unwrap().assignment.values()[0].unwrap()]
        );
    } else {
        let list = ListProblem::new(
            vec![7],
            (0..n)
                .map(|i| ListEntity::new(if i == 0 { vec![7] } else { vec![] }))
                .collect(),
        )
        .unwrap();
        if mode < 4 {
            let r = if mode == 2 {
                let mut network = ListConstraintNetwork::new(n, 1);
                network
                    .add_constraint("table", vec![ListScope::Value(0)], |v| {
                        let ListFact::Value {
                            element: Some(e), ..
                        } = &v[0]
                        else {
                            panic!()
                        };
                        score(e.position.entity)
                    })
                    .unwrap();
                list_local_search_from(&list, &network, list.initial_assignment(), cfg.clone())
                    .unwrap()
            } else {
                let mut b = StreamGraphBuilder::new();
                let src = ListStreamSources::<usize>::new(&mut b);
                b.score("table", src.values.port(), |v| {
                    score(v.position.unwrap().entity)
                })
                .unwrap();
                list_stream_local_search_from(
                    &list,
                    b.build(),
                    src,
                    list.initial_assignment(),
                    cfg.clone(),
                )
                .unwrap()
            };
            assert_eq!(r.completed_steps, cfg.max_steps);
            assert_eq!(r.accepted_moves, cfg.max_steps);
            assert_eq!(
                r.best.as_ref().unwrap().score,
                scores[r
                    .best
                    .as_ref()
                    .unwrap()
                    .assignment
                    .location(&7)
                    .unwrap()
                    .entity]
            );
        } else {
            let p = MixedProblem::new(
                FiniteProblem::new(vec![Variable::new(vec![0]).with_initial(0)]).unwrap(),
                list,
            );
            let settings = MixedLocalSearchConfig {
                search: cfg.clone(),
                joint_moves: false,
            };
            let r = if mode == 4 {
                let mut network = MixedConstraintNetwork::new(1, n, 1);
                network
                    .add_constraint("table", vec![MixedScope::ListValue(0)], |v| {
                        let MixedFact::ListValue {
                            element: Some(e), ..
                        } = &v[0]
                        else {
                            panic!()
                        };
                        score(e.position.entity)
                    })
                    .unwrap();
                mixed_local_search_from(&p, &network, p.initial_assignment(), settings).unwrap()
            } else {
                let mut b = StreamGraphBuilder::new();
                let src = MixedStreamSources::<usize, usize>::new(&mut b);
                b.score("table", src.lists.values.port(), |v| {
                    score(v.position.unwrap().entity)
                })
                .unwrap();
                mixed_stream_local_search_from(&p, b.build(), src, p.initial_assignment(), settings)
                    .unwrap()
            };
            assert_eq!(r.completed_steps, cfg.max_steps);
            assert_eq!(r.accepted_moves, cfg.max_steps);
            assert_eq!(
                r.best.as_ref().unwrap().score,
                scores[r
                    .best
                    .as_ref()
                    .unwrap()
                    .assignment
                    .lists()
                    .location(&7)
                    .unwrap()
                    .entity]
            );
        }
    }
    records.borrow().clone()
}
#[test]
fn all_six_backends_use_current_and_best_references_for_saved_steps() {
    let scores = [
        s(0, -4, -5),
        s(0, -4, -7),
        s(-2, 1, -2),
        s(-1, -1, -3),
        s(-1, -2, 2),
    ];
    for ranking in RANKINGS {
        let cfg = LocalSearchConfig {
            ranking,
            selection: MoveSelection::BestAccepted,
            max_steps: 4,
            neighborhood: Neighborhood::Change,
            acceptance: Acceptance::SimulatedAnnealing {
                starting_temperature: s(i64::MAX, i64::MAX, i64::MAX),
                cooling_steps: 100,
                seed: 23,
            },
            ..Default::default()
        };
        let mut expected = vec![0];
        let (mut current, mut best) = (0, 0);
        for _ in 0..cfg.max_steps {
            let ids: Vec<_> = (0..scores.len()).filter(|i| *i != current).collect();
            expected.extend(&ids);
            let rows: Vec<_> = ids.iter().map(|i| (scores[*i], true)).collect();
            current = ids[choose(
                &rows,
                ForagerConfig {
                    ranking,
                    current_score: scores[current],
                    best_score: scores[best],
                    ..Default::default()
                },
            )];
            if scores[current] > scores[best] {
                best = current;
            }
        }
        for mode in 0..6 {
            assert_eq!(
                run_backend(mode, cfg.clone(), &scores),
                expected,
                "mode {mode}, {ranking:?}"
            );
        }
    }
}
