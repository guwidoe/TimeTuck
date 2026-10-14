//! Original coverage-policy integration checks; no upstream method credit.
use timetuck::*;

mod basic {
    use super::*;
    pub(super) fn run_both(
        p: &FiniteProblem<i32>,
        c: RuinRecreateConfig<SimpleScore>,
        score: impl Fn(&[Option<i32>]) -> i64,
    ) -> Vec<RuinRecreateResult<i32, SimpleScore>> {
        let size = p.variables().len();
        let mut n = ConstraintNetwork::new(size);
        n.add_constraint("all", (0..size).collect(), |v| {
            SimpleScore::new(score(&v.iter().map(|v| v.copied()).collect::<Vec<_>>()))
        })
        .unwrap();
        let a = ruin_recreate_search_from(p, &n, p.initial_assignment(), c.clone()).unwrap();
        let b = ruin_recreate_stream_search_from(
            p,
            || {
                let mut b = StreamGraphBuilder::new();
                let s = b.source::<usize, (usize, Option<i32>)>();
                let all = b.collect_list(s.port(), |row| *row)?;
                b.score("all", &all, |v| {
                    let mut values = vec![None; size];
                    for &(i, value) in v.iter() {
                        values[i] = value;
                    }
                    SimpleScore::new(score(&values))
                })?;
                Ok((b.build(), s))
            },
            p.initial_assignment(),
            c,
        )
        .unwrap();
        assert_eq!(a.best, b.best);
        assert_eq!(
            (
                a.evaluations,
                a.repair_move_evaluations,
                a.attempts,
                a.complete_reconstructions,
                a.accepted_moves,
                a.termination
            ),
            (
                b.evaluations,
                b.repair_move_evaluations,
                b.attempts,
                b.complete_reconstructions,
                b.accepted_moves,
                b.termination
            )
        );
        vec![a, b]
    }
}
mod lists {
    use super::*;
    pub(super) fn run_both(
        p: &ListProblem<i32>,
        c: RuinRecreateConfig,
        score: impl Fn(&[Vec<i32>]) -> i64,
    ) -> Vec<ListRuinRecreateResult<i32, SimpleScore>> {
        let count = p.entities().len();
        let mut n = ListConstraintNetwork::new(count, p.values().len());
        n.add_constraint("all", (0..count).map(ListScope::Entity).collect(), |v| {
            SimpleScore::new(score(
                &v.iter()
                    .map(|v| {
                        let ListFact::Entity(l) = v else {
                            unreachable!()
                        };
                        l.to_vec()
                    })
                    .collect::<Vec<_>>(),
            ))
        })
        .unwrap();
        let a = ruin_recreate_list_search_from(p, &n, p.initial_assignment(), c.clone()).unwrap();
        let b = ruin_recreate_list_stream_search_from(
            p,
            || {
                let mut b = StreamGraphBuilder::new();
                let s = ListStreamSources::new(&mut b);
                let lists = b.collect_list(s.entities.port(), |row| row.clone())?;
                b.score("all", &lists, |rows| {
                    let mut all = vec![Vec::new(); count];
                    for (i, v) in rows.iter() {
                        all[*i] = v.clone();
                    }
                    SimpleScore::new(score(&all))
                })?;
                Ok((b.build(), s))
            },
            p.initial_assignment(),
            c,
        )
        .unwrap();
        assert_eq!(a.best, b.best);
        assert_eq!(
            (
                a.evaluations,
                a.repair_move_evaluations,
                a.attempts,
                a.complete_reconstructions,
                a.accepted_moves,
                a.termination
            ),
            (
                b.evaluations,
                b.repair_move_evaluations,
                b.attempts,
                b.complete_reconstructions,
                b.accepted_moves,
                b.termination
            )
        );
        vec![a, b]
    }
}
mod mixed {
    use super::*;
    type Problem = MixedProblem<i32, i32>;
    type Result = MixedRuinRecreateResult<i32, i32, SimpleScore>;
    pub(super) fn run_both(
        p: &Problem,
        c: MixedRuinRecreateConfig,
        score: impl Fn(&[Option<i32>], &[Vec<i32>]) -> i64,
    ) -> Result {
        let nb = p.basic().variables().len();
        let ne = p.lists().entities().len();
        let mut n = MixedConstraintNetwork::new(nb, ne, p.lists().values().len());
        n.add_constraint(
            "coupled",
            (0..nb)
                .map(MixedScope::Basic)
                .chain((0..ne).map(MixedScope::ListEntity))
                .collect(),
            |f| {
                let basic: Vec<_> = f[..nb]
                    .iter()
                    .map(|v| {
                        let MixedFact::Basic(v) = v else {
                            unreachable!()
                        };
                        v.copied()
                    })
                    .collect();
                let lists: Vec<_> = f[nb..]
                    .iter()
                    .map(|v| {
                        let MixedFact::ListEntity(v) = v else {
                            unreachable!()
                        };
                        v.to_vec()
                    })
                    .collect();
                SimpleScore::new(score(&basic, &lists))
            },
        )
        .unwrap();
        let a = ruin_recreate_mixed_search_from(p, &n, p.initial_assignment(), c.clone()).unwrap();
        let b = ruin_recreate_mixed_stream_search_from(
            p,
            || {
                let mut b = StreamGraphBuilder::new();
                let s = MixedStreamSources::<i32, i32>::new(&mut b);
                let basics = b.collect_list(s.basic.port(), |row| *row)?;
                let lists = b.collect_list(s.lists.entities.port(), |row| row.clone())?;
                let all = b.join_equal(&basics, &lists, |_| 0, |_| 0)?;
                b.score("coupled", &all, |(bs, ls)| {
                    let mut basic = vec![None; nb];
                    let mut lists = vec![vec![]; ne];
                    for (i, v) in bs.iter() {
                        basic[*i] = *v;
                    }
                    for (i, v) in ls.iter() {
                        lists[*i] = v.clone();
                    }
                    SimpleScore::new(score(&basic, &lists))
                })?;
                Ok((b.build(), s))
            },
            p.initial_assignment(),
            c.clone(),
        )
        .unwrap();
        assert_eq!(a.best, b.best);
        assert_eq!(
            (
                a.evaluations,
                a.repair_move_evaluations,
                a.attempts,
                a.complete_reconstructions,
                a.accepted_moves,
                a.termination
            ),
            (
                b.evaluations,
                b.repair_move_evaluations,
                b.attempts,
                b.complete_reconstructions,
                b.accepted_moves,
                b.termination
            )
        );
        assert!(a.evaluations <= c.search.max_evaluations);
        if let Some(best) = &a.best {
            p.validate(&best.assignment).unwrap();
            assert!(p.is_complete(&best.assignment));
            assert_eq!(
                best.score,
                SimpleScore::new(score(
                    best.assignment.basic().values(),
                    best.assignment.lists().lists()
                ))
            );
        }
        a
    }
}
fn config<'a>(pools: &'a [RepairPool<'a>]) -> RuinRecreateConfig<'a> {
    RuinRecreateConfig {
        ruin_sampling: RuinSampling::CoverageCycle,
        repair_pools: Some(pools),
        minimum_ruined: 1,
        maximum_ruined: 3,
        max_attempts: 32,
        seed: 195,
        assert_scores: true,
        max_evaluations: 2_000,
        ..Default::default()
    }
}

#[test]
fn weighted_basic_pools_respect_pins_replay_and_resource_caps() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![0, 1]).with_initial(0),
        Variable::new(vec![0, 1]).with_initial(0),
        Variable::new(vec![0, 1]).with_initial(0).with_pinned(true),
    ])
    .unwrap();
    let pools = [
        RepairPool {
            candidates: &[0, 2],
            weight: 1,
        },
        RepairPool {
            candidates: &[1, 2],
            weight: 3,
        },
    ];
    let score = |v: &[Option<i32>]| v.iter().flatten().map(|&v| i64::from(v)).sum();
    let results = basic::run_both(&p, config(&pools), score);
    let replay = basic::run_both(&p, config(&pools), score);
    for (r, replay) in results.into_iter().zip(replay) {
        assert_eq!(r.best, replay.best);
        assert_eq!(
            r.best.unwrap().assignment.values(),
            &[Some(1), Some(1), Some(0)]
        );
        assert_eq!(r.attempts, 32);
        assert_eq!(r.termination, RuinRecreateTermination::AttemptLimit);
    }
    for budget in 0..=16 {
        for r in basic::run_both(
            &p,
            RuinRecreateConfig {
                max_evaluations: budget,
                ..config(&pools)
            },
            score,
        ) {
            assert!(r.evaluations <= budget);
            assert!(r.attempts <= 32);
            if let Some(best) = r.best {
                assert_eq!(best.assignment.get(2), Some(&Some(0)));
            }
        }
    }
    let token = CancellationToken::new();
    token.cancel();
    for r in basic::run_both(
        &p,
        RuinRecreateConfig {
            cancellation: Some(&token),
            ..config(&pools)
        },
        score,
    ) {
        assert_eq!(r.termination, RuinRecreateTermination::Cancelled);
        assert_eq!(r.evaluations, 0);
        assert_eq!(r.attempts, 0);
    }
}

