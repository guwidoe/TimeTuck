//! Original real array-range solving, identity and operational key-group checks.
#[allow(dead_code)]
#[path = "fixtures/planning_model_access_fixture.rs"]
mod fixture;
use fixture::*;
use std::{
    any::Any,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use timetuck::*;
#[test]
fn fixed_array_range_configures_and_runs_real_list_solver() {
    let schema = list_schema();
    let access = ListVariableAccess::<ListValue>::new(
        schema
            .entity::<ListEntityType>()
            .unwrap()
            .genuine_variable("valueList")
            .unwrap(),
    )
    .unwrap();
    let range = access.array_range(|s: &ArraySolution| &s.values);
    let solution = ArraySolution {
        values: [ListValue(1), ListValue(2), ListValue(3)],
    };
    let ranges = range.read(&solution, 2).unwrap();
    let problem = ranges
        .list_problem(vec![ListEntity::new(vec![]); 2])
        .unwrap();
    let mut network = ListConstraintNetwork::new(2, 3);
    network
        .add_constraint(
            "assigned",
            vec![ListScope::Entity(0), ListScope::Entity(1)],
            |facts| {
                SimpleScore::new(
                    facts
                        .iter()
                        .map(|f| match f {
                            ListFact::Entity(row) => row.len() as i64,
                            _ => 0,
                        })
                        .sum(),
                )
            },
        )
        .unwrap();
    let result = solve_list(&problem, &network, SolverConfig::default()).unwrap();
    assert!(result.complete);
    assert_eq!(result.score, Some(SimpleScore::new(3)));
    for value in &solution.values {
        assert_eq!(
            result
                .assignment
                .lists()
                .iter()
                .flatten()
                .filter(|v| *v == value)
                .count(),
            1
        );
    }
    assert_eq!(solution.values, [ListValue(1), ListValue(2), ListValue(3)]);
}
#[test]
fn list_access_rejects_wrong_element_types_and_basic_or_shadow_fields() {
    let schema = list_schema();
    let variable = schema
        .entity::<ListEntityType>()
        .unwrap()
        .genuine_variable("valueList")
        .unwrap();
    assert!(ListVariableAccess::<Vec<ListValue>>::new(variable).is_err());
    let schema = PlanningSchema::new::<ArraySolution>(vec![PlanningEntityDefinition::of::<
        ListEntityType,
    >(vec![
        PlanningVariableDefinition::basic::<ListValue>("basic", false),
        PlanningVariableDefinition::shadow::<ListValue>("shadow"),
    ])])
    .unwrap();
    for field in ["basic", "shadow"] {
        assert!(
            ListVariableAccess::<ListValue>::new(
                schema
                    .entity::<ListEntityType>()
                    .unwrap()
                    .variable(field)
                    .unwrap()
            )
            .is_err()
        );
    }
}
#[test]
fn extractor_preserves_equal_distinct_objects_deduplicates_aliases_and_observes_changes() {
    let mut solution = extended_solution();
    solution.entities = vec![BaseEntity::new("equal"), BaseEntity::new("equal")];
    let mut access = extractor();
    access
        .member("alias", |s: &ExtendedSolution| {
            s.entities.iter().map(|e| e as &dyn Any).collect()
        })
        .unwrap();
    assert!(access.member("alias", |_| vec![]).is_err());
    let entities = access.extract(&solution).unwrap();
    assert_eq!(entities.len(), 11);
    let equal: Vec<_> = entities.into_iter().filter(|e| e.code == "equal").collect();
    assert_eq!(equal.len(), 2);
    assert!(!std::ptr::eq(equal[0], equal[1]));
    drop(equal);
    solution.entities.pop();
    solution.objects.push(Box::new(42_i32));
    assert_eq!(access.extract(&solution).unwrap().len(), 10);
    solution.entity = None;
    solution.sub_entity = None;
    assert_eq!(access.extract(&solution).unwrap().len(), 8);
}
#[test]
fn alignment_members_read_actual_keys_group_in_order_and_outlive_registry() {
    struct Solution;
    struct Entity {
        fact: i32,
    }
    let member = {
        let schema =
            PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Entity>(vec![])])
                .unwrap();
        let mut registry = AlignmentKeyRegistry::new(&schema);
        registry.register("fact", |e: &Entity| e.fact).unwrap();
        resolve_alignment_key::<Entity, i32>(
            Some(&registry),
            Some("supplier"),
            "shadow",
            Some("fact"),
        )
        .unwrap()
        .unwrap()
    };
    let mut entities = [Entity { fact: 2 }, Entity { fact: 1 }, Entity { fact: 2 }];
    let groups = member.groups(&entities);
    assert_eq!(groups.keys().copied().collect::<Vec<_>>(), vec![1, 2]);
    assert!(std::ptr::eq(groups[&1][0], &entities[1]));
    assert!(std::ptr::eq(groups[&2][0], &entities[0]));
    assert!(std::ptr::eq(groups[&2][1], &entities[2]));
    drop(groups);
    entities[0].fact = 1;
    assert_eq!(member.groups(&entities)[&1].len(), 2);
}
#[test]
fn alignment_rejects_all_variable_kinds_and_bad_bindings_before_readers() {
    struct Solution;
    struct Entity {
        value: i32,
    }
    let schema =
        PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Entity>(vec![
            PlanningVariableDefinition::basic::<i32>("basic", false),
            PlanningVariableDefinition::list::<i32>("list", false),
            PlanningVariableDefinition::shadow::<i32>("shadow"),
        ])])
        .unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let mut registry = AlignmentKeyRegistry::new(&schema);
    for name in ["basic", "list", "shadow", "fact"] {
        let reads = calls.clone();
        registry
            .register(name, move |e: &Entity| {
                reads.fetch_add(1, Ordering::Relaxed);
                e.value
            })
            .unwrap();
    }
    assert!(registry.register("fact", |e: &Entity| e.value).is_err());
    for name in ["basic", "list", "shadow", "missing"] {
        assert!(
            resolve_alignment_key::<Entity, i32>(
                Some(&registry),
                Some("supplier"),
                "shadow",
                Some(name)
            )
            .is_err()
        );
    }
    assert!(
        resolve_alignment_key::<Entity, String>(
            Some(&registry),
            Some("supplier"),
            "shadow",
            Some("fact")
        )
        .is_err()
    );
    assert!(resolve_alignment_key::<Entity, i32>(None, None, "shadow", Some("fact")).is_err());
    assert!(
        resolve_alignment_key::<Entity, i32>(None, None, "shadow", None)
            .unwrap()
            .is_none()
    );
    assert_eq!(calls.load(Ordering::Relaxed), 0);
    let member =
        resolve_alignment_key::<Entity, i32>(Some(&registry), None, "shadow", Some("fact"))
            .unwrap()
            .unwrap();
    assert_eq!(member.key(&Entity { value: 7 }), 7);
    assert_eq!(calls.load(Ordering::Relaxed), 1);
}
