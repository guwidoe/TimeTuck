//! Original typed shadow graph cache isolation, rollback, failures and solver integration.
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
struct Model;
struct Entity;
#[derive(Eq, PartialEq)]
struct Amount(i64); // Cached values deliberately do not implement Clone.
fn schema() -> PlanningSchema {
    PlanningSchema::new::<Model>(vec![PlanningEntityDefinition::of::<Entity>(vec![
        PlanningVariableDefinition::basic::<i64>("input", false),
        PlanningVariableDefinition::shadow::<Amount>("amount"),
    ])])
    .unwrap()
}
#[test]
fn per_identity_caches_survive_trials_and_rebuild_and_restart_after_retraction() {
    let schema = schema();
    let e = schema.entity::<Entity>().unwrap();
    let calls = &Cell::new(0);
    let reads = &Cell::new(0);
    let projects = &Cell::new(0);
    let mut shadows = ShadowSessionBuilder::<usize, i64>::new(&schema);
    let input = shadows
        .input(e.variable("input").unwrap(), 0, move |w| {
            reads.set(reads.get() + 1);
            Some(*w)
        })
        .unwrap();
    let read = input.clone();
    let amount = shadows
        .derive(e.variable("amount").unwrap(), 0, move |_, i| {
            calls.set(calls.get() + 1);
            i.get(&read).unwrap().map(|v| Amount(v * 2))
        })
        .unwrap();
    shadows.set_dependencies(&amount, &[input.erase()]).unwrap();
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, i64>();
    let mapped = b
        .map_shadows(source.port(), shadows, move |_, s| {
            projects.set(projects.get() + 1);
            Ok(s.read(&amount)?.value.unwrap().0)
        })
        .unwrap();
    let matches = b
        .score("amount", &mapped, |v| SimpleScore::new(*v))
        .unwrap();
    let mut g = b.build();
    {
        let mut t = g.transaction();
        t.apply(&source, TupleChange::put(7, 3)).unwrap();
        t.apply(&source, TupleChange::put(8, 9)).unwrap();
        t.commit().unwrap();
    }
    assert_eq!(calls.get(), 2);
    assert_eq!(g.score(), SimpleScore::new(24));
    // Scratch state last belonged to 8; unchanged 7 must use its own cache.
    {
        let mut t = g.transaction();
        t.apply(&source, TupleChange::put(7, 3)).unwrap();
        t.commit().unwrap();
    }
    assert_eq!(calls.get(), 2);
    let old = g.constraint_matches(&matches).unwrap().clone();
    let counts;
    {
        let mut t = g.transaction();
        t.apply(&source, TupleChange::put(7, 5)).unwrap();
        t.apply(&source, TupleChange::retract(8)).unwrap();
        assert_eq!(t.score().unwrap(), SimpleScore::new(10));
        counts = (calls.get(), reads.get(), projects.get());
    }
    assert_eq!((calls.get(), reads.get(), projects.get()), counts);
    assert_eq!(g.constraint_matches(&matches).unwrap(), &old);
    {
        let mut t = g.transaction();
        t.apply(&source, TupleChange::put(7, 3)).unwrap();
        t.apply(&source, TupleChange::put(8, 9)).unwrap();
        t.commit().unwrap();
    }
    assert_eq!(calls.get(), counts.0);
    g.assert_consistent().unwrap();
    let before = calls.get();
    {
        let mut t = g.transaction();
        t.apply(&source, TupleChange::put(7, 3)).unwrap();
        t.apply(&source, TupleChange::put(8, 9)).unwrap();
        t.commit().unwrap();
    }
    assert_eq!(calls.get(), before);
    {
        let mut t = g.transaction();
        t.apply(&source, TupleChange::retract(7)).unwrap();
        t.apply(&source, TupleChange::retract(7)).unwrap();
        t.apply(&source, TupleChange::put(7, 3)).unwrap();
        t.commit().unwrap();
    }
    assert_eq!(calls.get(), before + 1);
    assert_eq!(g.constraint_matches(&matches).unwrap(), &old);
}
#[test]
fn reader_supplier_projection_and_downstream_failures_restore_source_and_cache() {
    let schema = schema();
    let e = schema.entity::<Entity>().unwrap();
    let failure = &Cell::new(0);
    let calls = &Cell::new(0);
    let mut shadows = ShadowSessionBuilder::<usize, i64>::new(&schema);
    let input = shadows
        .input(e.variable("input").unwrap(), 0, move |w| {
            assert_ne!(failure.get(), 1, "reader");
            Some(*w)
        })
        .unwrap();
    let read = input.clone();
    let amount = shadows
        .derive(e.variable("amount").unwrap(), 0, move |_, i| {
            calls.set(calls.get() + 1);
            assert_ne!(failure.get(), 2, "supplier");
            i.get(&read).unwrap().map(|v| Amount(v * 2))
        })
        .unwrap();
    shadows.set_dependencies(&amount, &[input.erase()]).unwrap();
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, i64>();
    let mapped = b
        .map_shadows(source.port(), shadows, move |_, s| {
            assert_ne!(failure.get(), 3, "projection");
            if failure.get() == 4 {
                return Err(ShadowSessionError::UnregisteredSource);
            }
            Ok(s.read(&amount)?.value.unwrap().0)
        })
        .unwrap();
    b.score("amount", &mapped, move |v| {
        assert_ne!(failure.get(), 5, "score");
        SimpleScore::new(*v)
    })
    .unwrap();
    let mut g = b.build();
    {
        let mut t = g.transaction();
        t.apply(&source, TupleChange::put(0, 3)).unwrap();
        t.commit().unwrap();
    }
    for fail in 1..=5 {
        failure.set(fail);
        let r = catch_unwind(AssertUnwindSafe(|| {
            let mut t = g.transaction();
            let result = t.apply(&source, TupleChange::put(0, 7));
            if fail == 4 {
                assert!(matches!(t.score(), Err(GraphError::FailedTransaction)));
            }
            result
        }));
        if fail == 4 {
            assert!(matches!(r.unwrap(), Err(GraphError::Dependency(_))));
        } else {
            assert!(r.is_err());
        }
        failure.set(0);
        assert_eq!(g.score(), SimpleScore::new(6));
        assert_eq!(g.source_rows(&source).unwrap()[&0], 3);
        let before = calls.get();
        let mut t = g.transaction();
        t.apply(&source, TupleChange::put(0, 3)).unwrap();
        t.commit().unwrap();
        assert_eq!(calls.get(), before, "undo restores the original cache");
    }
    let mut t = g.transaction();
    t.apply(&source, TupleChange::put(0, 7)).unwrap();
    t.commit().unwrap();
    assert_eq!(g.score(), SimpleScore::new(14));
    g.assert_consistent().unwrap();
}
#[test]
fn list_move_scopes_and_saved_winners_restore_without_shadow_callbacks() {
    let schema = PlanningSchema::new::<Model>(vec![PlanningEntityDefinition::of::<Entity>(vec![
        PlanningVariableDefinition::list::<i64>("values", false),
        PlanningVariableDefinition::shadow::<Amount>("amount"),
    ])])
    .unwrap();
    let e = schema.entity::<Entity>().unwrap();
    let calls = &Cell::new(0);
    let projects = &Cell::new(0u64);
    let p = ListProblem::new(vec![1i64, 2, 3], vec![ListEntity::new(vec![3, 2, 1])]).unwrap();
    let factory = || {
        let mut shadows = ShadowSessionBuilder::<usize, Vec<i64>>::new(&schema);
        let input = shadows
            .list_input(e.variable("values").unwrap(), 0, Clone::clone)
            .unwrap();
        let read = input.clone();
        let amount = shadows
            .derive(e.variable("amount").unwrap(), 0, move |_, i| {
                calls.set(calls.get() + 1);
                let list = i.get(&read).unwrap()?;
                Some(Amount(
                    list.iter()
                        .enumerate()
                        .map(|(i, v)| (i as i64 + 1) * v)
                        .sum(),
                ))
            })
            .unwrap();
        shadows.set_dependencies(&amount, &[input.erase()]).unwrap();
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let sources = ListStreamSources::new(&mut b);
        let lists = b.map(sources.entities.port(), |(_, list): &(usize, Vec<i64>)| {
            list.clone()
        })?;
        let mapped = b.map_shadows(&lists, shadows, move |_, s| {
            projects.set(projects.get() + 1);
            Ok(s.read(&amount)?.value.unwrap().0)
        })?;
        b.score("amount", &mapped, |v| SimpleScore::new(*v))?;
        Ok::<_, GraphError>((b.build(), sources))
    };
    let (g, s) = factory().unwrap();
    let mut d = ListStreamScoreDirector::new(&p, g, s, p.initial_assignment()).unwrap();
    let movement = ListMove::Swap {
        left: ListPosition {
            entity: 0,
            index: 0,
        },
        right: ListPosition {
            entity: 0,
            index: 2,
        },
    };
    let before = d.score();
    let count;
    {
        let mut outer = d.edit_session().unwrap();
        outer.apply(&movement).unwrap();
        assert_eq!(outer.score(), SimpleScore::new(14));
        {
            let mut inner = outer.nested().unwrap();
            inner.apply(&movement).unwrap();
            assert_eq!(inner.score(), before);
        }
        assert_eq!(outer.score(), SimpleScore::new(14));
        count = calls.get();
    }
    assert_eq!(calls.get(), count);
    assert_eq!(d.score(), before);
    let (g, s) = factory().unwrap();
    let prior_projections = projects.get();
    let result = list_stream_local_search_from(
        &p,
        g,
        s,
        p.initial_assignment(),
        LocalSearchConfig {
            max_steps: 1,
            max_evaluations: 100,
            selection: MoveSelection::BestAccepted,
            assert_scores: false,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        projects.get() - prior_projections,
        result.evaluations,
        "saved winner installs without replay"
    );
    assert_eq!(result.best.unwrap().score, SimpleScore::new(14));
    // Exercise a later move after restoring the nested snapshots.
    d.apply(&movement).unwrap().commit();
    assert_eq!(d.score(), SimpleScore::new(14));
    d.assert_consistent().unwrap();
}