#[test]
fn weighted_list_pools_preserve_prefix_and_full_pins_on_both_backends() {
    let p = ListProblem::new(
        vec![0, 1, 2, 3],
        vec![
            ListEntity::new(vec![0, 1, 2]).with_pinned_prefix(1),
            ListEntity::new(vec![3]).with_pinned(true),
            ListEntity::new(vec![]),
        ],
    )
    .unwrap();
    let pools = [
        RepairPool {
            candidates: &[0, 1],
            weight: 1,
        },
        RepairPool {
            candidates: &[2, 3],
            weight: 2,
        },
    ];
    let score = |lists: &[Vec<i32>]| lists[2].len() as i64;
    for r in lists::run_both(&p, config(&pools), score) {
        let a = r.best.unwrap().assignment;
        assert_eq!(a.list(0), Some(&[0][..]));
        assert_eq!(a.list(1), Some(&[3][..]));
        assert_eq!(a.list(2).unwrap().len(), 2);
        assert!(p.is_complete(&a));
        assert_eq!(r.attempts, 32);
    }
    for cap in 0..=8 {
        for r in lists::run_both(
            &p,
            RuinRecreateConfig {
                max_move_evaluations: Some(cap),
                ..config(&pools)
            },
            score,
        ) {
            assert!(r.repair_move_evaluations <= cap);
            let a = r.best.unwrap().assignment;
            assert_eq!(a.list(0).unwrap()[0], 0);
            assert_eq!(a.list(1), Some(&[3][..]));
        }
    }
}

