// Original runtime integration regressions; no upstream method credit.
use EnvironmentMode as M;
use std::{cell::Cell, rc::Rc};
use timetuck::*;
fn graph() -> (StreamGraph<'static, SimpleScore>, PlanningStreamSource<i32>) {
    let mut b = StreamGraphBuilder::new();
    let source = b.source::<usize, (usize, Option<i32>)>();
    b.score("reward", source.port(), |(_, value)| {
        SimpleScore::new(i64::from(value.unwrap_or(0)))
    })
    .unwrap();
    (b.build(), source)
}
macro_rules! backends {
    (|$manager:ident| $body:block) => {{
        let p = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]); 3]).unwrap();
        let mut n = ConstraintNetwork::new(3);
        for i in 0..3 {
            n.add_constraint(format!("reward-{i}"), vec![i], |v| {
                SimpleScore::new(i64::from(v[0].copied().unwrap_or(0)))
            })
            .unwrap();
        }
        {
            let d = ScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
            let mut $manager = d.into_context(
                M::NoAssert,
                vec![M::NoAssert, M::FullAssert, M::NoAssert],
                ContextMetrics {
                    best_constraint_totals: true,
                    ..Default::default()
                },
            );
            $body
        }
        {
            let (g, source) = graph();
            let d = StreamScoreDirector::new(&p, g, source, p.initial_assignment()).unwrap();
            let mut $manager = d.into_context(
                M::NoAssert,
                vec![M::NoAssert, M::FullAssert, M::NoAssert],
                ContextMetrics {
                    best_constraint_totals: true,
                    ..Default::default()
                },
                |_| Ok(graph()),
            );
            $body
        }
    }};
}
#[test]
fn real_construction_and_search_keep_state_totals_and_cumulative_counts() {
    backends!(|manager| {
        manager.start().unwrap();
        manager.start_phase(0).unwrap();
        let first = manager.handle();
        let construction = manager
            .run(|d| d.run_construction(ConstructionConfig::default()))
            .unwrap();
        assert_eq!(construction.score, Some(SimpleScore::new(6)));
        assert!(construction.complete);
        let count = u128::from(construction.evaluations);
        assert_eq!(manager.calculation_count(), count);
        let before = first.director().unwrap().assignment().values().as_ptr();
        let score = manager.calculate_score().unwrap();
        assert_eq!(score, SimpleScore::new(6));
        manager.start_phase(1).unwrap();
        assert!(first.director().is_none());
        assert_eq!(
            manager
                .handle()
                .director()
                .unwrap()
                .assignment()
                .values()
                .as_ptr(),
            before
        );
        let search = manager
            .run(|d| d.run_local_search(LocalSearchConfig::default()))
            .unwrap();
        assert_eq!(search.best.as_ref().unwrap().score, SimpleScore::new(6));
        assert_eq!(
            manager.calculation_count(),
            count + 1 + u128::from(search.evaluations)
        );
        assert_eq!(
            manager
                .constraint_totals()
                .unwrap()
                .iter()
                .map(|(_, s)| s.score())
                .sum::<i64>(),
            6
        );
        manager.start_phase(2).unwrap();
        assert_eq!(manager.calculate_score().unwrap(), SimpleScore::new(6));
        assert_eq!(
            manager.handle().director().unwrap().assignment().values(),
            &[Some(2); 3]
        );
        let last = manager.handle();
        manager.finish().unwrap();
        manager.finish().unwrap();
        assert!(last.director().is_none());
        assert!(matches!(
            manager.calculate_score(),
            Err(ScoringContextError::Closed)
        ));
    });
}
#[test]
fn retained_editing_handles_apply_real_moves_and_retired_handles_reject_them() {
    backends!(|manager| {
        let old_changes = manager.change_director();
        old_changes
            .edit(|d| {
                d.apply(&Move::Change {
                    variable: 0,
                    value: Some(2),
                })?
                .commit();
                Ok(())
            })
            .unwrap();
        assert_eq!(manager.calculate_score().unwrap().score(), 2);
        manager.start().unwrap();
        manager.start_phase(0).unwrap();
        assert!(old_changes.same_resource(&manager.change_director()));
        manager.start_phase(1).unwrap();
        assert!(matches!(
            old_changes.edit(|_| Ok(())),
            Err(ScoringContextError::Closed)
        ));
        manager
            .change_director()
            .edit(|d| {
                d.apply(&Move::Change {
                    variable: 1,
                    value: Some(1),
                })?
                .commit();
                Ok(())
            })
            .unwrap();
        assert_eq!(manager.calculate_score().unwrap().score(), 3);
    });
}
#[test]
fn failed_dense_preparation_retains_assignment_cache_identity_and_count() {
    let bad = Cell::new(false);
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(1)]).unwrap();
    let mut n = ConstraintNetwork::new(1);
    n.add_constraint("fallible", vec![0], |_| {
        if bad.get() {
            SimpleScore::new(i64::MAX)
        } else {
            SimpleScore::new(1)
        }
    })
    .unwrap();
    n.add_constraint("one", vec![], |_| SimpleScore::new(1))
        .unwrap();
    let d = ScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
    let mut manager = d.into_context(M::NoAssert, vec![M::FullAssert], ContextMetrics::default());
    manager.calculate_score().unwrap();
    manager.start().unwrap();
    let original = manager.handle();
    let assignment = original.director().unwrap().assignment().values().as_ptr();
    bad.set(true);
    assert!(matches!(
        manager.start_phase(0),
        Err(ScoringContextError::Scoring(_))
    ));
    assert!(original.same_resource(&manager.handle()));
    assert_eq!(
        original.director().unwrap().assignment().values().as_ptr(),
        assignment
    );
    assert_eq!(manager.calculation_count(), 1);
    bad.set(false);
    assert_eq!(manager.calculate_score().unwrap().score(), 2);
    manager.start_phase(0).unwrap();
    assert!(original.director().is_none());
}
#[test]
fn failed_stream_graph_factory_retains_the_live_resource() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(1)]).unwrap();
    let (g, source) = graph();
    let d = StreamScoreDirector::new(&p, g, source, p.initial_assignment()).unwrap();
    let fail = Rc::new(Cell::new(true));
    let flag = fail.clone();
    let mut manager = d.into_context(
        M::NoAssert,
        vec![M::FullAssert],
        ContextMetrics::default(),
        move |_| {
            if flag.get() {
                Err(StreamScoringError::PlanningSourceNotEmpty)
            } else {
                Ok(graph())
            }
        },
    );
    manager.start().unwrap();
    let old = manager.handle();
    assert!(manager.start_phase(0).is_err());
    assert!(old.same_resource(&manager.handle()));
    assert_eq!(manager.calculate_score().unwrap().score(), 1);
    fail.set(false);
    manager.start_phase(0).unwrap();
    assert!(old.director().is_none());
    assert_eq!(manager.calculate_score().unwrap().score(), 1);
}
#[test]
fn shutdown_on_drop_and_bad_lifecycle_calls_are_observable() {
    backends!(|manager| {
        assert!(matches!(
            manager.start_phase(0),
            Err(ScoringContextError::NotStarted)
        ));
        assert!(matches!(
            manager.run(|d| d.run_construction(ConstructionConfig::default())),
            Err(ScoringContextError::NotStarted)
        ));
        manager.start().unwrap();
        assert!(matches!(
            manager.start_phase(3),
            Err(ScoringContextError::PhaseIndex { phase: 3, count: 3 })
        ));
        let live = manager.handle();
        drop(manager);
        assert!(live.director().is_none());
    });
}
#[test]
fn full_mode_detects_stale_dense_scores_at_managed_calculations() {
    let changed = Cell::new(0);
    let p = FiniteProblem::new(vec![Variable::new(vec![0]).with_initial(0)]).unwrap();
    let mut n = ConstraintNetwork::new(1);
    n.add_constraint("external", vec![0], |_| SimpleScore::new(changed.get()))
        .unwrap();
    let d = ScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
    let mut manager = d.into_context(M::FullAssert, vec![], ContextMetrics::default());
    changed.set(1);
    assert!(matches!(
        manager.calculate_score(),
        Err(ScoringContextError::Scoring(ScoringError::ScoreCorruption(
            _
        )))
    ));
    assert_eq!(manager.calculation_count(), 0);
}
#[test]
fn effective_weight_overrides_survive_actual_dense_and_stream_rebuilds() {
    let p = FiniteProblem::new(vec![Variable::new(vec![1]).with_initial(1)]).unwrap();
    let mut n = ConstraintNetwork::new(1);
    n.add_weighted_constraint("reward", vec![0], SimpleScore::new(2), |_| 1)
        .unwrap();
    let weights =
        ConstraintWeightOverrides::from_iter([("reward".to_owned(), SimpleScore::new(5))]);
    let mut d = ScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
    d.apply_weight_overrides(&weights).unwrap();
    let mut manager = d.into_context(
        M::NoAssert,
        vec![M::FullAssert, M::NoAssert],
        ContextMetrics {
            best_score: true,
            ..Default::default()
        },
    );
    manager.start().unwrap();
    for phase in 0..2 {
        manager.start_phase(phase).unwrap();
        assert_eq!(manager.calculate_score().unwrap(), SimpleScore::new(5));
        assert_eq!(
            manager
                .handle()
                .director()
                .unwrap()
                .constraint_weights()
                .map(|(n, w)| (n.to_owned(), *w))
                .collect::<Vec<_>>(),
            vec![("reward".to_owned(), SimpleScore::new(5))]
        );
    }
    assert_eq!(manager.best_score(), Some(&SimpleScore::new(5)));
    fn weighted_graph() -> (StreamGraph<'static, SimpleScore>, PlanningStreamSource<i32>) {
        let mut b = StreamGraphBuilder::new();
        let source = b.source::<usize, (usize, Option<i32>)>();
        b.score_weighted("reward", source.port(), SimpleScore::new(2), |(_, v)| {
            i64::from(v.unwrap_or(0))
        })
        .unwrap();
        (b.build(), source)
    }
    let (g, source) = weighted_graph();
    let mut d = StreamScoreDirector::new(&p, g, source, p.initial_assignment()).unwrap();
    d.apply_weight_overrides(&weights).unwrap();
    let mut manager = d.into_context(
        M::NoAssert,
        vec![M::FullAssert, M::NoAssert],
        ContextMetrics {
            best_score: true,
            ..Default::default()
        },
        |_| Ok(weighted_graph()),
    );
    manager.start().unwrap();
    for phase in 0..2 {
        manager.start_phase(phase).unwrap();
        assert_eq!(manager.calculate_score().unwrap(), SimpleScore::new(5));
        assert_eq!(
            manager
                .handle()
                .director()
                .unwrap()
                .graph()
                .constraint_weights()
                .map(|(n, w)| (n.to_owned(), *w))
                .collect::<Vec<_>>(),
            vec![("reward".to_owned(), SimpleScore::new(5))]
        );
    }
    assert_eq!(manager.best_score(), Some(&SimpleScore::new(5)));
}
