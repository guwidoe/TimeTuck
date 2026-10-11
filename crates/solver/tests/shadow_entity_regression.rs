//! Original entity notification, publication, rollback and indexed-work checks.
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;
struct Solution;
struct Entity;
struct OtherEntity;
fn schema() -> PlanningSchema {
    let fields = || {
        vec![
            PlanningVariableDefinition::basic::<i64>("input", true),
            PlanningVariableDefinition::shadow::<i64>("a"),
            PlanningVariableDefinition::shadow::<i64>("b"),
        ]
    };
    PlanningSchema::new::<Solution>(vec![
        PlanningEntityDefinition::of::<Entity>(fields()),
        PlanningEntityDefinition::of::<OtherEntity>(fields()),
    ])
    .unwrap()
}
fn owners<K: Eq + Clone, W>(
    session: &ShadowSession<'_, '_, K, W>,
    groups: Vec<Vec<ShadowNodeRef>>,
) -> Vec<K> {
    groups
        .iter()
        .map(|g| session.entity(&g[0]).unwrap().clone())
        .collect()
}
#[test]
fn canonical_types_separate_equal_keys_and_foreign_notifications_are_atomic() {
    let schema = schema();
    let entity = schema.entity::<Entity>().unwrap();
    let other = schema.entity::<OtherEntity>().unwrap();
    let mut builder = ShadowSessionBuilder::<usize, ()>::new(&schema);
    let a = builder
        .derive(entity.variable("a").unwrap(), 0, |_, _| Some(1i64))
        .unwrap();
    let b = builder
        .derive(other.variable("a").unwrap(), 0, |_, _| Some(2i64))
        .unwrap();
    let c = builder
        .derive(entity.variable("b").unwrap(), 0, |_, _| Some(3i64))
        .unwrap();
    let mut foreign = ShadowSessionBuilder::<usize, ()>::new(&schema);
    let foreign = foreign
        .derive(entity.variable("a").unwrap(), 0, |_, _| Some(1i64))
        .unwrap();
    let mut session = builder.build();
    assert!(matches!(
        session.changed_entities(),
        Err(ShadowSessionError::NotCurrent)
    ));
    let names = |nodes: Vec<ShadowNodeRef>| {
        nodes
            .iter()
            .map(|n| session.variable(n).unwrap().name().to_owned())
            .collect::<Vec<_>>()
    };
    assert_eq!(names(session.entity_nodes(&a.erase()).unwrap()), ["a", "b"]);
    assert_eq!(names(session.entity_nodes(&b.erase()).unwrap()), ["a"]);
    let groups = session
        .affected_entities(&[c.erase(), b.erase(), a.erase(), c.erase()])
        .unwrap();
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].len(), 2);
    assert_eq!(groups[1].len(), 1);
    assert_eq!(
        session
            .variable(&groups[0][0])
            .unwrap()
            .entity()
            .entity_type(),
        PlanningType::of::<Entity>()
    );
    assert_eq!(
        session
            .variable(&groups[1][0])
            .unwrap()
            .entity()
            .entity_type(),
        PlanningType::of::<OtherEntity>()
    );
    session.update(&()).unwrap();
    assert_eq!(
        session.mark_entities_dirty(&[a.erase(), foreign.erase()]),
        Err(ShadowSessionError::ForeignNode)
    );
    assert_eq!(session.read(&a).unwrap().value, Some(&1));
    assert_eq!(session.changed_entities().unwrap().len(), 2);
    assert!(matches!(
        session.entity_nodes(&foreign.erase()),
        Err(ShadowSessionError::ForeignNode)
    ));
    session
        .mark_entities_dirty(&[a.erase(), c.erase(), a.erase()])
        .unwrap();
    assert!(matches!(
        session.changed_entities(),
        Err(ShadowSessionError::NotCurrent)
    ));
    session.update(&()).unwrap();
    assert!(session.changed_entities().unwrap().is_empty());
}
#[test]
fn owner_notifications_recompute_external_facts_and_reach_other_owners() {
    let schema = schema();
    let entity = schema.entity::<Entity>().unwrap();
    let calls = [Cell::new(0), Cell::new(0), Cell::new(0)];
    let mut builder = ShadowSessionBuilder::<usize, i64>::new(&schema);
    let a = builder
        .derive(entity.variable("a").unwrap(), 0, |w, _| {
            calls[0].set(calls[0].get() + 1);
            Some(*w)
        })
        .unwrap();
    let b = builder
        .derive(entity.variable("b").unwrap(), 0, |_, _| {
            calls[1].set(calls[1].get() + 1);
            Some(7i64)
        })
        .unwrap();
    let input = a.clone();
    let count = &calls[2];
    let c = builder
        .derive(entity.variable("a").unwrap(), 1, move |_, inputs| {
            count.set(count.get() + 1);
            inputs.get(&input).unwrap().map(|v| v * 2)
        })
        .unwrap();
    builder.set_dependencies(&c, &[a.erase()]).unwrap();
    let mut session = builder.build();
    session.update(&2).unwrap();
    for call in &calls {
        call.set(0);
    }
    // World facts not represented by readers need an explicit notification.
    session.update(&3).unwrap();
    assert!(session.changed_entities().unwrap().is_empty());
    session
        .mark_entities_dirty(&[a.erase(), b.erase()])
        .unwrap();
    session.update(&3).unwrap();
    assert_eq!(calls.each_ref().map(Cell::get), [1, 1, 1]);
    assert_eq!(session.read(&a).unwrap().value, Some(&3));
    assert_eq!(session.read(&b).unwrap().value, Some(&7));
    assert_eq!(session.read(&c).unwrap().value, Some(&6));
    let groups = session.changed_entities().unwrap();
    assert_eq!(owners(&session, groups.clone()), [0, 1]);
    assert_eq!(groups[0].len(), 2);
    for call in &calls {
        call.set(0);
    }
    session.mark_entities_dirty(&[a.erase()]).unwrap();
    session.update(&3).unwrap();
    assert_eq!(calls.each_ref().map(Cell::get), [1, 1, 0]);
    assert!(session.changed_entities().unwrap().is_empty());
}
#[test]
fn change_reports_clear_on_no_change_and_follow_checkpoint_and_cycle_recovery() {
    let schema = schema();
    let entity = schema.entity::<Entity>().unwrap();
    let mut builder = ShadowSessionBuilder::<usize, i64>::new(&schema);
    let input = builder
        .input(entity.variable("input").unwrap(), 9, |w| Some(*w))
        .unwrap();
    let a = builder
        .derive(entity.variable("a").unwrap(), 0, |_, _| Some(1i64))
        .unwrap();
    let b = builder
        .derive(entity.variable("a").unwrap(), 1, |_, _| Some(2i64))
        .unwrap();
    let healthy = builder
        .derive(entity.variable("b").unwrap(), 0, |_, _| Some(3i64))
        .unwrap();
    let absent = builder
        .derive::<i64>(entity.variable("b").unwrap(), 2, |_, _| None)
        .unwrap();
    let mut session = builder.build();
    session.update(&1).unwrap();
    assert_eq!(
        owners(&session, session.changed_entities().unwrap()),
        [0, 1]
    );
    let saved = session.checkpoint().unwrap();
    session.update(&2).unwrap();
    assert_eq!(session.read(&input).unwrap().value, Some(&2));
    assert!(session.changed_entities().unwrap().is_empty());
    assert_eq!(
        session.restore(&saved, &2),
        Err(ShadowSessionError::SnapshotMismatch)
    );
    assert!(session.changed_entities().unwrap().is_empty());
    session.restore(&saved, &1).unwrap();
    assert_eq!(
        owners(&session, session.changed_entities().unwrap()),
        [0, 1]
    );
    session.set_dependencies(&a, &[b.erase()]).unwrap();
    session.set_dependencies(&b, &[a.erase()]).unwrap();
    session.update(&1).unwrap();
    assert!(session.read(&a).unwrap().inconsistent);
    assert!(session.read(&b).unwrap().inconsistent);
    assert!(!session.read(&healthy).unwrap().inconsistent);
    assert_eq!(session.read(&healthy).unwrap().value, Some(&3));
    assert_eq!(session.read(&absent).unwrap().value, None);
    assert_eq!(
        owners(&session, session.changed_entities().unwrap()),
        [0, 1]
    );
    session.update(&1).unwrap();
    assert!(session.changed_entities().unwrap().is_empty());
    session.set_dependencies(&a, &[]).unwrap();
    session.update(&1).unwrap();
    assert_eq!(
        owners(&session, session.changed_entities().unwrap()),
        [0, 1]
    );
    assert!(!session.read(&b).unwrap().inconsistent);
}
#[test]
fn failed_refresh_retains_reports_and_pending_owner_notifications_for_retry() {
    let schema = schema();
    let entity = schema.entity::<Entity>().unwrap();
    let fail = Cell::new(false);
    let mut builder = ShadowSessionBuilder::<usize, i64>::new(&schema);
    let a = builder
        .derive(entity.variable("a").unwrap(), 0, |w, _| Some(*w))
        .unwrap();
    let b = builder
        .derive(entity.variable("a").unwrap(), 1, |w, _| {
            assert!(!fail.get(), "supplier failed");
            Some(*w + 1)
        })
        .unwrap();
    let mut session = builder.build();
    session.update(&1).unwrap();
    session
        .mark_entities_dirty(&[a.erase(), b.erase()])
        .unwrap();
    fail.set(true);
    assert!(catch_unwind(AssertUnwindSafe(|| session.update(&5))).is_err());
    assert!(matches!(
        session.changed_entities(),
        Err(ShadowSessionError::NotCurrent)
    ));
    fail.set(false);
    session.update(&5).unwrap();
    assert_eq!(
        owners(&session, session.changed_entities().unwrap()),
        [0, 1]
    );
    assert_eq!(session.read(&a).unwrap().value, Some(&5));
    session.update(&5).unwrap();
    assert!(session.changed_entities().unwrap().is_empty());
}
#[test]
fn solution_projection_observes_changed_owners_and_failure_restores_the_report() {
    let schema = schema();
    let entity = schema.entity::<Entity>().unwrap();
    let fail = Cell::new(false);
    let mut builder = ShadowSessionBuilder::<usize, i64>::new(&schema);
    let input = builder
        .input(entity.variable("input").unwrap(), 0, |w| Some(*w))
        .unwrap();
    let source = input.clone();
    let a = builder
        .derive(entity.variable("a").unwrap(), 0, move |_, inputs| {
            inputs.get(&source).unwrap().copied()
        })
        .unwrap();
    builder.set_dependencies(&a, &[input.erase()]).unwrap();
    let output = a.clone();
    let mut manager = ShadowSolutionManager::new(
        builder.build(),
        move |_, session| {
            Ok::<_, &str>((
                *session.read(&output).unwrap().value.unwrap(),
                owners(session, session.changed_entities().unwrap()),
            ))
        },
        |_, _| Ok(()),
        |_, h: &(i64, Vec<usize>)| {
            if fail.get() {
                Err("score failed")
            } else {
                Ok(h.0)
            }
        },
    );
    let mut solution = ManagedSolution::new(1, (0, vec![]));
    manager.update(&mut solution).unwrap();
    assert_eq!(solution.shadows, (1, vec![0]));
    // A rejected no-change refresh must restore a previously nonempty report.
    fail.set(true);
    assert!(manager.update(&mut solution).is_err());
    assert_eq!(
        owners(
            manager.session(),
            manager.session().changed_entities().unwrap()
        ),
        [0]
    );
    fail.set(false);
    manager.update(&mut solution).unwrap();
    assert_eq!(solution.shadows, (1, vec![]));
    solution.assignment = 3;
    fail.set(true);
    assert!(manager.update(&mut solution).is_err());
    assert_eq!(solution.shadows, (1, vec![]));
    assert!(manager.session().changed_entities().unwrap().is_empty());
    assert_eq!(manager.session().read(&a).unwrap().value, Some(&1));
    fail.set(false);
    manager.update(&mut solution).unwrap();
    assert_eq!(solution.shadows, (3, vec![0]));
    assert_eq!(
        owners(
            manager.session(),
            manager.session().changed_entities().unwrap()
        ),
        [0]
    );
}
#[test]
fn owner_queries_and_notifications_do_not_compare_or_clone_the_key_universe() {
    struct Key {
        value: usize,
        comparisons: Rc<Cell<usize>>,
    }
    impl PartialEq for Key {
        fn eq(&self, other: &Self) -> bool {
            self.comparisons.set(self.comparisons.get() + 1);
            self.value == other.value
        }
    }
    impl Eq for Key {}
    let comparisons = Rc::new(Cell::new(0));
    let schema = schema();
    let entity = schema.entity::<Entity>().unwrap();
    let mut builder = ShadowSessionBuilder::<Key, ()>::new(&schema);
    let mut selected = Vec::new();
    for owner in 0..1024 {
        for name in ["a", "b"] {
            let node = builder
                .derive(
                    entity.variable(name).unwrap(),
                    Key {
                        value: owner,
                        comparisons: comparisons.clone(),
                    },
                    |_, _| Some(1i64),
                )
                .unwrap();
            if owner == 511 {
                selected.push(node);
            }
        }
    }
    let mut session = builder.build();
    session.update(&()).unwrap();
    comparisons.set(0);
    assert_eq!(session.entity_nodes(&selected[0].erase()).unwrap().len(), 2);
    assert_eq!(
        session
            .affected_entities(&[selected[1].erase(), selected[0].erase()])
            .unwrap()
            .len(),
        1
    );
    session
        .mark_entities_dirty(&[selected[0].erase(), selected[1].erase()])
        .unwrap();
    session.update(&()).unwrap();
    assert!(session.changed_entities().unwrap().is_empty());
    assert_eq!(comparisons.get(), 0);
}