#[test]
fn mixed_pools_keep_basic_and_registry_identities_distinct() {
    let p = MixedProblem::new(
        FiniteProblem::new(vec![
            Variable::new(vec![0, 1]).with_initial(0),
            Variable::new(vec![0, 1]).with_initial(0).with_pinned(true),
        ])
        .unwrap(),
        ListProblem::new(
            vec![0, 1],
            vec![
                ListEntity::new(vec![0, 1]).with_pinned_prefix(1),
                ListEntity::new(vec![]),
            ],
        )
        .unwrap(),
    );
    let pools = [
        RepairPool {
            candidates: &[0, 2],
            weight: 1,
        },
        RepairPool {
            candidates: &[1, 3],
            weight: 2,
        },
    ];
    for order in [
        MixedConstructionOrder::BasicThenLists,
        MixedConstructionOrder::ListsThenBasic,
    ] {
        let r = mixed::run_both(
            &p,
            MixedRuinRecreateConfig {
                search: config(&pools),
                order,
            },
            |basic, lists| i64::from(basic[0].unwrap_or(0)) + lists[1].len() as i64,
        );
        let a = r.best.unwrap().assignment;
        assert_eq!(a.basic().values(), &[Some(1), Some(0)]);
        assert_eq!(a.lists().list(0), Some(&[0][..]));
        assert_eq!(a.lists().list(1), Some(&[1][..]));
        assert_eq!(r.attempts, 32);
    }
}
