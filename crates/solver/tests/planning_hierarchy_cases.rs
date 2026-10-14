// Full operational native adaptations; explicit composition replaces JVM scans.
// Apache-2.0 attribution is recorded in compat/provenance.json.
#[path = "fixtures/planning_hierarchy_fixture.rs"]
mod fixture;
use fixture::solve;
use fixture::*;
use timetuck::*;
fn class_case(extra: Option<&'static str>, reward_extra: bool) {
    let mut input = solution(
        3,
        2,
        if extra == Some("subValue") {
            "Subvalue"
        } else {
            "Value2"
        },
    );
    if extra == Some("subValue") {
        input.values = generated(3, "value")
    }
    let width = if extra.is_some() { 2 } else { 1 };
    let scope = (0..2)
        .flat_map(|i| {
            if reward_extra {
                vec![i * width, i * width + 1]
            } else {
                vec![i * width]
            }
        })
        .collect();
    let output = solve(input, move || solution_binding(extra), scope, 1);
    assert_eq!(
        output.score,
        Some(SimpleScore::new(if reward_extra { 4 } else { 2 }))
    );
    assert_eq!(
        output.score.unwrap().score(),
        class_score(&output, reward_extra)
    );
    assert!(
        output
            .entities
            .iter()
            .all(|e| e.base.value.is_some() && (extra.is_none() || e.extra.is_some()))
    );
}
fn interface_case(values: usize, extra: bool) {
    let width = if extra { 2 } else { 1 };
    let scope = (0..2 * width).collect();
    let output = solve(
        interface_solution(values, 2),
        move || interface_solution_binding(extra),
        scope,
        1,
    );
    assert_eq!(
        output.score,
        Some(SimpleScore::new(if extra { 4 } else { 2 }))
    );
    assert_eq!(
        output.score.unwrap().score(),
        interface_score(&output, extra)
    );
    assert!(
        output
            .entities
            .iter()
            .all(|e| e.value().is_some() && (!extra || e.extra.is_some()))
    );
}
#[test]
fn entity_both_classes_annotated() {
    class_case(Some("subValue"), false)
}
#[test]
fn entity_both_classes_annotated_base_interface() {
    interface_case(3, false)
}
#[test]
fn entity_only_base_class_annotated() {
    class_case(None, false)
}
#[test]
fn entity_only_base_class_annotated_base_interface() {
    interface_case(1, false)
}
#[test]
fn entity_add_new_variable() {
    class_case(Some("value2"), true)
}
#[test]
fn entity_add_new_variable_base_interface() {
    interface_case(3, true)
}
#[test]
fn entity_replace_variable() {
    let error = match child_binding(None).member(
        "value",
        "valueRange",
        |e| e.extra.clone(),
        |e, v| e.extra = v,
    ) {
        Err(e) => e,
        Ok(_) => panic!("duplicate accepted"),
    };
    assert!(
        error
            .to_string()
            .contains("redefines the genuine variables ([value]), which is not permitted.")
    );
}
#[test]
fn entity_replace_variable_base_interface() {
    let error = match interface_binding(false).member(
        "value",
        "valueRange",
        |e| e.value(),
        |e, v| e.set_value(v),
    ) {
        Err(e) => e,
        Ok(_) => panic!("duplicate accepted"),
    };
    assert!(
        error
            .to_string()
            .contains("redefines the genuine variables ([value]), which is not permitted.")
    );
}
fn extended(collections: usize, _extra: Option<&str>) -> ExtendedSolution {
    let base = solution(3, 2, "Subvalue");
    let rows = base.entities.clone();
    let mut result = ExtendedSolution {
        base,
        second: if collections > 1 {
            rows.clone()
        } else {
            vec![]
        },
        objects: if collections > 2 {
            rows.clone()
        } else {
            vec![]
        },
        raw: if collections > 2 { rows } else { vec![] },
    };
    for (group, rows) in [
        &mut result.base.entities,
        &mut result.second,
        &mut result.objects,
        &mut result.raw,
    ]
    .into_iter()
    .enumerate()
    {
        for (i, e) in rows.iter_mut().enumerate() {
            e.base.id = i * collections + group;
        }
    }
    result
}
fn assert_extended(s: &ExtendedSolution, multiplier: i64, expected: i64) {
    let count = s
        .base
        .entities
        .iter()
        .chain(&s.second)
        .chain(&s.objects)
        .chain(&s.raw)
        .filter(|e| e.base.value.is_some())
        .count() as i64;
    assert_eq!(s.base.score, Some(SimpleScore::new(expected)));
    assert_eq!(count * multiplier, expected);
}
#[test]
fn solution_both_classes_annotated_base_abstract() {
    let s = solve(
        extended(2, None),
        || extended_binding(2, None),
        (0..4).collect(),
        2,
    );
    assert_extended(&s, 2, 8);
}
#[test]
fn solution_only_base_class_annotated() {
    let s = solve(
        extended(1, None),
        || extended_binding(1, None),
        (0..2).collect(),
        1,
    );
    assert_extended(&s, 1, 2);
}
#[test]
fn solution_both_classes_annotated_configured_xml() {
    let xml = SolverXmlConfiguration::parse(include_str!(
        "fixtures/planning_hierarchy/bothClassAnnotatedConfig.xml"
    ))
    .unwrap();
    let signature = XmlFactorySignature::from_configuration(&xml).unwrap();
    let mut registry = XmlSolverFactoryRegistry::default();
    registry
        .register(signature, |c| {
            let goal: SimpleScore = c
                .root()
                .child("termination")
                .unwrap()
                .value("bestScoreLimit")
                .unwrap()
                .parse()
                .unwrap();
            build_factory(
                || extended_binding(4, Some("subValue")),
                (0..16).step_by(2).collect(),
                2,
                goal,
            )
        })
        .unwrap();
    let factory = registry.prepare(&xml).unwrap();
    let mut manager = factory.build_manager(1).unwrap();
    let job = manager
        .job_builder(1)
        .with_problem(extended(4, Some("subValue")))
        .run()
        .unwrap();
    let result = job.wait();
    let s = result.as_ref().as_ref().unwrap();
    assert_extended(s, 2, 16);
    assert!(
        s.base
            .entities
            .iter()
            .chain(&s.second)
            .chain(&s.objects)
            .chain(&s.raw)
            .all(|e| e.extra.is_some())
    );
    manager.shutdown().unwrap();
}
