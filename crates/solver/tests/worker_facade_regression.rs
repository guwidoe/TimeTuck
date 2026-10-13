use serde_json::json;
use std::{cell::Cell, collections::BTreeSet, time::SystemTime};
use timetuck::*;
#[test]
fn real_nonempty_patch_multigeneration_and_output_selection_preserve_owned_parent_data() {
    let mut events = vec![];
    let sink = |event| {
        events.push(event);
        Ok(())
    };
    let mut f =
        WorkerDatasetFacade::new(InMemoryWorkerDatasetStore::default(), sink, SystemTime::now);
    let parent = f
        .create_dataset(
            "root".into(),
            BTreeSet::from(["parent".into()]),
            json!({"items":[{"id":"a","cost":3}]}),
            json!({"limit":2}),
            false,
        )
        .unwrap();
    let patch=JsonPatch::parse(r#"[{"op":"replace","path":"/items/[id=a]/cost","value":9},{"op":"add","path":"/items/-","value":{"id":"b","cost":1}}]"#,JsonPatchMode::Model).unwrap();
    let child = f
        .patch_dataset(
            &parent.id,
            WorkerDatasetSelector::Input,
            "child".into(),
            Some(json!({"limit":7})),
            &patch,
            true,
        )
        .unwrap();
    let childdata = f.get_dataset(&child.id).unwrap();
    assert_eq!(
        childdata.input,
        json!({"items":[{"id":"a","cost":9},{"id":"b","cost":1}]})
    );
    assert_eq!(childdata.configuration, json!({"limit":7}));
    assert_eq!(childdata.metadata.tags, parent.tags);
    assert_eq!(
        f.get_dataset(&parent.id).unwrap().input,
        json!({"items":[{"id":"a","cost":3}]})
    );
    let mut independent = childdata.input;
    independent["items"][0]["cost"] = json!(99);
    assert_eq!(
        f.get_dataset(&child.id).unwrap().input["items"][0]["cost"],
        json!(9)
    );
    f.store_mut()
        .set_output(&child.id, json!({"items":[{"id":"solved","cost":0}]}))
        .unwrap();
    let grandchild = f
        .create_from_dataset(
            &child.id,
            WorkerDatasetSelector::Output,
            "grandchild".into(),
            BTreeSet::new(),
            json!({}),
            false,
        )
        .unwrap();
    assert_eq!(grandchild.parent_id, Some(child.id));
    assert_eq!(grandchild.origin_id, parent.id);
    assert_eq!(
        f.get_dataset(&grandchild.id).unwrap().input,
        json!({"items":[{"id":"solved","cost":0}]})
    );
}
#[test]
fn invalid_patch_unknown_selection_and_sink_errors_have_concrete_publication_boundaries() {
    let clock_calls = Cell::new(0);
    let sink_calls = Cell::new(0);
    let mut f = WorkerDatasetFacade::new(
        InMemoryWorkerDatasetStore::default(),
        |_| {
            sink_calls.set(sink_calls.get() + 1);
            Ok(())
        },
        || {
            clock_calls.set(clock_calls.get() + 1);
            SystemTime::UNIX_EPOCH
        },
    );
    let parent = f
        .create_dataset(
            "root".into(),
            BTreeSet::new(),
            json!({"x":1}),
            json!({}),
            false,
        )
        .unwrap();
    let patch = JsonPatch::parse(
        r#"[{"op":"replace","path":"/missing","value":2}]"#,
        JsonPatchMode::Strict,
    )
    .unwrap();
    assert!(matches!(
        f.patch_dataset(
            &parent.id,
            WorkerDatasetSelector::Input,
            "bad".into(),
            None,
            &patch,
            false
        ),
        Err(WorkerFacadeError::Patch(_))
    ));
    assert!(matches!(
        f.create_from_dataset(
            &parent.id,
            WorkerDatasetSelector::Output,
            "bad".into(),
            BTreeSet::new(),
            json!({}),
            false
        ),
        Err(WorkerFacadeError::MissingOutput(_))
    ));
    assert_eq!(f.store().len(), 1);
    assert_eq!(clock_calls.get(), 1);
    assert_eq!(sink_calls.get(), 2);
    assert!(f.solve_dataset("unknown").is_err());
    assert_eq!(sink_calls.get(), 2);
    let mut f = WorkerDatasetFacade::new(
        InMemoryWorkerDatasetStore::default(),
        |event| {
            if matches!(event, WorkerDatasetEvent::ValidateCompute { .. }) {
                Err("delivery failed".into())
            } else {
                Ok(())
            }
        },
        SystemTime::now,
    );
    let error = f
        .create_dataset(
            "persisted".into(),
            BTreeSet::new(),
            json!({"x":1}),
            json!({}),
            true,
        )
        .unwrap_err();
    let WorkerFacadeError::Event { dataset, kind, .. } = error else {
        panic!("sink error")
    };
    assert_eq!(kind, WorkerDatasetEventKind::ValidateCompute);
    assert_eq!(f.store().len(), 1);
    assert_eq!(f.get_dataset(&dataset).unwrap().input, json!({"x":1}));
}
#[test]
fn emitted_start_command_can_drive_actual_native_solver_using_stored_inputs() {
    let mut queue = vec![];
    let mut f = WorkerDatasetFacade::new(
        InMemoryWorkerDatasetStore::default(),
        |event| {
            queue.push(event);
            Ok(())
        },
        SystemTime::now,
    );
    let metadata = f
        .create_dataset(
            "solve".into(),
            BTreeSet::new(),
            json!({"domain":[1,2,3],"entities":2}),
            json!({"maxEvaluations":100}),
            false,
        )
        .unwrap();
    f.solve_dataset(&metadata.id).unwrap();
    let stored = f.get_dataset(&metadata.id).unwrap();
    drop(f);
    assert_eq!(
        queue.last(),
        Some(&WorkerDatasetEvent::SolveStart {
            id: metadata.id.clone()
        })
    );
    let domain = stored.input["domain"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_i64().unwrap())
        .collect::<Vec<_>>();
    let count = stored.input["entities"].as_u64().unwrap() as usize;
    let problem = FiniteProblem::new(vec![Variable::new(domain); count]).unwrap();
    let mut network = ConstraintNetwork::new(count);
    network
        .add_constraint("sum", (0..count).collect(), |values| {
            SimpleScore::new(values.iter().flatten().map(|&&v| v).sum())
        })
        .unwrap();
    let result = solve(
        &problem,
        &network,
        SolverConfig {
            limits: SolverLimits {
                max_evaluations: stored.configuration["maxEvaluations"].as_u64().unwrap(),
                ..Default::default()
            },
            local_search: vec![],
            ..Default::default()
        },
    )
    .unwrap();
    assert!(result.complete);
    assert_eq!(result.score, Some(SimpleScore::new(6)));
    assert_eq!(result.assignment.values(), &[Some(3), Some(3)]);
}
