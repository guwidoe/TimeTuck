// Adapted from pinned Timefold Community DefaultSolverTest and test fixtures.
// Apache-2.0; provenance and native substitutions: compat/provenance.json.
use std::collections::BTreeSet;
use timetuck::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct BasicValue {
    id: usize,
    strength: usize,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ListValue(usize);

#[test]
fn solve_multi_entity_mixed_model() {
    for (entities, values, others) in [(2, 2, 2), (2, 3, 2), (2, 2, 3), (3, 2, 2)] {
        let original = (0..others)
            .map(|id| BasicValue {
                id,
                strength: others * 10 - id,
            })
            .collect::<Vec<_>>();
        let mut ascending = original.clone();
        ascending.sort_by_key(|v| v.strength);
        // First basic uses the fixture's ascending strength comparator. The second
        // retains encounter order. These are distinct entity types from list owners.
        let basics = (0..entities)
            .flat_map(|_| {
                [
                    Variable::new(ascending.clone()),
                    Variable::new(original.clone()),
                ]
            })
            .collect();
        let p = MixedProblem::new(
            FiniteProblem::new(basics).unwrap(),
            ListProblem::new(
                (0..values).map(ListValue).collect(),
                vec![ListEntity::new(vec![]); entities],
            )
            .unwrap(),
        );
        let mut n = MixedConstraintNetwork::<BasicValue, ListValue, SimpleScore>::new(
            2 * entities,
            entities,
            values,
        );
        for entity in 0..entities {
            n.add_constraint(
                format!("basic entity {entity}"),
                vec![
                    MixedScope::Basic(2 * entity),
                    MixedScope::Basic(2 * entity + 1),
                ],
                |f| {
                    let MixedFact::Basic(a) = f[0] else {
                        panic!("basic")
                    };
                    let MixedFact::Basic(b) = f[1] else {
                        panic!("basic")
                    };
                    SimpleScore::new(i64::from(a.is_some()) + i64::from(b.is_some()))
                },
            )
            .unwrap();
            n.add_constraint(
                format!("list entity {entity}"),
                vec![MixedScope::ListEntity(entity)],
                |f| {
                    let MixedFact::ListEntity(list) = f[0] else {
                        panic!("list")
                    };
                    SimpleScore::new(if list.len() == 1 { 2 } else { 1 })
                },
            )
            .unwrap();
        }
        let mut events = 0;
        let result = solve_mixed_with_observer(
            &p,
            &n,
            MixedSolverConfig {
                limits: SolverLimits {
                    max_move_evaluations: Some(40),
                    ..Default::default()
                },
                ..Default::default()
            },
            |_| events += 1,
        )
        .unwrap();
        assert!(events > 0);
        assert!(result.complete);
        assert!(result.solution().is_some());
        assert!(result.work.move_evaluations <= 40);
        assert_eq!(
            result
                .assignment
                .lists()
                .lists()
                .iter()
                .filter(|l| l.is_empty())
                .count(),
            entities.saturating_sub(values)
        );
        for pair in result.assignment.basic().values().chunks_exact(2) {
            assert!(pair[0].is_some() && pair[1].is_some());
            assert_ne!(pair[0], pair[1]);
        }
        // Full original test objective, independent of the constraint callbacks.
        let expected = 2 * entities
            + result
                .assignment
                .lists()
                .lists()
                .iter()
                .map(|l| if l.len() == 1 { 2 } else { 1 })
                .sum::<usize>();
        assert_eq!(result.score, Some(SimpleScore::new(expected as i64)));
    }
}

#[test]
fn solve_mixed_model_with_multiple_providers() {
    // Solution-level basic range and three explicit entity-level list ranges.
    let lists = ListProblem::from_entity_ranges(
        (0..3)
            .map(|_| ListEntity::new(vec![]).with_value_range((0..3).map(ListValue).collect()))
            .collect(),
    )
    .unwrap();
    assert_eq!(lists.values(), &[ListValue(0), ListValue(1), ListValue(2)]);
    let p = MixedProblem::new(
        FiniteProblem::new(vec![
            Variable::new(
                (0..3).map(|id| BasicValue { id, strength: 0 }).collect()
            );
            3
        ])
        .unwrap(),
        lists,
    );
    let n = MixedConstraintNetwork::<_, _, SimpleScore>::new(3, 3, 3);
    let mut events = 0;
    let result = solve_mixed_with_observer(
        &p,
        &n,
        MixedSolverConfig {
            local_search: vec![MixedLocalSearchConfig {
                search: LocalSearchConfig {
                    max_steps: 16,
                    ..Default::default()
                },
                ..Default::default()
            }],
            ..Default::default()
        },
        |_| events += 1,
    )
    .unwrap();
    assert!(events > 0);
    assert!(result.solution().is_some());
    p.validate(&result.assignment).unwrap();
    assert!(
        result
            .assignment
            .basic()
            .values()
            .iter()
            .all(Option::is_some)
    );
    let mut values = result
        .assignment
        .lists()
        .lists()
        .iter()
        .flatten()
        .map(|v| v.0)
        .collect::<Vec<_>>();
    values.sort();
    assert_eq!(values, [0, 1, 2]);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LeadOrValue {
    Value(usize),
    Lead(usize),
}
#[test]
fn solve_multi_entity() {
    let mut variables = vec![Variable::new(vec![LeadOrValue::Value(0), LeadOrValue::Value(1)]); 3];
    variables.extend(vec![
        Variable::new((0..3).map(LeadOrValue::Lead).collect());
        3
    ]);
    let p = FiniteProblem::new(variables).unwrap();
    let n = ConstraintNetwork::<_, SimpleScore>::new(6);
    let mut events = 0;
    let result = solve_with_observer(
        &p,
        &n,
        SolverConfig {
            limits: SolverLimits {
                max_move_evaluations: Some(40),
                ..Default::default()
            },
            ..Default::default()
        },
        |_| events += 1,
    )
    .unwrap();
    assert!(events > 0);
    assert!(result.solution().is_some());
    assert!(result.work.move_evaluations <= 40);
    assert!(
        result.assignment.values()[..3]
            .iter()
            .all(|v| matches!(v, Some(LeadOrValue::Value(_))))
    );
    assert!(
        result.assignment.values()[3..]
            .iter()
            .all(|v| matches!(v, Some(LeadOrValue::Lead(_))))
    );
    assert!(
        p.with_initial_assignment(&Assignment::new(vec![Some(LeadOrValue::Lead(0)); 6]))
            .is_err()
    );
}

#[test]
fn solve_list_var_entity_range_model_with_tree_set() {
    let entities = [vec!["v1", "v2"], vec!["v1", "v3"]]
        .into_iter()
        .map(|range| {
            let sorted = range.into_iter().collect::<BTreeSet<_>>();
            ListEntity::new(vec![]).with_value_range(sorted.into_iter().collect())
        })
        .collect();
    let p = ListProblem::from_entity_ranges(entities)
        .unwrap()
        .allowing_unassigned();
    assert_eq!(p.values(), &["v1", "v2", "v3"]);
    let n = ListConstraintNetwork::<_, SimpleScore>::new(2, 3);
    let mut events = 0;
    let result = solve_list_with_observer(
        &p,
        &n,
        SolverConfig {
            construction: ConstructionConfig {
                optional: OptionalConstruction::NonWorsening,
                ..Default::default()
            },
            limits: SolverLimits {
                max_move_evaluations: Some(10),
                ..Default::default()
            },
            ..Default::default()
        },
        |_| events += 1,
    )
    .unwrap();
    assert!(events > 0);
    assert!(result.solution().is_some());
    assert!(result.work.move_evaluations <= 10);
    p.validate(&result.assignment).unwrap();
    assert!(!result.assignment.lists()[0].contains(&"v3"));
    assert!(!result.assignment.lists()[1].contains(&"v2"));
}
