// Adapted from Community VariableSupportTest.shadowVariableListGraphEvents.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;
struct Solution;
struct Visit;
const IDS: [&str; 6] = ["a1", "a2", "b1", "b2", "b3", "c"];
struct World {
    rows: Vec<Vec<&'static str>>,
    previous: [Option<usize>; 6],
    owners: [usize; 6],
}
fn edge_ids(
    session: &ShadowSession<'_, '_, &'static str, World>,
    edges: &[ShadowDependencyEdge],
) -> Vec<(String, &'static str, String, &'static str)> {
    edges
        .iter()
        .map(|e| {
            (
                session.variable(&e.source).unwrap().name().into(),
                *session.entity(&e.source).unwrap(),
                session.variable(&e.target).unwrap().name().into(),
                *session.entity(&e.target).unwrap(),
            )
        })
        .collect()
}
#[test]
fn shadow_variable_list_graph_events() {
    let schema =
        PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Visit>(vec![
            PlanningVariableDefinition::shadow::<i64>("serviceReadyTime"),
            PlanningVariableDefinition::shadow::<i64>("serviceStartTime"),
            PlanningVariableDefinition::shadow::<i64>("serviceFinishTime"),
        ])])
        .unwrap();
    let entity = schema.entity::<Visit>().unwrap();
    let ready_field = entity.variable("serviceReadyTime").unwrap();
    let start_field = entity.variable("serviceStartTime").unwrap();
    let finish_field = entity.variable("serviceFinishTime").unwrap();
    let mut world = World {
        rows: vec![vec!["a1", "b1", "c"], vec!["a2", "b2"], vec!["b3"]],
        previous: [None, None, Some(0), Some(1), None, Some(2)],
        owners: [0, 1, 0, 1, 2, 0],
    };
    let mut builder = ShadowSessionBuilder::new(&schema);
    let mut ready = vec![];
    for (i, id) in IDS.iter().enumerate() {
        ready.push(
            builder
                .derive(ready_field, *id, move |w: &World, inputs| {
                    let _owner = w.owners[i];
                    let previous = inputs.for_field::<i64>(finish_field).unwrap();
                    let time = match previous.first() {
                        Some(Some(v)) => **v + 30,
                        Some(None) => return None,
                        None => 0,
                    };
                    // C combines its local stages into one native node without extra edges.
                    Some(time + if i == 5 { 30 } else { 0 })
                })
                .unwrap(),
        );
    }
    let mut start = vec![];
    for (rep, group) in [(0, 0..2), (2, 2..5)] {
        let sources: Vec<_> = group.map(|i| ready[i].erase()).collect();
        let node = builder
            .derive(start_field, IDS[rep], move |_: &World, inputs| {
                inputs
                    .for_field::<i64>(ready_field)
                    .unwrap()
                    .into_iter()
                    .flatten()
                    .copied()
                    .max()
            })
            .unwrap();
        builder.set_dependencies(&node, &sources).unwrap();
        start.push(node);
    }
    let mut finish = vec![];
    for (i, id) in IDS[..5].iter().enumerate() {
        let group_start = start[usize::from(i >= 2)].clone();
        let source = group_start.erase();
        let node = builder
            .derive(finish_field, *id, move |_: &World, inputs| {
                inputs.get(&group_start).unwrap().map(|v| v + 30)
            })
            .unwrap();
        builder.set_dependencies(&node, &[source]).unwrap();
        finish.push(node);
    }
    for (i, node) in ready.iter().enumerate() {
        let ends = finish.clone();
        builder
            .bind_dependencies(node, move |w: &World| {
                Ok(w.previous[i]
                    .map(|previous| vec![ends[previous].erase()])
                    .unwrap_or_default())
            })
            .unwrap();
    }
    let mut session = builder.build();
    let initial = session
        .update_notified_with_dependency_events(&world)
        .unwrap();
    assert!(initial.removed.is_empty());
    let actual = edge_ids(&session, &initial.added);
    let mut expected = vec![];
    for (rep, group) in [("a1", 0..2), ("b1", 2..5)] {
        for i in group {
            expected.push((
                "serviceReadyTime".to_owned(),
                IDS[i],
                "serviceStartTime".to_owned(),
                rep,
            ));
            expected.push((
                "serviceStartTime".to_owned(),
                rep,
                "serviceFinishTime".to_owned(),
                IDS[i],
            ));
        }
    }
    for (i, previous) in world.previous.iter().enumerate() {
        if let Some(previous) = previous {
            expected.push((
                "serviceFinishTime".to_owned(),
                IDS[*previous],
                "serviceReadyTime".to_owned(),
                IDS[i],
            ));
        }
    }
    for edge in &expected {
        assert_eq!(actual.iter().filter(|e| *e == edge).count(), 1);
    }
    assert!(actual.len() <= expected.len());
    assert_eq!(session.dependency_edges().len(), actual.len());
    // Fact notifications identify affected dynamic targets; owner repeats detach once.
    let removed = session
        .before_dependencies_changed(&[ready[2].erase(), ready[5].erase()])
        .unwrap();
    let mut actual = edge_ids(&session, &removed);
    actual.sort();
    assert_eq!(
        actual,
        vec![
            (
                "serviceFinishTime".into(),
                "a1",
                "serviceReadyTime".into(),
                "b1"
            ),
            (
                "serviceFinishTime".into(),
                "b1",
                "serviceReadyTime".into(),
                "c"
            ),
        ]
    );
    assert!(
        session
            .before_dependencies_changed(&[ready[2].erase(), ready[5].erase()])
            .unwrap()
            .is_empty()
    );
    let detached = session.dependency_edges();
    world.rows[0] = vec!["a1", "c"];
    assert_eq!(session.dependency_edges(), detached);
    // Field mutations do not run resolvers or add edges before flush.
    world.previous[5] = Some(0);
    world.owners[5] = 0;
    world.previous[2] = None;
    assert_eq!(session.dependency_edges(), detached);
    assert_eq!(
        session.assert_current(),
        Err(ShadowSessionError::NotCurrent)
    );
    let update = session
        .update_notified_with_dependency_events(&world)
        .unwrap();
    session.assert_current().unwrap();
    assert!(update.removed.is_empty());
    assert_eq!(
        edge_ids(&session, &update.added),
        vec![(
            "serviceFinishTime".into(),
            "a1",
            "serviceReadyTime".into(),
            "c"
        ),]
    );
}
