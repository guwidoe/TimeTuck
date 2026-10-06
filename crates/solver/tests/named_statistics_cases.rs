// Adapted from pinned Community ProblemSizeStatisticsTest and test fixtures.
// Apache-2.0; explicit native entity/field bindings replace reflection.
// See compat/provenance.json for test and fixture attribution.
use timetuck::*;
fn metadata(
    name: &str,
    field: &str,
    n: usize,
    values: usize,
    list: bool,
) -> Vec<StatisticsEntityType> {
    let slots = (0..n).collect();
    let variable = if list {
        StatisticsVariable::list(field, slots)
    } else {
        StatisticsVariable::basic(field, slots).with_empty_value_count(values)
    };
    vec![StatisticsEntityType {
        name: name.into(),
        entity_count: n,
        variables: vec![variable],
    }]
}
fn check(s: &NamedProblemSize, name: &str, field: &str, entities: usize, values: usize) {
    assert_eq!(s.entity_count, entities);
    assert_eq!(s.variable_count, entities);
    assert_eq!(s.approximate_value_count, values);
    assert_eq!(s.entities_by_type.len(), 1);
    assert_eq!(s.entities_by_type[name], entities);
    assert_eq!(s.values_by_type_variable[name][field], values);
}
fn basic(entities: usize, values: usize, initialized: bool, name: &str) -> NamedProblemSize {
    let p = FiniteProblem::new(
        (0..entities)
            .map(|i| {
                let v = Variable::new((0..values).collect());
                if initialized {
                    v.with_initial(i % values)
                } else {
                    v
                }
            })
            .collect(),
    )
    .unwrap();
    NamedProblemSize::from_basic(&p, &metadata(name, "value", entities, values, false)).unwrap()
}
#[test]
fn simple_solution_entity_and_variable_count() {
    check(
        &basic(7, 5, true, "TestdataEntity"),
        "TestdataEntity",
        "value",
        7,
        5,
    );
}
#[test]
fn empty_solution_entity_and_variable_count() {
    check(
        &basic(0, 3, true, "TestdataEntity"),
        "TestdataEntity",
        "value",
        0,
        3,
    );
}
#[test]
fn uninitialized_solution_entity_and_variable_count() {
    check(
        &basic(10, 6, false, "TestdataEntity"),
        "TestdataEntity",
        "value",
        10,
        6,
    );
}
#[test]
fn single_entity_and_value() {
    check(
        &basic(1, 1, true, "TestdataEntity"),
        "TestdataEntity",
        "value",
        1,
        1,
    );
}
#[test]
fn composite_value_ranges_entity_and_variable_count() {
    let first: Vec<_> = (0..4).collect();
    let second: Vec<_> = (4..8).collect();
    let domain: Vec<_> = first.into_iter().chain(second).collect();
    let p = FiniteProblem::new(vec![Variable::new(domain); 6]).unwrap();
    let s = NamedProblemSize::from_basic(
        &p,
        &metadata("TestdataCompositeEntity", "value", 6, 8, false),
    )
    .unwrap();
    check(&s, "TestdataCompositeEntity", "value", 6, 8);
}
#[test]
fn multiple_entity_types_entity_and_variable_count() {
    #[derive(Clone, Eq, PartialEq)]
    enum Value {
        Fact(usize),
        Text(String),
    }
    let first: Vec<_> = (0..5).map(Value::Fact).collect();
    let second: Vec<_> = (0..5)
        .map(|i| Value::Text(format!("Generated Value {i}")))
        .collect();
    let mut variables = vec![];
    for i in (0..8).step_by(2) {
        variables.push(Variable::new(first.clone()).with_initial(first[i % 5].clone()));
    }
    for i in (1..8).step_by(2) {
        variables.push(Variable::new(second.clone()).with_initial(second[i / 5].clone()));
    }
    let p = FiniteProblem::new(variables).unwrap();
    let schema = vec![
        StatisticsEntityType {
            name: "First".into(),
            entity_count: 4,
            variables: vec![StatisticsVariable::basic("value", (0..4).collect())],
        },
        StatisticsEntityType {
            name: "Second".into(),
            entity_count: 4,
            variables: vec![StatisticsVariable::basic("value", (4..8).collect())],
        },
    ];
    let s = NamedProblemSize::from_basic(&p, &schema).unwrap();
    assert_eq!(s.entity_count, 8);
    assert_eq!(s.variable_count, 8);
    assert_eq!(s.approximate_value_count, 10);
    for name in ["First", "Second"] {
        assert_eq!(s.entities_by_type[name], 4);
        assert_eq!(s.values_by_type_variable[name]["value"], 5);
    }
}
#[test]
fn list_variable_entity_and_variable_count() {
    let p = ListProblem::new(
        (0..4).map(|i| format!("Generated Value {i}")).collect(),
        vec![ListEntity::new(vec![]); 6],
    )
    .unwrap();
    let s = NamedProblemSize::from_list(
        &p,
        &metadata("TestdataEqualsByCodeListEntity", "valueList", 6, 4, true),
    )
    .unwrap();
    check(&s, "TestdataEqualsByCodeListEntity", "valueList", 6, 4);
}
#[test]
fn cascading_list_variable_entity_and_variable_count() {
    let p = ListProblem::new(
        (1..=3).collect::<Vec<_>>(),
        vec![ListEntity::new(vec![]); 5],
    )
    .unwrap();
    let s = NamedProblemSize::from_list(
        &p,
        &metadata("TestdataSingleCascadingEntity", "valueList", 5, 3, true),
    )
    .unwrap();
    check(&s, "TestdataSingleCascadingEntity", "valueList", 5, 3);
}
#[test]
fn record_entity_and_variable_count() {
    #[derive(Clone, Eq, PartialEq)]
    struct RecordValue {
        code: String,
    }
    let values: Vec<_> = (0..4)
        .map(|i| RecordValue {
            code: format!("Generated Value {i}"),
        })
        .collect();
    let p = FiniteProblem::new(
        (0..9)
            .map(|i| Variable::new(values.clone()).with_initial(values[i % 4].clone()))
            .collect(),
    )
    .unwrap();
    let s =
        NamedProblemSize::from_basic(&p, &metadata("TestdataRecordEntity", "value", 9, 4, false))
            .unwrap();
    check(&s, "TestdataRecordEntity", "value", 9, 4);
}
#[test]
fn format_approximate_problem_scale() {
    for (count, expected) in [
        (100u64, "100"),
        (250, "250"),
        (1_234_567, "1,234,567"),
        (123_456_789, "123,456,789"),
        (1_123_456_789, "1,123,456,789"),
        (321_123_456_789, "3.211235 × 10^11"),
    ] {
        assert_eq!(
            ProblemScale::from_log10((count as f64).log10()).formatted(),
            expected
        );
    }
    for log in [f64::NEG_INFINITY, f64::INFINITY, f64::NAN] {
        assert_eq!(ProblemScale::from_log10(log).formatted(), "0");
    }
    assert_eq!(ProblemScale::from_log10(0.0).formatted(), "1");
}
