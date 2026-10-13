// Adapted from MockProblemChangeDirectorTest.problemChange, Apache-2.0.
// All original native controls below earn no additional source-method credit.
#[path = "fixtures/problem_alias_fixture.rs"]
mod fixture;
use fixture::*;
use std::{
    sync::{Arc, mpsc},
    time::Duration,
};
use timetuck::*;
#[test]
fn problem_change() {
    let eg = Arc::new(String::from("entityGroupOne"));
    let vg = Arc::new(String::from("valueGroupOne"));
    let added_entity = item("newly added entity", eg.clone(), None);
    let removed_entity = item("entity to remove", eg.clone(), None);
    let added_fact = item("newly added fact", vg.clone(), None);
    let removed_fact = item("fact to remove", vg.clone(), None);
    let changed_entity = item("changed entity", eg.clone(), None);
    let changed_fact = item("changed entity value", vg.clone(), None);
    let removed_working_entity = item("working entity to remove", eg.clone(), None);
    let removed_working_fact = item("working fact to remove", vg, None);
    let changed_working_entity = item("working changed entity", eg, None);
    let mut bindings = Bindings::default();
    bindings.bind(
        ProblemObjectKind::Entity,
        removed_entity.clone(),
        removed_working_entity.clone(),
    );
    bindings.bind(
        ProblemObjectKind::Fact,
        removed_fact.clone(),
        removed_working_fact.clone(),
    );
    bindings.bind(
        ProblemObjectKind::Entity,
        changed_entity.clone(),
        changed_working_entity.clone(),
    );
    let mut p = Problem::generated();
    assert_eq!(
        (
            p.fact_groups.len(),
            p.facts.len(),
            p.entity_groups.len(),
            p.entities.len()
        ),
        (2, 5, 3, 7)
    );
    for (i, e) in p.entities.iter().enumerate() {
        assert_eq!(e.code, format!("Generated Entity {i}"));
        let state = e.state.lock().unwrap();
        assert_eq!(state.long, 0); // Source random long property is irrelevant here.
        assert!(Arc::ptr_eq(state.value.as_ref().unwrap(), &p.facts[i % 5]));
        assert!(Arc::ptr_eq(
            state.group.as_ref().unwrap(),
            &p.entity_groups[i % 3]
        ));
    }
    for (i, f) in p.facts.iter().enumerate() {
        assert!(Arc::ptr_eq(
            f.state.lock().unwrap().group.as_ref().unwrap(),
            &p.fact_groups[i % 2]
        ));
    }

    p.entities.extend([
        removed_working_entity.clone(),
        changed_working_entity.clone(),
    ]);
    p.facts.push(removed_working_fact.clone());
    let mut s = session(p);
    let before = s.score().unwrap();
    s.edit_problem(
        |p, k, e| bindings.lookup(p, k, e),
        |e| {
            e.add_entity(added_entity.clone(), |p| {
                p.entities.push(added_entity.clone())
            })?;
            e.remove_entity(removed_entity.clone(), |p, i| {
                assert!(Arc::ptr_eq(&p.entities[i], &removed_working_entity));
                p.entities.remove(i);
            })?;
            e.change_variable(changed_entity.clone(), "value", |p, i| {
                assert!(Arc::ptr_eq(&p.entities[i], &changed_working_entity));
                p.entities[i].state.lock().unwrap().value = Some(changed_fact.clone());
            })?;
            e.change_property(ProblemObjectKind::Entity, changed_entity.clone(), |p, i| {
                p.entities[i].state.lock().unwrap().group = None
            })?;
            e.add_fact(added_fact.clone(), |p| p.facts.push(added_fact.clone()))?;
            e.remove_fact(removed_fact.clone(), |p, i| {
                assert!(Arc::ptr_eq(&p.facts[i], &removed_working_fact));
                p.facts.remove(i);
            })
        },
    )
    .unwrap();
    // Every source final assertion, observing actual canonical allocations.
    assert!(
        !s.solution()
            .entities
            .iter()
            .any(|e| Arc::ptr_eq(e, &removed_working_entity))
    );
    assert!(
        !s.solution()
            .facts
            .iter()
            .any(|e| Arc::ptr_eq(e, &removed_working_fact))
    );
    assert!(Arc::ptr_eq(
        changed_working_entity
            .state
            .lock()
            .unwrap()
            .value
            .as_ref()
            .unwrap(),
        &changed_fact
    ));
    assert!(changed_working_entity.state.lock().unwrap().group.is_none());
    // Original scoring/callback and alias ownership controls around the full edit.
    assert_eq!(s.score().unwrap(), before);
    assert_eq!(s.score().unwrap(), SimpleScore::new(s.solution().score()));
    assert_eq!(s.calculator().notifications, [true, false].repeat(6));
    assert_eq!(s.calculator().resets, 2);
    assert!(changed_entity.state.lock().unwrap().value.is_none());
    assert!(changed_entity.state.lock().unwrap().group.is_some());
    assert!(
        s.solution()
            .entities
            .iter()
            .any(|e| Arc::ptr_eq(e, &added_entity))
    );
    assert!(
        s.solution()
            .facts
            .iter()
            .any(|e| Arc::ptr_eq(e, &added_fact))
    );
}
#[test]
fn alias_shifted_handles_and_real_score_change() {
    let p = Problem::generated();
    let canonical = p.entities[3].clone();
    let external = item("external", p.entity_groups[0].clone(), None);
    let removed = p.entities[0].clone();
    let mut bindings = Bindings::default();
    bindings.bind(
        ProblemObjectKind::Entity,
        external.clone(),
        canonical.clone(),
    );
    let mut s = session(p);
    let before = s.score().unwrap();
    let mut observed = vec![];
    s.edit_problem(
        |p, k, e| {
            let h = bindings.lookup(p, k, e);
            if Arc::ptr_eq(e, &external) {
                observed.push(h);
            }
            h
        },
        |e| {
            e.remove_entity(removed.clone(), |p, i| {
                p.entities.remove(i);
            })?;
            e.change_variable(external.clone(), "value", |p, i| {
                p.entities[i].state.lock().unwrap().value = None
            })?;
            e.change_property(ProblemObjectKind::Entity, external.clone(), |p, i| {
                p.entities[i].state.lock().unwrap().group = None
            })
        },
    )
    .unwrap();
    assert_eq!(
        observed,
        [ProblemLookup::Unique(2), ProblemLookup::Unique(2)]
    );
    assert_eq!(s.score().unwrap(), before - SimpleScore::new(4));
    assert_eq!(s.score().unwrap(), SimpleScore::new(s.solution().score()));
    assert!(canonical.state.lock().unwrap().value.is_none());
    assert!(canonical.state.lock().unwrap().group.is_none());
    assert!(external.state.lock().unwrap().group.is_some());
    assert!(matches!(
        bindings.lookup(s.solution(), ProblemObjectKind::Entity, &removed),
        ProblemLookup::Missing
    ));
    assert_eq!(s.calculator().notifications, [true, false].repeat(3));
    assert_eq!(s.calculator().resets, 2);
}
#[test]
fn alias_live_problem_change_receipt_follows_actual_calculator_scoring() {
    const WAIT: Duration = Duration::from_secs(10);
    let p = Problem::generated();
    let target = p.entities[2].clone();
    let external = item("external", p.entity_groups[0].clone(), None);
    let mut bindings = Bindings::default();
    bindings.bind(ProblemObjectKind::Entity, external.clone(), target.clone());
    let before = p.score();
    let (started, starting) = mpsc::channel();
    let (release, released) = mpsc::channel();
    let mut manager = LiveSolverManager::<Session, i64, String>::new(1).unwrap();
    let mut first = true;
    let job = manager
        .submit(
            1,
            session(p),
            LiveSolverConfig::default(),
            move |s, _, token| {
                if first {
                    first = false;
                    started.send(token.clone()).unwrap();
                    released.recv_timeout(WAIT).unwrap();
                }
                let mut c = Calculator::default();
                let mut registry = MatchRegistry::new(false);
                c.reset(s.solution(), &mut registry).unwrap();
                Ok(c.calculate_score().score())
            },
            |_, _| {},
        )
        .unwrap();
    let token = starting.recv_timeout(WAIT).unwrap();
    let receipt = job
        .add_problem_change(move |s| {
            s.edit_problem(
                |p, k, e| bindings.lookup(p, k, e),
                |e| {
                    e.change_variable(external.clone(), "value", |p, i| {
                        p.entities[i].state.lock().unwrap().value = None
                    })
                },
            )
            .map_err(|e| e.to_string())?;
            assert_eq!(s.score().unwrap(), SimpleScore::new(before - 1));
            assert_eq!(s.calculator().notifications, [true, false]);
            Ok(())
        })
        .unwrap();
    assert!(token.is_cancelled());
    assert_eq!(receipt.try_result(), None);
    release.send(()).unwrap();
    assert_eq!(receipt.wait_timeout(WAIT), Some(Ok(())));
    let result = job.wait_timeout(WAIT).unwrap();
    let result = result.as_ref().as_ref().unwrap();
    assert_eq!(result.revision, 1);
    assert_eq!(*result.solution, before - 1);
    assert!(target.state.lock().unwrap().value.is_none());
    manager.shutdown().unwrap();
}
