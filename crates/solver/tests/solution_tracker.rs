//! Original live-model diagnostics and snapshot isolation regressions.
use std::{
    cell::Cell,
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
struct Solution;
struct Route;
#[derive(Clone)]
struct World {
    assignment: ListAssignment<i32>,
    totals: BTreeMap<usize, Option<i64>>,
}
fn schema() -> PlanningSchema {
    PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Route>(vec![
        PlanningVariableDefinition::list::<i32>("visits", false),
        PlanningVariableDefinition::shadow::<i64>("total"),
    ])])
    .unwrap()
}
fn totals(assignment: &ListAssignment<i32>) -> BTreeMap<usize, Option<i64>> {
    assignment
        .lists()
        .iter()
        .enumerate()
        .map(|(i, list)| (i, Some(list.iter().map(|&v| i64::from(v)).sum())))
        .collect()
}
#[test]
fn live_moves_use_registry_events_and_explain_stale_shadow_after_undo() {
    let s = schema();
    let owner = s.entity::<Route>().unwrap();
    let list = owner.variable("visits").unwrap();
    let total = owner.variable("total").unwrap();
    let p = ListProblem::new(
        vec![1, 2, 3],
        vec![ListEntity::new(vec![1, 2]), ListEntity::new(vec![3])],
    )
    .unwrap();
    let mut w = World {
        assignment: p.initial_assignment(),
        totals: totals(&p.initial_assignment()),
    };
    let mut registry = VariableStateRegistry::new(&s);
    let bindings = [(0usize, 0), (1, 1)];
    registry
        .synchronize_lists(list, &w.assignment, &bindings)
        .unwrap();
    let mut tracker = SolutionTracker::new(&s);
    tracker
        .list(
            list,
            |w: &World| w.assignment.lists().iter().cloned().enumerate().collect(),
            i32::to_string,
        )
        .unwrap();
    tracker
        .use_events(registry.tracker::<usize, i32>(list).unwrap())
        .unwrap();
    let shadow = tracker
        .scalar(total, |w: &World| w.totals.clone(), i64::to_string)
        .unwrap();
    tracker.before_move(&w);
    let saved = w.clone();
    p.apply(
        &mut w.assignment,
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
    )
    .unwrap()
    .commit();
    registry
        .synchronize_lists(list, &w.assignment, &bindings)
        .unwrap();
    for e in 0..2 {
        shadow.before(e);
    }
    w.totals = totals(&w.assignment);
    for e in 0..2 {
        shadow.after(e);
    }
    tracker.after_move(&w).unwrap();
    // Undo the genuine lists but intentionally forget their derived totals.
    w.assignment = saved.assignment.clone();
    registry
        .synchronize_lists(list, &w.assignment, &bindings)
        .unwrap();
    tracker.after_undo(&w).unwrap();
    tracker.rebuilt_before(&saved).unwrap();
    let rebuilt = World {
        assignment: w.assignment.clone(),
        totals: totals(&w.assignment),
    };
    tracker.rebuilt_undo(&rebuilt).unwrap();
    let report = tracker.report().unwrap();
    assert!(report.forward_events.is_empty());
    assert!(
        report.undo_events.is_empty(),
        "unchanged stale shadows produce no change-event requirement"
    );
    assert_eq!(report.before_vs_undo.len(), 2);
    assert_eq!(report.rebuilt_vs_undo.len(), 2);
    assert!(report.before_vs_undo.iter().all(|v| v.variable == total));
    assert_eq!(
        (
            report.before_vs_undo[0].expected.as_str(),
            report.before_vs_undo[0].actual.as_str()
        ),
        ("3", "1")
    );
    assert_eq!(
        (
            report.before_vs_undo[1].expected.as_str(),
            report.before_vs_undo[1].actual.as_str()
        ),
        ("3", "5")
    );
    assert!(report.rebuilt_vs_before.is_empty());
    // A new move starts a fresh event window and does not retain old corruption.
    w = rebuilt;
    tracker.before_move(&w);
    tracker.after_move(&w).unwrap();
    tracker.after_undo(&w).unwrap();
    tracker.rebuilt_before(&w).unwrap();
    tracker.rebuilt_undo(&w).unwrap();
    assert!(tracker.report().unwrap().is_empty());
}
#[test]
fn snapshots_use_typed_equality_and_distinguish_missing_entities_from_null() {
    let s = schema();
    let field = s.entity::<Route>().unwrap().variable("total").unwrap();
    let mut t = SolutionTracker::new(&s);
    // Identical labels must not hide unequal values. Values need Eq, not Clone/Ord.
    #[derive(Eq, PartialEq)]
    struct Value(i64);
    struct Marker;
    let extra =
        PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Marker>(vec![
            PlanningVariableDefinition::shadow::<Value>("x"),
        ])])
        .unwrap();
    let mut nonclone = SolutionTracker::new(&extra);
    nonclone
        .scalar(
            extra.entity::<Marker>().unwrap().variable("x").unwrap(),
            |v: &i64| BTreeMap::from([(0, Some(Value(*v)))]),
            |_| "same".into(),
        )
        .unwrap();
    assert_eq!(
        nonclone
            .snapshot(&1)
            .differences(&nonclone.snapshot(&2))
            .unwrap()
            .len(),
        1
    );
    t.scalar(
        field,
        |w: &BTreeMap<usize, Option<i64>>| w.clone(),
        i64::to_string,
    )
    .unwrap();
    let a = t.snapshot(&BTreeMap::from([(0, None), (1, Some(1))]));
    let b = t.snapshot(&BTreeMap::from([(1, Some(1)), (2, None)]));
    let diff = a.differences(&b).unwrap();
    assert_eq!(diff.len(), 2);
    assert_eq!(
        (&diff[0].expected[..], &diff[0].actual[..]),
        ("null", "<missing entity>")
    );
    assert_eq!(
        (&diff[1].expected[..], &diff[1].actual[..]),
        ("<missing entity>", "null")
    );
    let mut other = SolutionTracker::new(&s);
    other
        .scalar(
            field,
            |w: &BTreeMap<usize, Option<i64>>| w.clone(),
            i64::to_string,
        )
        .unwrap();
    assert_eq!(
        a.differences(&other.snapshot(&BTreeMap::new())),
        Err(SolutionTrackingError::ForeignSnapshot)
    );
}
#[test]
fn failed_capture_preserves_event_window_and_registration_checks_canonical_fields() {
    let s = schema();
    let field = s.entity::<Route>().unwrap().variable("total").unwrap();
    let panic = Cell::new(false);
    let mut t = SolutionTracker::new(&s);
    let event = t
        .scalar(
            field,
            |w: &i64| {
                assert!(!panic.get(), "reader failure");
                BTreeMap::from([(0usize, Some(*w))])
            },
            i64::to_string,
        )
        .unwrap();
    assert!(matches!(
        t.scalar(
            field,
            |_: &i64| BTreeMap::from([(0usize, Some(1i64))]),
            i64::to_string
        ),
        Err(SolutionTrackingError::DuplicateField)
    ));
    let foreign = schema();
    assert!(matches!(
        t.scalar(
            foreign
                .entity::<Route>()
                .unwrap()
                .variable("total")
                .unwrap(),
            |_: &i64| BTreeMap::from([(0usize, Some(1i64))]),
            i64::to_string
        ),
        Err(SolutionTrackingError::Metadata)
    ));
    assert_eq!(
        t.after_move(&1),
        Err(SolutionTrackingError::MissingBeforeMove)
    );
    t.before_move(&1);
    event.before(0);
    event.after(0);
    panic.set(true);
    assert!(catch_unwind(AssertUnwindSafe(|| t.after_move(&2))).is_err());
    panic.set(false);
    t.after_move(&2).unwrap();
    assert_eq!(
        t.after_move(&2),
        Err(SolutionTrackingError::StageAlreadyCaptured)
    );
    event.before(0);
    event.after(0);
    t.after_undo(&1).unwrap();
    assert_eq!(
        t.after_undo(&1),
        Err(SolutionTrackingError::StageAlreadyCaptured)
    );
    assert!(matches!(
        t.report(),
        Err(SolutionTrackingError::MissingRebuild)
    ));
    t.rebuilt_before(&1).unwrap();
    t.rebuilt_undo(&1).unwrap();
    assert!(t.report().unwrap().is_empty());
    assert_eq!(
        t.use_events(event),
        Err(SolutionTrackingError::RegistrationClosed)
    );
}
