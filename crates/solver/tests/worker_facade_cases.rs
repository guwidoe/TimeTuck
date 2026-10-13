// Full pinned Community DefaultSolverWorkerFacadeTest adaptation, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use serde_json::json;
use std::{
    collections::BTreeSet,
    time::{Duration, SystemTime},
};
use timetuck::*;
#[derive(Default)]
struct Recording {
    events: Vec<WorkerDatasetEvent>,
}
impl WorkerDatasetEventSink for Recording {
    fn emit(&mut self, event: WorkerDatasetEvent) -> Result<(), String> {
        self.events.push(event);
        Ok(())
    }
}
type Facade = WorkerDatasetFacade<InMemoryWorkerDatasetStore, Recording, fn() -> SystemTime>;
fn facade() -> Facade {
    WorkerDatasetFacade::new(
        InMemoryWorkerDatasetStore::default(),
        Recording::default(),
        (|| SystemTime::UNIX_EPOCH + Duration::from_secs(123)) as fn() -> SystemTime,
    )
}
fn tags(values: &[&str]) -> BTreeSet<String> {
    values.iter().map(|v| (*v).into()).collect()
}
fn create(
    f: &mut Facade,
    name: &str,
    value: &str,
    labels: &[&str],
    solve: bool,
) -> WorkerDatasetMetadata {
    f.create_dataset(
        name.into(),
        tags(labels),
        json!({"value":value}),
        json!({}),
        solve,
    )
    .unwrap()
}
fn events(f: &Facade, kind: WorkerDatasetEventKind) -> Vec<&WorkerDatasetEvent> {
    f.sink()
        .events
        .iter()
        .filter(|e| e.kind() == kind)
        .collect()
}
fn validation(f: &Facade) -> (&str, bool) {
    match events(f, WorkerDatasetEventKind::ValidateCompute)
        .last()
        .unwrap()
    {
        WorkerDatasetEvent::ValidateCompute { id, solve } => (id.as_str(), *solve),
        _ => unreachable!(),
    }
}
fn empty_patch() -> JsonPatch {
    JsonPatch::parse("[]", JsonPatchMode::Model).unwrap()
}
#[test]
fn create_dataset_persists_and_emits_validate_compute_without_solve() {
    let mut f = facade();
    let m = create(&mut f, "run-created", "hello", &["test-tag"], false);
    assert!(!m.id.trim().is_empty());
    assert_eq!(
        events(&f, WorkerDatasetEventKind::Created),
        vec![&WorkerDatasetEvent::Created { id: m.id.clone() }]
    );
    assert_eq!(events(&f, WorkerDatasetEventKind::ValidateCompute).len(), 1);
    assert_eq!(validation(&f), (m.id.as_str(), false));
    assert!(!m.id.trim().is_empty());
    assert_eq!(m.name, "run-created");
    assert_eq!(m.status, WorkerDatasetStatus::DatasetCreated);
    assert!(m.submitted >= SystemTime::UNIX_EPOCH);
    assert_eq!(m.tags, tags(&["test-tag"]));
    let stored = f.get_dataset(&m.id).unwrap();
    assert_eq!(stored.input, json!({"value":"hello"}));
    assert_eq!(
        (stored.metadata.id, stored.metadata.name),
        (m.id, "run-created".into())
    );
}
#[test]
fn create_and_solve_dataset_emits_validate_compute_with_solve_true() {
    let mut f = facade();
    let m = create(&mut f, "run-solve", "x", &["test-tag"], true);
    assert_eq!(events(&f, WorkerDatasetEventKind::ValidateCompute).len(), 1);
    assert_eq!(validation(&f), (m.id.as_str(), true));
    assert_eq!(m.tags, tags(&["test-tag"]));
    assert_eq!(f.get_dataset(&m.id).unwrap().input, json!({"value":"x"}));
    assert_eq!(f.get_metadata(&m.id).unwrap().name, "run-solve");
}
#[test]
fn solve_dataset_sends_start_command() {
    let mut f = facade();
    let parent = create(&mut f, "run-parent", "x", &["test-tag"], false);
    f.solve_dataset(&parent.id).unwrap();
    assert_eq!(
        events(&f, WorkerDatasetEventKind::SolveStart),
        vec![&WorkerDatasetEvent::SolveStart {
            id: parent.id.clone()
        }]
    );
    let m = f.get_metadata(&parent.id).unwrap();
    assert_eq!(m.id, parent.id);
    assert_eq!(m.name, parent.name);
    assert_eq!(m.tags, tags(&["test-tag"]));
}
#[test]
fn solve_dataset_unknown_id_throws_item_not_found() {
    let mut f = facade();
    assert!(matches!(
        f.solve_dataset("does-not-exist"),
        Err(WorkerFacadeError::NotFound(_))
    ));
    assert!(events(&f, WorkerDatasetEventKind::SolveStart).is_empty());
}
#[test]
fn create_dataset_from_existing_dataset_missing_parent_throws_item_not_found() {
    let mut f = facade();
    assert!(matches!(
        f.create_from_dataset(
            "missing",
            WorkerDatasetSelector::Input,
            "run-child".into(),
            tags(&[]),
            json!({}),
            false
        ),
        Err(WorkerFacadeError::NotFound(_))
    ));
    assert!(events(&f, WorkerDatasetEventKind::ValidateCompute).is_empty());
}
#[test]
fn create_dataset_from_existing_dataset_emits_validate_compute_without_solve() {
    let mut f = facade();
    let parent = create(&mut f, "run-parent", "orig", &[], false);
    f.sink_mut().events.clear();
    let child = f
        .create_from_dataset(
            &parent.id,
            WorkerDatasetSelector::Input,
            "run-child".into(),
            tags(&["t2"]),
            json!({}),
            false,
        )
        .unwrap();
    assert_eq!(
        events(&f, WorkerDatasetEventKind::Created),
        vec![&WorkerDatasetEvent::Created {
            id: child.id.clone()
        }]
    );
    assert_eq!(events(&f, WorkerDatasetEventKind::ValidateCompute).len(), 1);
    assert!(!validation(&f).1);
    assert_eq!(child.status, WorkerDatasetStatus::DatasetCreated);
    assert_eq!(child.name, "run-child");
    assert_eq!(child.tags, tags(&["t2"]));
    assert_eq!(child.parent_id, Some(parent.id.clone()));
    assert_eq!(child.origin_id, parent.origin_id);
    let stored = f.get_dataset(&child.id).unwrap();
    assert_eq!(
        (stored.metadata.parent_id, stored.metadata.origin_id),
        (Some(parent.id.clone()), parent.origin_id)
    );
    assert_eq!(stored.input, json!({"value":"orig"}));
    assert_eq!(
        f.get_dataset(&parent.id).unwrap().input,
        json!({"value":"orig"})
    );
}
#[test]
fn create_and_solve_dataset_from_existing_dataset_emits_validate_compute_with_solve_true() {
    let mut f = facade();
    let parent = create(&mut f, "run-parent", "orig", &[], false);
    f.sink_mut().events.clear();
    let child = f
        .create_from_dataset(
            &parent.id,
            WorkerDatasetSelector::Input,
            "run-child".into(),
            tags(&["test-child-tag"]),
            json!({}),
            true,
        )
        .unwrap();
    assert_eq!(events(&f, WorkerDatasetEventKind::ValidateCompute).len(), 1);
    assert!(validation(&f).1);
    assert_eq!(child.status, WorkerDatasetStatus::DatasetCreated);
    assert_eq!(child.tags, tags(&["test-child-tag"]));
    assert_eq!(child.parent_id, Some(parent.id));
    assert_eq!(child.origin_id, parent.origin_id);
}
#[test]
fn patch_dataset_missing_parent_throws_item_not_found() {
    let mut f = facade();
    assert!(matches!(
        f.patch_dataset(
            "missing",
            WorkerDatasetSelector::Input,
            "run-patch".into(),
            None,
            &empty_patch(),
            false
        ),
        Err(WorkerFacadeError::NotFound(_))
    ));
    assert!(events(&f, WorkerDatasetEventKind::ValidateCompute).is_empty());
}
#[test]
fn patch_dataset_emits_validate_compute_without_solve() {
    let mut f = facade();
    let parent = create(&mut f, "run-parent", "orig", &[], false);
    f.sink_mut().events.clear();
    let child = f
        .patch_dataset(
            &parent.id,
            WorkerDatasetSelector::Input,
            "run-patch".into(),
            None,
            &empty_patch(),
            false,
        )
        .unwrap();
    assert_eq!(events(&f, WorkerDatasetEventKind::ValidateCompute).len(), 1);
    assert!(!validation(&f).1);
    assert_eq!(child.parent_id, Some(parent.id.clone()));
    assert_eq!(child.origin_id, parent.origin_id);
    let stored = f.get_dataset(&child.id).unwrap();
    assert_eq!(
        (stored.metadata.parent_id, stored.metadata.origin_id),
        (Some(parent.id.clone()), parent.origin_id)
    );
    assert_eq!(stored.input, json!({"value":"orig"}));
    assert_eq!(
        f.get_dataset(&parent.id).unwrap().input,
        json!({"value":"orig"})
    );
}
#[test]
fn patch_and_solve_dataset_emits_validate_compute_with_solve_true() {
    let mut f = facade();
    let parent = create(&mut f, "run-parent", "orig", &[], false);
    f.sink_mut().events.clear();
    let child = f
        .patch_dataset(
            &parent.id,
            WorkerDatasetSelector::Input,
            "run-patch".into(),
            None,
            &empty_patch(),
            true,
        )
        .unwrap();
    assert_eq!(events(&f, WorkerDatasetEventKind::ValidateCompute).len(), 1);
    assert!(validation(&f).1);
    assert_eq!(child.parent_id, Some(parent.id));
    assert_eq!(child.origin_id, parent.origin_id);
    assert_eq!(
        f.get_dataset(&child.id).unwrap().input,
        json!({"value":"orig"})
    );
}
