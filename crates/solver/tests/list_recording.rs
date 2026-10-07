//! Original integration and failure atomicity tests for recorded list edits.
use std::{
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
struct Solution;
struct Owner;
fn schema() -> PlanningSchema {
    PlanningSchema::new::<Solution>(vec![
        PlanningEntityDefinition::of::<Owner>(vec![PlanningVariableDefinition::list::<i32>(
            "visits", false,
        )]),
        PlanningEntityDefinition::of::<i32>(vec![
            PlanningVariableDefinition::shadow::<usize>("owner"),
            PlanningVariableDefinition::shadow::<usize>("index"),
        ]),
    ])
    .unwrap()
}
#[test]
fn committed_native_moves_undo_into_published_shadows_and_stream_score() {
    let s = schema();
    let field = s.entity::<Owner>().unwrap().variable("visits").unwrap();
    let p = ListProblem::new(
        vec![1, 2, 3],
        vec![
            ListEntity::new(vec![1, 2]).with_pinned_prefix(1),
            ListEntity::new(vec![3]),
        ],
    )
    .unwrap();
    let bindings = [(10usize, 0), (20, 1)];
    let mut assignment = p.initial_assignment();
    let original = assignment.clone();
    let mut registry = VariableStateRegistry::new(&s);
    registry
        .synchronize_lists(field, &assignment, &bindings)
        .unwrap();
    let mut supply =
        ListStateSupply::from_problem(registry.list::<usize, i32>(field).unwrap(), &p, &bindings)
            .unwrap();
    for (property, name) in [
        (ListShadowProperty::Owner, "owner"),
        (ListShadowProperty::Index, "index"),
    ] {
        supply
            .externalize(property, s.entity::<i32>().unwrap().variable(name).unwrap())
            .unwrap();
    }
    let mut builder = StreamGraphBuilder::<SimpleScore>::new();
    let source = builder.source::<i32, (usize, usize)>();
    builder
        .score("position", source.port(), |(owner, index)| {
            SimpleScore::new(-((*owner + *index) as i64))
        })
        .unwrap();
    let mut graph = builder.build();
    let refresh = |supply: &mut ListStateSupply<'_, usize, i32>,
                   graph: &mut StreamGraph<'_, SimpleScore>| {
        supply.refresh_all().unwrap();
        let mut tx = graph.transaction();
        for v in [1, 2, 3] {
            let shadow = supply.shadow_values(&v).unwrap();
            tx.apply(
                &source,
                TupleChange::put(v, (shadow.owner.unwrap(), shadow.index.unwrap())),
            )
            .unwrap();
        }
        tx.commit().unwrap();
    };
    refresh(&mut supply, &mut graph);
    assert_eq!(graph.score(), SimpleScore::new(-41));
    let recorder = ListChangeRecorder::<usize, i32>::new(field).unwrap();
    let undo = recorder.undo_handle();
    recorder
        .before(10, 1, 2, assignment.list(0).unwrap())
        .unwrap();
    recorder
        .before(20, 1, 1, assignment.list(1).unwrap())
        .unwrap();
    p.apply(
        &mut assignment,
        &ListMove::Change {
            source: ListPosition {
                entity: 0,
                index: 1,
            },
            destination: ListPosition {
                entity: 1,
                index: 1,
            },
        },
    )
    .unwrap()
    .commit();
    recorder
        .after(10, 1, 1, assignment.list(0).unwrap())
        .unwrap();
    recorder
        .after(20, 1, 2, assignment.list(1).unwrap())
        .unwrap();
    registry
        .synchronize_lists(field, &assignment, &bindings)
        .unwrap();
    refresh(&mut supply, &mut graph);
    assert_eq!(graph.score(), SimpleScore::new(-51));
    assert_eq!(supply.shadow_values(&2).unwrap().owner, Some(20));
    let mut lists = BTreeMap::from([
        (10, assignment.list(0).unwrap().to_vec()),
        (20, assignment.list(1).unwrap().to_vec()),
    ]);
    let mut updates = 0;
    undo.execute(&mut lists, |event, working| {
        if event == ListChangeNotification::UpdateShadows {
            // Publish the whole field once: cross-list replay can temporarily duplicate values.
            registry
                .update_lists(field, working.iter().map(|(&e, v)| (e, v.clone())))
                .unwrap();
            refresh(&mut supply, &mut graph);
            updates += 1;
        }
    })
    .unwrap();
    assignment = ListAssignment::new(vec![lists[&10].clone(), lists[&20].clone()]);
    p.validate(&assignment).unwrap();
    assert_eq!(assignment, original);
    assert_eq!(updates, 1);
    assert_eq!(graph.score(), SimpleScore::new(-41));
    assert_eq!(supply.shadow_values(&2).unwrap().owner, Some(10));
    assert_eq!(supply.shadow_values(&2).unwrap().index, Some(1));
    assert!(supply.is_pinned(&1));
}
#[test]
fn invalid_replay_and_panicking_callbacks_preserve_lists_and_live_records() {
    let s = schema();
    let f = s.entity::<Owner>().unwrap().variable("visits").unwrap();
    let r = ListChangeRecorder::<usize, i32>::new(f).unwrap();
    let mut lists = BTreeMap::from([(0, vec![1, 2]), (1, vec![3])]);
    for (e, value) in [(0, 4), (1, 5)] {
        r.before(e, 0, 1, &lists[&e]).unwrap();
        lists.get_mut(&e).unwrap()[0] = value;
        r.after(e, 0, 1, &lists[&e]).unwrap();
    }
    let changed = lists.clone();
    // The first reverse edit is valid; a later bad edit still emits no callbacks.
    lists.get_mut(&0).unwrap()[0] = 99;
    assert_eq!(
        r.undo_changes(&mut lists, |_, _| panic!("not validated")),
        Err(ListRecordingError::StateMismatch)
    );
    assert_eq!(lists[&0], [99, 2]);
    lists = changed.clone();
    let removed = lists.remove(&0).unwrap();
    assert_eq!(
        r.undo_changes(&mut lists, |_, _| panic!("not validated")),
        Err(ListRecordingError::UnknownEntity)
    );
    lists.insert(0, removed);
    for panic_at in 0..5 {
        let mut calls = 0;
        assert!(
            catch_unwind(AssertUnwindSafe(|| r.undo_changes(&mut lists, |_, _| {
                assert_eq!(r.before(8, 0, 0, &[]), Err(ListRecordingError::Busy));
                let n = calls;
                calls += 1;
                assert_ne!(n, panic_at, "callback failure");
            })))
            .is_err()
        );
        assert_eq!(lists, changed);
    }
    let mut snapshots = vec![];
    r.undo_changes(&mut lists, |e, w| snapshots.push((e, w.clone())))
        .unwrap();
    assert_eq!(snapshots.len(), 5);
    assert_eq!(snapshots[0].1, changed);
    assert_eq!(snapshots[1].1[&1], [3]);
    assert_eq!(snapshots[2].1, snapshots[1].1);
    assert_eq!(snapshots[3].1, lists);
    assert_eq!(snapshots[4].1, lists);
    assert_eq!(lists, BTreeMap::from([(0, vec![1, 2]), (1, vec![3])]));
    r.before(0, 0, 1, &lists[&0]).unwrap();
}
#[test]
fn invalid_ranges_and_outside_mutations_leave_bracket_repairable() {
    let s = schema();
    let f = s.entity::<Owner>().unwrap().variable("visits").unwrap();
    assert!(matches!(
        ListChangeRecorder::<usize, u32>::new(f),
        Err(ListRecordingError::Metadata)
    ));
    assert!(matches!(
        ListChangeRecorder::<usize, usize>::new(
            s.entity::<i32>().unwrap().variable("owner").unwrap()
        ),
        Err(ListRecordingError::Metadata)
    ));
    let r = ListChangeRecorder::<usize, i32>::new(f).unwrap();
    let n = r.non_delegating();
    assert_eq!(
        n.before(0, 2, 1, &[1, 2, 3]),
        Err(ListRecordingError::InvalidRange {
            from: 2,
            to: 1,
            length: 3
        })
    );
    assert_eq!(n.before(0, 1, 2, &[1, 2, 3]).unwrap(), None);
    assert_eq!(
        r.after(0, 1, 4, &[1, 4, 3]),
        Err(ListRecordingError::InvalidRange {
            from: 1,
            to: 4,
            length: 3
        })
    );
    assert_eq!(
        r.after(0, 1, 2, &[9, 4, 3]),
        Err(ListRecordingError::ChangedOutsideRange)
    );
    assert_eq!(
        r.after(0, 1, 2, &[1, 4, 9]),
        Err(ListRecordingError::ChangedOutsideRange)
    );
    assert!(r.after(0, 1, 2, &[1, 4, 3]).unwrap().is_some());
    let mut lists = BTreeMap::from([(0, vec![1, 4, 3])]);
    n.undo_changes(&mut lists, |_, _| {}).unwrap();
    assert_eq!(lists[&0], [1, 2, 3]);
}
