//! Original native scoped/stream constraint-total monitoring regressions.
use std::{cell::Cell, collections::BTreeMap};
use timetuck::*;
fn s(n: i64) -> SimpleScore {
    SimpleScore::new(n)
}
fn basic() -> FiniteProblem<i64> {
    FiniteProblem::new(vec![Variable::new(vec![0, 1, 2])]).unwrap()
}
fn lists() -> ListProblem<i64> {
    ListProblem::new(vec![1, 2], vec![ListEntity::new(vec![])]).unwrap()
}
fn cost(v: &[i64]) -> i64 {
    -v.iter()
        .enumerate()
        .map(|(i, v)| (i as i64 + 1) * v)
        .sum::<i64>()
}
trait Expected {
    fn expected(&self) -> BTreeMap<String, ConstraintTotalMetric<SimpleScore>>;
}
fn totals(
    items: impl IntoIterator<Item = (&'static str, i64, usize)>,
) -> BTreeMap<String, ConstraintTotalMetric<SimpleScore>> {
    items
        .into_iter()
        .map(|(n, v, c)| {
            (
                n.into(),
                ConstraintTotalMetric {
                    score: s(v),
                    match_count: c,
                },
            )
        })
        .collect()
}
impl Expected for Assignment<i64> {
    fn expected(&self) -> BTreeMap<String, ConstraintTotalMetric<SimpleScore>> {
        totals([
            ("basic", self.values()[0].unwrap_or(0), 1),
            ("disabled", 0, 0),
            ("zero", 0, 1),
        ])
    }
}
impl Expected for ListAssignment<i64> {
    fn expected(&self) -> BTreeMap<String, ConstraintTotalMetric<SimpleScore>> {
        totals([
            ("list", cost(&self.lists()[0]), 1),
            ("disabled", 0, 0),
            ("zero", 0, 1),
        ])
    }
}
impl Expected for MixedAssignment<i64, i64> {
    fn expected(&self) -> BTreeMap<String, ConstraintTotalMetric<SimpleScore>> {
        let mut t = self.basic().expected();
        t.insert(
            "list".into(),
            ConstraintTotalMetric {
                score: s(cost(&self.lists().lists()[0])),
                match_count: 1,
            },
        );
        t
    }
}
fn bn(c: &Cell<u128>) -> ConstraintNetwork<'_, i64, SimpleScore> {
    let mut n = ConstraintNetwork::new(1);
    n.add_constraint("basic", vec![0], |v| {
        c.set(c.get() + 1);
        s(v[0].copied().unwrap_or(0))
    })
    .unwrap();
    n.add_weighted_constraint("disabled", vec![0], s(0), |_| -> i64 {
        panic!("disabled callback")
    })
    .unwrap();
    n.add_constraint("zero", vec![0], |_| {
        c.set(c.get() + 1);
        s(0)
    })
    .unwrap();
    n
}
fn ln(c: &Cell<u128>) -> ListConstraintNetwork<'_, i64, SimpleScore> {
    let mut n = ListConstraintNetwork::new(1, 2);
    n.add_constraint("list", vec![ListScope::Entity(0)], |v| {
        c.set(c.get() + 1);
        let ListFact::Entity(v) = v[0] else {
            unreachable!()
        };
        s(cost(v))
    })
    .unwrap();
    n.add_weighted_constraint("disabled", vec![ListScope::Entity(0)], s(0), |_| -> i64 {
        panic!("disabled callback")
    })
    .unwrap();
    n.add_constraint("zero", vec![ListScope::Entity(0)], |_| {
        c.set(c.get() + 1);
        s(0)
    })
    .unwrap();
    n
}
fn mn(c: &Cell<u128>) -> MixedConstraintNetwork<'_, i64, i64, SimpleScore> {
    let mut n = MixedConstraintNetwork::new(1, 1, 2);
    n.add_constraint("basic", vec![MixedScope::Basic(0)], |v| {
        c.set(c.get() + 1);
        let MixedFact::Basic(v) = v[0] else {
            unreachable!()
        };
        s(v.copied().unwrap_or(0))
    })
    .unwrap();
    n.add_constraint("list", vec![MixedScope::ListEntity(0)], |v| {
        c.set(c.get() + 1);
        let MixedFact::ListEntity(v) = v[0] else {
            unreachable!()
        };
        s(cost(v))
    })
    .unwrap();
    n.add_weighted_constraint("disabled", vec![MixedScope::Basic(0)], s(0), |_| -> i64 {
        panic!("disabled callback")
    })
    .unwrap();
    n.add_constraint("zero", vec![MixedScope::Basic(0)], |_| {
        c.set(c.get() + 1);
        s(0)
    })
    .unwrap();
    n
}
fn bg(c: &Cell<u128>) -> (StreamGraph<'_, SimpleScore>, PlanningStreamSource<i64>) {
    let mut b = StreamGraphBuilder::new();
    let source = b.source::<usize, (usize, Option<i64>)>();
    b.score("basic", source.port(), |(_, v)| {
        c.set(c.get() + 1);
        s(v.unwrap_or(0))
    })
    .unwrap();
    b.score_weighted("disabled", source.port(), s(0), |_| -> i64 {
        panic!("disabled callback")
    })
    .unwrap();
    b.score("zero", source.port(), |_| {
        c.set(c.get() + 1);
        s(0)
    })
    .unwrap();
    (b.build(), source)
}
fn lg(c: &Cell<u128>) -> (StreamGraph<'_, SimpleScore>, ListStreamSources<i64>) {
    let mut b = StreamGraphBuilder::new();
    let source = ListStreamSources::new(&mut b);
    b.score("list", source.entities.port(), |(_, v)| {
        c.set(c.get() + 1);
        s(cost(v))
    })
    .unwrap();
    b.score_weighted("disabled", source.entities.port(), s(0), |_| -> i64 {
        panic!("disabled callback")
    })
    .unwrap();
    b.score("zero", source.entities.port(), |_| {
        c.set(c.get() + 1);
        s(0)
    })
    .unwrap();
    (b.build(), source)
}
fn mg(c: &Cell<u128>) -> (StreamGraph<'_, SimpleScore>, MixedStreamSources<i64, i64>) {
    let mut b = StreamGraphBuilder::new();
    let source = MixedStreamSources::new(&mut b);
    b.score("basic", source.basic.port(), |(_, v)| {
        c.set(c.get() + 1);
        s(v.unwrap_or(0))
    })
    .unwrap();
    b.score("list", source.lists.entities.port(), |(_, v)| {
        c.set(c.get() + 1);
        s(cost(v))
    })
    .unwrap();
    b.score_weighted("disabled", source.basic.port(), s(0), |_| -> i64 {
        panic!("disabled callback")
    })
    .unwrap();
    b.score("zero", source.basic.port(), |_| {
        c.set(c.get() + 1);
        s(0)
    })
    .unwrap();
    (b.build(), source)
}
fn plans() -> Vec<SolverSequence<'static>> {
    let construction = SolverPhase::Construction(Default::default());
    let local = SolverPhase::LocalSearch(LocalSearchConfig {
        max_steps: 3,
        ..Default::default()
    });
    let repair = SolverPhase::RuinRecreate(RuinRecreateConfig {
        minimum_ruined: 1,
        maximum_ruined: 1,
        max_attempts: 3,
        seed: 7,
        ..Default::default()
    });
    let exact = SolverPhase::Exact(Default::default());
    let bounded = SolverPhase::Exact(ExactPhaseConfig {
        strategy: ExactSearchStrategy::BranchAndBound(Default::default()),
        ..Default::default()
    });
    let complete = SolverPhase::Exact(ExactPhaseConfig {
        strategy: ExactSearchStrategy::CompleteBranchAndBound(Default::default()),
        ..Default::default()
    });
    vec![
        vec![construction.clone()],
        vec![construction.clone(), local.clone()],
        vec![construction.clone(), repair.clone()],
        vec![exact.clone()],
        vec![bounded.clone()],
        vec![construction.clone(), complete.clone()],
        vec![
            construction.clone(),
            SolverPhase::Custom(Default::default()),
        ],
        vec![construction, local, repair, exact, bounded, complete],
    ]
    .into_iter()
    .map(|phases| SolverSequence {
        phases,
        ..Default::default()
    })
    .collect()
}
fn mixed(c: SolverSequence<'static>) -> MixedSolverSequence<'static> {
    MixedSolverSequence {
        limits: c.limits,
        phases: c
            .phases
            .into_iter()
            .map(|p| match p {
                SolverPhase::Construction(c) => {
                    MixedSolverPhase::Construction(MixedConstructionConfig {
                        construction: c,
                        ..Default::default()
                    })
                }
                SolverPhase::LocalSearch(c) => {
                    MixedSolverPhase::LocalSearch(MixedLocalSearchConfig {
                        search: c,
                        joint_moves: true,
                    })
                }
                SolverPhase::RuinRecreate(c) => {
                    MixedSolverPhase::RuinRecreate(MixedRuinRecreateConfig {
                        search: c,
                        ..Default::default()
                    })
                }
                SolverPhase::Exact(c) => MixedSolverPhase::Exact(c),
                SolverPhase::Custom(c) => MixedSolverPhase::Custom(c),
            })
            .collect(),
    }
}
fn verify<A: Expected>(snapshot: &SolveMetricSnapshot<SimpleScore>, a: &A) {
    let BestConstraintTotals::Available(t) = &snapshot.best_constraint_totals else {
        panic!("{:?}", snapshot.best_constraint_totals)
    };
    assert_eq!(**t, a.expected());
}
fn observe<A: Expected>(m: &SolverMetrics<SimpleScore>, e: SolverEvent<'_, A, SimpleScore>) {
    if let SolverEvent::Best(u) = e {
        let snap = m.snapshot();
        let snap = &snap.active[0];
        assert_eq!(snap.best_score, Some(u.score));
        verify(snap, u.assignment);
    }
}
macro_rules! matrix {
    ($test:ident,$p:expr,$a:ty,$network:ident,$graph:ident,$d:ident,$sd:ident,$solve:ident,$ssolve:ident,$convert:expr,$movement:expr) => {
        #[test]
        fn $test() {
            for plan in plans() {
                for stream in [false, true] {
                    for retained in [false, true] {
                        let mut baseline = None;
                        for request in [false, true] {
                            let p = $p;
                            let calls = Cell::new(0);
                            let network = $network(&calls);
                            let factory = || Ok($graph(&calls));
                            let metrics = SolverMetrics::default();
                            let metrics = if request {
                                metrics.with_best_constraint_totals()
                            } else {
                                metrics
                            };
                            let mut events = |e: SolverEvent<'_, $a, SimpleScore>| {
                                if request {
                                    observe(&metrics, e);
                                }
                            };
                            macro_rules! extensions {
                                () => {
                                    SolverExtensions {
                                        metrics: Some(&metrics),
                                        events: Some(&mut events),
                                        command: Some(&mut |_,
                                                            c: &mut CustomPhaseContext<
                                            '_,
                                            $a,
                                            _,
                                            SimpleScore,
                                            _,
                                        >| {
                                            c.execute(&$movement)?;
                                            Ok(())
                                        }),
                                        ..Default::default()
                                    }
                                };
                            }
                            let config = ($convert)(plan.clone());
                            let result = if stream {
                                if retained {
                                    let (g, s) = factory().unwrap();
                                    let mut d = $sd::new(&p, g, s, p.initial_assignment()).unwrap();
                                    let r = d
                                        .run_solver_with_extensions(config, extensions!())
                                        .unwrap();
                                    assert_eq!(d.assignment(), &r.assignment);
                                    r
                                } else {
                                    $ssolve(&p, factory, config, extensions!()).unwrap()
                                }
                            } else {
                                if retained {
                                    let mut d =
                                        $d::new(&p, &network, p.initial_assignment()).unwrap();
                                    let r = d
                                        .run_solver_with_extensions(config, extensions!())
                                        .unwrap();
                                    assert_eq!(d.assignment(), &r.assignment);
                                    r
                                } else {
                                    $solve(&p, &network, config, extensions!()).unwrap()
                                }
                            };
                            let latest = metrics.snapshot().latest.unwrap();
                            assert_eq!(latest.best_score, result.score);
                            assert_eq!(latest.work, result.work);
                            if request {
                                verify(&latest, &result.assignment);
                            } else {
                                assert_eq!(
                                    latest.best_constraint_totals,
                                    BestConstraintTotals::Disabled
                                );
                            }
                            let result = (result, calls.get());
                            if let Some(b) = baseline.take() {
                                assert_eq!(result, b);
                            } else {
                                baseline = Some(result);
                            }
                        }
                    }
                }
            }
        }
    };
}
matrix!(
    basic_routes,
    basic(),
    Assignment<i64>,
    bn,
    bg,
    ScoreDirector,
    StreamScoreDirector,
    solve_with_extensions,
    solve_stream_with_extensions,
    |c| c,
    Move::Change {
        variable: 0,
        value: Some(0)
    }
);
matrix!(
    list_routes,
    lists(),
    ListAssignment<i64>,
    ln,
    lg,
    ListScoreDirector,
    ListStreamScoreDirector,
    solve_list_with_extensions,
    solve_list_stream_with_extensions,
    |c| c,
    ListMove::Change {
        source: ListPosition {
            entity: 0,
            index: 0
        },
        destination: ListPosition {
            entity: 0,
            index: 1
        }
    }
);
matrix!(mixed_routes,MixedProblem::new(basic(),lists()),MixedAssignment<i64,i64>,mn,mg,MixedScoreDirector,MixedStreamScoreDirector,solve_mixed_with_extensions,solve_mixed_stream_with_extensions,mixed, MixedMove {basic:Move::Change {variable:0,value:Some(0)},lists:ListMove::Composite(vec![])});

