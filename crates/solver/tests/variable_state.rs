//! Original live score/list integration, field isolation and invalid-input regressions.
use timetuck::*;
struct Solution;
struct Job;
fn schema() -> PlanningSchema {
    PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Job>(vec![
        PlanningVariableDefinition::basic::<i32>("worker", true),
        PlanningVariableDefinition::basic::<i32>("backup", true),
        PlanningVariableDefinition::list::<i32>("visits", false),
        PlanningVariableDefinition::shadow::<i32>("finish"),
    ])])
    .unwrap()
}
#[test]
fn shared_inverse_state_tracks_scored_assignments_and_rollback() {
    let s = schema();
    let field = s.entity::<Job>().unwrap().variable("worker").unwrap();
    let mut r = VariableStateRegistry::new(&s);
    let state = r.basic::<usize, i32>(field).unwrap();
    let tracker = r.tracker::<usize, i32>(field).unwrap();
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]).allowing_unassigned(); 3]).unwrap();
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source();
    let universe = b.source::<usize, i32>();
    let inverse = basic_inverse_stream(&mut b, &source, universe.port(), Some).unwrap();
    let matches = b
        .score("balance", &inverse, |(_, owners)| {
            let delta = owners.len() as i64 - 1;
            SimpleScore::new(-delta * delta)
        })
        .unwrap();
    let mut graph = b.build();
    let mut tx = graph.transaction();
    for i in 0..2 {
        tx.apply(&universe, TupleChange::put(i, i as i32)).unwrap();
    }
    tx.commit().unwrap();
    let mut d = StreamScoreDirector::new(&p, graph, source, p.initial_assignment()).unwrap();
    let bindings = [(0usize, 0), (1, 1), (2, 2)];
    r.synchronize_basic(field, d.assignment(), &bindings)
        .unwrap();
    for code in 0..27 {
        tracker.audit([]);
        let before = state.snapshot();
        let mut digits = code;
        let values = (0..3)
            .map(|_| {
                let n = digits % 3;
                digits /= 3;
                if n == 2 { None } else { Some(n) }
            })
            .collect::<Vec<_>>();
        let movement = Move::Composite(
            values
                .iter()
                .enumerate()
                .map(|(variable, &value)| Move::Change { variable, value })
                .collect(),
        );
        let t = d.apply(&movement).unwrap();
        r.synchronize_basic(field, t.assignment(), &bindings)
            .unwrap();
        for value in 0..2 {
            let expected = values
                .iter()
                .enumerate()
                .filter_map(|(e, v)| (*v == Some(value)).then_some(e))
                .collect::<Vec<_>>();
            assert_eq!(state.inverse(&value), expected);
            assert_eq!(
                t.graph().constraint_matches(&matches).unwrap()[&(value as usize)]
                    .0
                    .1
                    .keys()
                    .copied()
                    .collect::<Vec<_>>(),
                expected
            );
        }
        let changed = values
            .iter()
            .enumerate()
            .filter_map(|(e, v)| (before[&e] != *v).then_some((field, e)));
        assert!(tracker.audit(changed).is_empty());
        if code % 2 == 0 {
            t.commit();
        } else {
            drop(t);
            r.synchronize_basic(field, d.assignment(), &bindings)
                .unwrap();
            assert_eq!(state.snapshot(), before);
        }
        assert!(state.same_instance(&r.basic(field).unwrap()));
    }
    d.assert_consistent().unwrap();
}
#[test]
fn observed_list_moves_refresh_positions_neighbors_and_restore_old_handles() {
    let s = schema();
    let v = s.entity::<Job>().unwrap().variable("visits").unwrap();
    let mut r = VariableStateRegistry::new(&s);
    let state = r.list::<&str, i32>(v).unwrap();
    let tracker = r.tracker::<&str, i32>(v).unwrap();
    let p = ListProblem::new(
        vec![10, 11, 12],
        vec![ListEntity::new(vec![10, 11]), ListEntity::new(vec![12])],
    )
    .unwrap();
    let mut assignment = p.initial_assignment();
    let bindings = [("a", 0), ("b", 1)];
    r.synchronize_lists(v, &assignment, &bindings).unwrap();
    tracker.audit([]);
    let before = state.snapshot();
    let mut observer = |event: ListChangeEvent<'_, i32>| {
        if event.phase == ListChangePhase::After {
            r.synchronize_lists(v, event.assignment, &bindings).unwrap();
        }
    };
    {
        let _trial = p
            .apply_observed(
                &mut assignment,
                &ListMove::Change {
                    source: ListPosition {
                        entity: 0,
                        index: 1,
                    },
                    destination: ListPosition {
                        entity: 1,
                        index: 0,
                    },
                },
                &mut observer,
            )
            .unwrap();
        assert_eq!(
            state.location(&11),
            Some(VariableListLocation {
                entity: "b",
                index: 0,
                previous: None,
                next: Some(12)
            })
        );
        assert_eq!(state.location(&12).unwrap().previous, Some(11));
        assert_eq!(state.location(&10).unwrap().next, None);
        assert!(tracker.audit([(v, "a"), (v, "b")]).is_empty());
    }
    assert_eq!(state.snapshot(), before);
    assert_eq!(state.location(&11).unwrap().entity, "a");
    assert_eq!(state.location(&11).unwrap().previous, Some(10));
    assert_eq!(state.location(&12).unwrap().index, 0);
    assert!(tracker.audit([(v, "a"), (v, "b")]).is_empty());
}
#[test]
fn field_resets_invalid_bindings_and_unassignment_preserve_shared_state() {
    let s = schema();
    let e = s.entity::<Job>().unwrap();
    let v = e.variable("worker").unwrap();
    let other = e.variable("backup").unwrap();
    let lists = e.variable("visits").unwrap();
    let mut r = VariableStateRegistry::new(&s);
    let t = r.tracker::<usize, i32>(v).unwrap();
    let state = r.basic::<usize, i32>(v).unwrap();
    r.reset_basic(v, [(0usize, Some(1)), (1, None)]).unwrap();
    let detached = state.snapshot();
    r.reset_basic(other, [(0usize, Some(2))]).unwrap();
    let backup = r.basic::<usize, i32>(other).unwrap();
    assert!(!state.same_instance(&backup));
    r.change_basic(v, 0usize, None::<i32>).unwrap();
    assert!(state.inverse(&1).is_empty());
    assert_eq!(detached[&0], Some(1));
    assert!(t.audit([(v, 0), (other, 0)]).is_empty());
    r.reset_basic(v, [(0usize, Some(1))]).unwrap();
    assert!(state.same_instance(&r.basic(v).unwrap()));
    assert_eq!(backup.inverse(&2), [0]);
    t.before(0);
    let before = state.snapshot();
    assert!(r.reset_basic(v, [(0usize, Some(1)), (0, Some(2))]).is_err());
    assert!(
        r.synchronize_basic(v, &Assignment::new(vec![Some(1)]), &[(0usize, 1)])
            .is_err()
    );
    assert!(
        r.synchronize_basic(v, &Assignment::new(vec![Some(1)]), &[(0usize, 0), (1, 0)])
            .is_err()
    );
    assert!(r.basic::<usize, u32>(v).is_err());
    assert!(r.basic::<String, i32>(v).is_err());
    assert!(r.list::<usize, i32>(v).is_err());
    assert!(
        r.basic::<usize, i32>(e.variable("finish").unwrap())
            .is_err()
    );
    let foreign = schema();
    assert!(
        r.basic::<usize, i32>(foreign.entity::<Job>().unwrap().variable("worker").unwrap())
            .is_err()
    );
    assert_eq!(state.snapshot(), before);
    assert_eq!(
        t.audit([(v, 0)]),
        [VariableEventViolation {
            entity: 0,
            missing: MissingVariableEvent::After
        }]
    );
    r.reset_lists(lists, [(0usize, vec![1, 2]), (1, vec![3])])
        .unwrap();
    let l = r.list::<usize, i32>(lists).unwrap();
    let old = l.snapshot();
    assert!(
        r.update_lists(lists, [(0usize, vec![1, 2]), (1, vec![2, 3])])
            .is_err()
    );
    assert_eq!(l.snapshot(), old);
    r.update_lists(lists, [(0usize, vec![1]), (1, vec![2, 3])])
        .unwrap();
    assert_eq!(l.location(&2).unwrap().entity, 1);
    r.reset_lists::<usize, i32>(lists, [(0usize, vec![]), (1, vec![])])
        .unwrap();
    assert!(!l.is_assigned(&2));
    assert!(l.same_instance(&r.list(lists).unwrap()));
}
