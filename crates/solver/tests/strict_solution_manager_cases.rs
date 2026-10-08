// Adapted from Timefold Community SolutionManagerTest inconsistent-update methods
// and testdomain/shadow/no_inconsistent_field fixtures. Apache-2.0.
// Copyright Timefold BV and contributors. See compat/provenance.json.
use std::{cell::Cell, collections::BTreeSet};
use timetuck::*;
struct Solution;
struct Value;
fn inconsistent_case() {
    let schema =
        PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Value>(vec![
            PlanningVariableDefinition::shadow::<usize>("entity"),
            PlanningVariableDefinition::shadow::<i64>("startTime"),
            PlanningVariableDefinition::shadow::<i64>("endTime"),
        ])])
        .unwrap();
    let p = ListProblem::new(vec!["a1", "a2"], vec![ListEntity::new(vec!["a2", "a1"])]).unwrap();
    let entity = schema.entity::<Value>().unwrap();
    let owner_field = entity.variable("entity").unwrap();
    let start_field = entity.variable("startTime").unwrap();
    let end_field = entity.variable("endTime").unwrap();
    let mut builder = ShadowSessionBuilder::<&str, ListAssignment<&str>>::new(&schema);
    let mut owners = vec![];
    let mut starts = vec![];
    let mut ends = vec![];
    let ids = ["a1", "a2"];
    for id in ids {
        owners.push(
            builder
                .input(owner_field, id, move |a| {
                    a.element(&id).map(|p| p.position.entity)
                })
                .unwrap(),
        );
        starts.push(
            builder
                .derive(start_field, id, move |_, i| {
                    i.for_field::<usize>(owner_field)
                        .unwrap()
                        .first()
                        .copied()
                        .flatten()?;
                    let dependencies = i
                        .for_field::<i64>(end_field)
                        .unwrap()
                        .into_iter()
                        .collect::<Option<Vec<_>>>()?;
                    Some(dependencies.into_iter().copied().max().unwrap_or(0))
                })
                .unwrap(),
        );
    }
    for (index, id) in ids.into_iter().enumerate() {
        let start = starts[index].clone();
        ends.push(
            builder
                .derive(end_field, id, move |_, i| {
                    i.get(&start).unwrap().map(|v| v + 60)
                })
                .unwrap(),
        );
    }
    for (index, id) in ids.into_iter().enumerate() {
        let owner = owners[index].erase();
        let source_ends = ends.clone();
        builder
            .bind_dependencies(&starts[index], move |a| {
                let mut sources = vec![owner.clone()];
                if let Some(previous) = a.element(&id).and_then(|p| p.previous) {
                    sources.push(
                        source_ends[ids.iter().position(|id| id == previous).unwrap()].erase(),
                    );
                }
                if id == "a2" {
                    sources.push(source_ends[0].erase());
                }
                Ok(sources)
            })
            .unwrap();
        builder
            .set_dependencies(&ends[index], &[starts[index].erase()])
            .unwrap();
    }
    let score_calls = Cell::new(0);
    let mut manager = list_declarative_solution_manager(
        &p,
        builder.build(),
        move |_, s| {
            Ok::<_, &str>(
                ends.iter()
                    .map(|node| s.read(node).unwrap().value.copied())
                    .collect::<Vec<_>>(),
            )
        },
        |_, _| Ok(()),
        |_, h| {
            score_calls.set(score_calls.get() + 1);
            Ok(HardSoftScore::new(
                0,
                -h.derived.iter().flatten().sum::<i64>(),
            ))
        },
    );
    let mut solution = ManagedSolution::new(
        p.initial_assignment(),
        ListDerivedShadows {
            builtin: vec![ListBuiltinValues::default(); 2],
            derived: vec![None; 2],
        },
    );
    let original = &solution as *const _;
    let before = solution.clone();
    let error = manager.update(&mut solution).unwrap_err();
    assert!(error.to_string().contains("requires a consistent solution"));
    let ShadowSolutionError::Shadows(ConsistentShadowUpdateError::Inconsistent(report)) = error
    else {
        panic!("not an inconsistent-solution error")
    };
    // The native API leaves ownership with the caller instead of embedding its
    // solution reference in a Java exception. Check that exact object and data.
    assert!(std::ptr::eq(original, &solution));
    assert_eq!(solution, before);
    assert_eq!(report.loops.len(), 1);
    let actual = report.loops[0]
        .iter()
        .map(|key| {
            assert_eq!(key.entity_type, PlanningType::of::<Value>());
            (key.entity, key.variable.as_str())
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        actual,
        BTreeSet::from([
            ("a1", "startTime"),
            ("a1", "endTime"),
            ("a2", "startTime"),
            ("a2", "endTime")
        ])
    );
    assert_eq!(score_calls.get(), 0);
}
#[test]
fn update_inconsistent() {
    inconsistent_case();
}
#[test]
fn update_only_shadow_variables_inconsistent() {
    // The source method uses ordinary update despite its method name.
    inconsistent_case();
}