#[test]
fn live_weights_use_effective_configuration_and_snapshots_stay_owned() {
    let p = FiniteProblem::new(vec![Variable::new(vec![2]).with_initial(2)]).unwrap();
    let calls = Cell::new(0);
    let mut n = ConstraintNetwork::new(1);
    n.add_weighted_constraint("active", vec![0], s(1), |v| {
        calls.set(calls.get() + 1);
        v[0].copied().unwrap()
    })
    .unwrap();
    n.add_weighted_constraint("off", vec![0], s(0), |v| {
        calls.set(calls.get() + 1);
        v[0].copied().unwrap()
    })
    .unwrap();
    let mut b = StreamGraphBuilder::new();
    let source = b.source::<usize, (usize, Option<i64>)>();
    b.score_weighted("active", source.port(), s(1), |(_, v)| {
        calls.set(calls.get() + 1);
        v.unwrap()
    })
    .unwrap();
    b.score_weighted("off", source.port(), s(0), |(_, v)| {
        calls.set(calls.get() + 1);
        v.unwrap()
    })
    .unwrap();
    let empty = b.filter(source.port(), |_| false).unwrap();
    b.score("empty", &empty, |_| s(9)).unwrap();
    let mut scoped = ScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
    let mut stream =
        StreamScoreDirector::new(&p, b.build(), source, p.initial_assignment()).unwrap();
    let config = SolverSequence {
        phases: vec![SolverPhase::Construction(ConstructionConfig {
            max_evaluations: 1,
            ..Default::default()
        })],
        ..Default::default()
    };
    let metrics = SolverMetrics::default().with_best_constraint_totals();
    let mut saved = Vec::new();
    for (active, off) in [(0, 3), (2, 0)] {
        let weights = [("active".to_owned(), s(active)), ("off".to_owned(), s(off))]
            .into_iter()
            .collect::<ConstraintWeightOverrides<_>>();
        macro_rules! check {
            ($director:ident,$stream:expr) => {{
                $director.apply_weight_overrides(&weights).unwrap();
                let before = calls.get();
                let result = $director
                    .run_solver_with_extensions(
                        config.clone(),
                        SolverExtensions {
                            metrics: Some(&metrics),
                            ..Default::default()
                        },
                    )
                    .unwrap();
                assert_eq!(
                    calls.get(),
                    before,
                    "retained initialized score needs no callbacks"
                );
                assert_eq!(result.score, Some(s(2 * (active + off))));
                let snapshot = metrics.snapshot().latest.unwrap();
                let BestConstraintTotals::Available(t) = &snapshot.best_constraint_totals else {
                    panic!("{:?}", snapshot.best_constraint_totals)
                };
                assert_eq!(
                    t["active"],
                    ConstraintTotalMetric {
                        score: s(2 * active),
                        match_count: usize::from(active != 0)
                    }
                );
                assert_eq!(
                    t["off"],
                    ConstraintTotalMetric {
                        score: s(2 * off),
                        match_count: usize::from(off != 0)
                    }
                );
                if $stream {
                    assert_eq!(
                        t["empty"],
                        ConstraintTotalMetric {
                            score: s(0),
                            match_count: 0
                        }
                    );
                }
                saved.push(snapshot);
            }};
        }
        check!(scoped, false);
        check!(stream, true);
    }
    for old in &saved[..2] {
        let BestConstraintTotals::Available(t) = &old.best_constraint_totals else {
            unreachable!()
        };
        assert_eq!(t["off"].score, s(6));
        assert_eq!(t["active"].match_count, 0);
    }
    assert_eq!(
        n.constraint_weights()
            .map(|(n, s)| (n.to_owned(), *s))
            .collect::<Vec<_>>(),
        vec![("active".into(), s(1)), ("off".into(), s(0))]
    );
}

