use std::{
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
struct Solution;
struct Owner;
fn schema<V: 'static>(optional: bool) -> PlanningSchema {
    PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Owner>(vec![
        PlanningVariableDefinition::list::<V>("visits", optional),
    ])])
    .unwrap()
}
#[test]
fn pending_tracker_matches_map_through_churn_without_cloning_actions() {
    struct Action(Box<usize>); // Intentionally not Clone.
    let mut actual = PendingListChanges::new();
    let mut expected = BTreeMap::new();
    let mut code = 37u64;
    for step in 0..4000 {
        code = code.wrapping_mul(6364136223846793005).wrapping_add(1);
        let key = (code >> 32) % 17;
        match code % 5 {
            0 => {
                actual.clear();
                expected.clear();
                assert_eq!(actual.storage(), PendingListChangeStorage::Empty);
            }
            1 | 2 => {
                let item = Action(Box::new(step));
                let pointer = (&*item.0) as *const usize;
                let result = actual.insert(key, item);
                if let std::collections::btree_map::Entry::Vacant(e) = expected.entry(key) {
                    e.insert((step, pointer));
                    assert!(result.is_ok());
                } else {
                    assert_eq!(result, Err(ListRecordingError::AlreadyOpen));
                }
            }
            _ => {
                let got = actual.remove(&key);
                let want = expected.remove(&key);
                assert_eq!(got.as_ref().map(|a| (*a.0, (&*a.0) as *const usize)), want);
            }
        }
        assert_eq!(actual.len(), expected.len());
        assert_eq!(actual.is_empty(), expected.is_empty());
        for (key, &(value, pointer)) in &expected {
            let item = actual.get(key).unwrap();
            assert_eq!(*item.0, value);
            assert_eq!((&*item.0) as *const usize, pointer);
        }
    }
}
#[test]
fn rebasing_open_interleaved_edits_preserves_notifications_and_independence() {
    let s = schema::<i32>(false);
    let f = s.entity::<Owner>().unwrap().variable("visits").unwrap();
    let r = ListChangeRecorder::<usize, i32>::new(f)
        .unwrap()
        .non_delegating();
    assert_eq!(r.before(0, 0, 1, &[1]).unwrap(), None);
    r.before(1, 0, 1, &[3]).unwrap();
    r.after(0, 0, 1, &[2]).unwrap();
    let mut entity_calls = vec![];
    let mut value_calls = vec![];
    let rebased = r
        .rebase(
            f,
            |e| {
                entity_calls.push(*e);
                Ok::<_, ()>(e + 10)
            },
            |v| {
                value_calls.push(*v);
                Ok(v + 100)
            },
        )
        .unwrap();
    assert_eq!(entity_calls, [0, 1]);
    assert_eq!(value_calls, [1, 2, 3]);
    let undo = rebased.undo_handle();
    let mut target = BTreeMap::from([(10, vec![102]), (11, vec![104])]);
    assert_eq!(
        undo.execute(&mut target, |_, _| panic!("unclosed")),
        Err(ListRecordingError::UnclosedBracket)
    );
    assert_eq!(rebased.after(11, 0, 1, &[104]).unwrap(), None);
    let mut events = vec![];
    undo.execute(&mut target, |e, _| events.push(e)).unwrap();
    assert_eq!(target, BTreeMap::from([(10, vec![101]), (11, vec![103])]));
    assert_eq!(events.len(), 5);
    assert!(matches!(
        events[0],
        ListChangeNotification::Before { entity: 11, .. }
    ));
    assert!(matches!(
        events[2],
        ListChangeNotification::Before { entity: 10, .. }
    ));
    let mut original = BTreeMap::from([(0, vec![2]), (1, vec![4])]);
    assert_eq!(
        r.undo_changes(&mut original, |_, _| {}),
        Err(ListRecordingError::UnclosedBracket)
    );
    r.after(1, 0, 1, &[4]).unwrap();
    r.undo_changes(&mut original, |_, _| {}).unwrap();
    assert_eq!(original, BTreeMap::from([(0, vec![1]), (1, vec![3])]));
    rebased.before(10, 0, 1, &[101]).unwrap();
    rebased.after(10, 0, 1, &[105]).unwrap();
    target.insert(10, vec![105]);
    undo.execute(&mut target, |_, _| {}).unwrap();
    assert_eq!(target[&10], [101]);
}
#[test]
fn mappings_are_memoized_and_can_change_native_types() {
    let s = schema::<i32>(false);
    let f = s.entity::<Owner>().unwrap().variable("visits").unwrap();
    let target_s = schema::<String>(false);
    let target_f = target_s
        .entity::<Owner>()
        .unwrap()
        .variable("visits")
        .unwrap();
    let r = ListChangeRecorder::<usize, i32>::new(f).unwrap();
    r.before(0, 0, 1, &[1, 3]).unwrap();
    r.after(0, 0, 1, &[2, 3]).unwrap();
    r.before(0, 0, 1, &[2, 3]).unwrap();
    r.after(0, 0, 1, &[4, 3]).unwrap();
    let mut e_calls = 0;
    let mut v_calls = vec![];
    let rebased = r
        .rebase(
            target_f,
            |_| {
                e_calls += 1;
                Ok::<_, ()>("target".to_owned())
            },
            |v| {
                v_calls.push(*v);
                Ok(format!("value-{v}"))
            },
        )
        .unwrap();
    assert_eq!(e_calls, 1);
    assert_eq!(v_calls, [1, 3, 2, 4]);
    let mut target = BTreeMap::from([(
        "target".to_owned(),
        vec!["value-4".to_owned(), "value-3".to_owned()],
    )]);
    rebased
        .undo_changes(&mut target, |e, _| match e {
            ListChangeNotification::Before { variable, .. }
            | ListChangeNotification::After { variable, .. } => assert_eq!(variable, target_f),
            _ => {}
        })
        .unwrap();
    assert_eq!(target["target"], ["value-1", "value-3"]);
    r.before(0, 0, 1, &[4, 3]).unwrap();
    r.after(0, 0, 1, &[5, 3]).unwrap();
    let mut source = BTreeMap::from([(0, vec![5, 3])]);
    r.undo_changes(&mut source, |_, _| {}).unwrap();
    assert_eq!(source[&0], [1, 3]);
}
#[test]
fn failed_rebases_do_not_consume_source_and_reentrant_mutations_are_busy() {
    let s = schema::<i32>(false);
    let f = s.entity::<Owner>().unwrap().variable("visits").unwrap();
    let r = ListChangeRecorder::<usize, i32>::new(f).unwrap();
    for (e, before, after) in [(0, 1, 2), (1, 3, 4)] {
        r.before(e, 0, 1, &[before]).unwrap();
        r.after(e, 0, 1, &[after]).unwrap();
    }
    assert!(matches!(
        r.rebase(f, |_| Ok::<_, ()>(0usize), |v| Ok(*v)),
        Err(ListRecordingRebaseError::EntityCollision)
    ));
    assert!(matches!(
        r.rebase(f, |e| Ok::<_, ()>(*e), |_| Ok(0i32)),
        Err(ListRecordingRebaseError::ValueCollision)
    ));
    assert!(matches!(
        r.rebase(f, |e| Ok(*e), |_| Err::<i32, _>("missing")),
        Err(ListRecordingRebaseError::Lookup("missing"))
    ));
    assert!(
        catch_unwind(AssertUnwindSafe(|| r.rebase(
            f,
            |e| Ok::<_, ()>(*e),
            |_| -> Result<i32, ()> { panic!("lookup") }
        )))
        .is_err()
    );
    let wrong = schema::<i32>(true);
    let wrong_f = wrong.entity::<Owner>().unwrap().variable("visits").unwrap();
    assert!(matches!(
        r.rebase(
            wrong_f,
            |_| -> Result<usize, ()> { panic!("metadata first") },
            |v| Ok(*v)
        ),
        Err(ListRecordingRebaseError::Recording(
            ListRecordingError::Metadata
        ))
    ));
    let rebased = r
        .rebase(
            f,
            |e| {
                assert_eq!(r.before(7, 0, 0, &[]), Err(ListRecordingError::Busy));
                Ok::<_, ()>(*e)
            },
            |v| Ok(*v),
        )
        .unwrap();
    let mut source = BTreeMap::from([(0, vec![2]), (1, vec![4])]);
    let mut target = source.clone();
    rebased.undo_changes(&mut target, |_, _| {}).unwrap();
    r.undo_changes(&mut source, |_, _| {}).unwrap();
    assert_eq!(source, target);
    assert_eq!(source, BTreeMap::from([(0, vec![1]), (1, vec![3])]));
}