#[test]
fn exact_budget_prefixes_keep_best_totals_and_zero_budget_is_unavailable() {
    let p = FiniteProblem::new(vec![Variable::new(vec![2, 0, 1])]).unwrap();
    for strategy in [
        ExactSearchStrategy::Enumerate,
        ExactSearchStrategy::BranchAndBound(Default::default()),
    ] {
        for budget in 0..=5 {
            for stream in [false, true] {
                for retained in [false, true] {
                    let calls = Cell::new(0);
                    let n = bn(&calls);
                    let metrics = SolverMetrics::default().with_best_constraint_totals();
                    let config = SolverSequence {
                        limits: SolverLimits {
                            max_evaluations: budget,
                            ..Default::default()
                        },
                        phases: vec![SolverPhase::Exact(ExactPhaseConfig {
                            strategy: strategy.clone(),
                            ..Default::default()
                        })],
                    };
                    macro_rules! extensions {
                        () => {
                            SolverExtensions {
                                metrics: Some(&metrics),
                                ..Default::default()
                            }
                        };
                    }
                    let r = if stream {
                        if retained {
                            let (g, s) = bg(&calls);
                            StreamScoreDirector::new(&p, g, s, p.initial_assignment())
                                .unwrap()
                                .run_solver_with_extensions(config, extensions!())
                                .unwrap()
                        } else {
                            solve_stream_with_extensions(
                                &p,
                                || Ok(bg(&calls)),
                                config,
                                extensions!(),
                            )
                            .unwrap()
                        }
                    } else if retained {
                        ScoreDirector::new(&p, &n, p.initial_assignment())
                            .unwrap()
                            .run_solver_with_extensions(config, extensions!())
                            .unwrap()
                    } else {
                        solve_with_extensions(&p, &n, config, extensions!()).unwrap()
                    };
                    let snapshot = metrics.snapshot().latest.unwrap();
                    if r.score.is_some() {
                        verify(&snapshot, &r.assignment);
                        assert_eq!(snapshot.best_score, r.score);
                    } else {
                        assert_eq!(
                            snapshot.best_constraint_totals,
                            BestConstraintTotals::Unavailable(
                                ConstraintTotalUnavailable::NoBestYet
                            )
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn optional_construction_tie_adopts_final_contributions_without_a_new_best_event() {
    for stream in [false, true] {
        for retained in [false, true] {
            let p =
                FiniteProblem::new(vec![Variable::new(vec![1_i64]).allowing_unassigned()]).unwrap();
            let mut n = ConstraintNetwork::new(1);
            n.add_constraint("positive", vec![0], |v| s(v[0].copied().unwrap_or(0)))
                .unwrap();
            n.add_constraint("negative", vec![0], |v| s(-v[0].copied().unwrap_or(0)))
                .unwrap();
            let graph = || {
                let mut b = StreamGraphBuilder::new();
                let source = b.source::<usize, (usize, Option<i64>)>();
                b.score("positive", source.port(), |(_, v)| s(v.unwrap_or(0)))
                    .unwrap();
                b.score("negative", source.port(), |(_, v)| s(-v.unwrap_or(0)))
                    .unwrap();
                Ok((b.build(), source))
            };
            let metrics = SolverMetrics::default().with_best_constraint_totals();
            let mut events_count = 0;
            let mut events = |e: SolverEvent<'_, Assignment<i64>, SimpleScore>| {
                if let SolverEvent::Best(_) = e {
                    events_count += 1;
                    let BestConstraintTotals::Available(t) =
                        &metrics.snapshot().active[0].best_constraint_totals
                    else {
                        panic!()
                    };
                    assert_eq!(t["positive"].score, s(0));
                }
            };
            macro_rules! extensions {
                () => {
                    SolverExtensions {
                        metrics: Some(&metrics),
                        events: Some(&mut events),
                        ..Default::default()
                    }
                };
            }
            let config = SolverSequence {
                phases: vec![SolverPhase::Construction(ConstructionConfig {
                    optional: OptionalConstruction::NonWorsening,
                    ..Default::default()
                })],
                ..Default::default()
            };
            let r = if stream {
                if retained {
                    let (g, s) = graph().unwrap();
                    StreamScoreDirector::new(&p, g, s, p.initial_assignment())
                        .unwrap()
                        .run_solver_with_extensions(config, extensions!())
                        .unwrap()
                } else {
                    solve_stream_with_extensions(&p, graph, config, extensions!()).unwrap()
                }
            } else if retained {
                ScoreDirector::new(&p, &n, p.initial_assignment())
                    .unwrap()
                    .run_solver_with_extensions(config, extensions!())
                    .unwrap()
            } else {
                solve_with_extensions(&p, &n, config, extensions!()).unwrap()
            };
            assert_eq!(events_count, 1);
            assert_eq!(r.assignment.values(), &[Some(1)]);
            let BestConstraintTotals::Available(t) =
                metrics.snapshot().latest.unwrap().best_constraint_totals
            else {
                panic!()
            };
            assert_eq!(t["positive"].score, s(1));
            assert_eq!(t["negative"].score, s(-1));
        }
    }
}
