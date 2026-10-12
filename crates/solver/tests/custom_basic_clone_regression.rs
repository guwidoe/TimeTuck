// Original native projection, isolation and metadata checks; shared TEST fixtures
// are attributed in compat/provenance.json.
#[path = "fixtures/custom_basic_clone_fixture.rs"]
mod fixture;
use fixture::*;
use std::{cell::Cell, sync::Arc};
use timetuck::*;
fn config() -> SolverConfig<'static, SimpleScore> {
    SolverConfig {
        local_search: vec![LocalSearchConfig {
            max_steps: 2,
            max_evaluations: 32,
            ..Default::default()
        }],
        ..Default::default()
    }
}
#[test]
fn solved_objects_rebind_to_actual_best_values_and_isolate_the_input() {
    let source = source(Policy::Correct);
    let accessors = accessors();
    let mut n = ConstraintNetwork::<Arc<Value>, SimpleScore>::new(1);
    n.add_constraint("prefer2", vec![0], |row| {
        SimpleScore::new(i64::from(row[0].as_ref().is_some_and(|v| v.0 == "2")))
    })
    .unwrap();
    let result = cloner(Policy::Correct)
        .solve(&accessors, &source, &n, config())
        .unwrap();
    assert_eq!(result.native.score, Some(SimpleScore::new(1)));
    assert_eq!(result.solution.entity.value.as_ref().unwrap().0, "2");
    assert!(Arc::ptr_eq(
        result.solution.entity.value.as_ref().unwrap(),
        &source.values[1]
    ));
    assert!(!Arc::ptr_eq(&source.entity, &result.solution.entity));
    assert!(std::ptr::eq(
        result.binding.entities()[0].object_ref::<Entity>().unwrap(),
        result.solution.entity.as_ref()
    ));
    assert_eq!(
        result.binding.problem().initial_assignment(),
        result.native.assignment
    );
    assert!(source.entity.value.is_none());
    assert_eq!(source.score, None);
    for (old, new) in source.values.iter().zip(&result.solution.values) {
        assert!(Arc::ptr_eq(old, new));
    }
}
#[test]
fn dropped_assignment_and_changed_provider_are_rejected_without_publishing() {
    for change_range in [false, true] {
        let source = source(Policy::Correct);
        let a = accessors();
        let n = ConstraintNetwork::new(1);
        let mut policy = BasicSolutionCloner::new(
            move |s: &Domain, assignment: &Assignment<Arc<Value>>, score| {
                let mut values = s.values.clone();
                if change_range {
                    values.push(Arc::new(Value("extra")));
                }
                Domain {
                    entity: Arc::new(Entity {
                        code: s.entity.code,
                        value: if change_range {
                            assignment.values()[0].clone()
                        } else {
                            None
                        },
                    }),
                    values,
                    score: Some(score),
                    cloned: true,
                }
            },
            |s| s.score,
        );
        let error = policy.solve(&a, &source, &n, config()).err().unwrap();
        assert!(matches!(
            (&error, change_range),
            (BasicSolutionCloneError::AssignmentChanged, false)
                | (BasicSolutionCloneError::EntityLayoutChanged, true)
        ));
        assert!(source.entity.value.is_none());
        assert_eq!(source.values.len(), 2);
        assert_eq!(source.score, None);
    }
}
#[test]
fn wrong_score_metadata_rejects_before_projection_and_panics_leave_input_reusable() {
    let source = source(Policy::Correct);
    let a = accessors();
    let calls = Cell::new(0);
    let mut wrong = BasicSolutionCloner::new(
        |_: &Domain, _: &Assignment<Arc<Value>>, _: HardSoftScore| {
            calls.set(calls.get() + 1);
            panic!("must reject before cloning")
        },
        |_| Some(HardSoftScore::ZERO),
    );
    let n = ConstraintNetwork::new(1);
    assert!(matches!(
        wrong.solve(&a, &source, &n, SolverConfig::default()),
        Err(BasicSolutionCloneError::ScoreTypeMismatch)
    ));
    assert_eq!(calls.get(), 0);
    let n = ConstraintNetwork::new(1);
    let mut policy = BasicSolutionCloner::new(
        |s: &Domain, assignment: &Assignment<Arc<Value>>, score| {
            calls.set(calls.get() + 1);
            if calls.get() == 1 {
                panic!("projection failure");
            }
            Domain {
                entity: Arc::new(Entity {
                    code: s.entity.code,
                    value: assignment.values()[0].clone(),
                }),
                values: s.values.clone(),
                score: Some(score),
                cloned: true,
            }
        },
        |s| s.score,
    );
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| policy.solve(
            &a,
            &source,
            &n,
            config()
        )))
        .is_err()
    );
    assert!(source.entity.value.is_none());
    assert_eq!(source.score, None);
    assert!(
        policy
            .solve(&a, &source, &n, config())
            .unwrap()
            .solution
            .cloned
    );
    assert_eq!(calls.get(), 3);
}
#[test]
fn collection_aliases_survive_cloning_and_reordered_backlinks_are_rejected() {
    struct Aliases {
        first: Vec<Arc<Entity>>,
        second: Vec<Arc<Entity>>,
        values: Vec<Arc<Value>>,
        score: Option<SimpleScore>,
    }
    let values = vec![Arc::new(Value("1")), Arc::new(Value("2"))];
    let a = Arc::new(Entity {
        code: "A",
        value: None,
    });
    let b = Arc::new(Entity {
        code: "B",
        value: None,
    });
    let source = Aliases {
        first: vec![a.clone(), b.clone(), a.clone()],
        second: vec![b, a],
        values,
        score: None,
    };
    let schema = PlanningComposition::new(vec![PlanningComponent::registered::<Entity>(vec![
        PlanningVariableDefinition::basic::<Arc<Value>>("value", false),
    ])])
    .unwrap();
    let layout = Arc::new(
        SolutionComposition::new(vec![PlanningSolutionComponent::registered::<Aliases>(
            vec![
                SolutionMemberDefinition::entity_collection::<Entity>("first"),
                SolutionMemberDefinition::entity_collection::<Entity>("second"),
                SolutionMemberDefinition::value_range::<Arc<Value>>("values", "range"),
                SolutionMemberDefinition::score::<SimpleScore>("score"),
            ],
        )])
        .unwrap()
        .compile::<Aliases>(
            &schema,
            &[PlanningType::of::<Entity>()],
            SolutionCompositionPolicy::default(),
            PlanningCompositionPolicy::default(),
        )
        .unwrap(),
    );
    let mut accessors = SolutionBasicAccessors::new(
        layout,
        vec![SolutionFieldRange::new::<Entity>(
            "value",
            vec!["range".into()],
        )],
    )
    .unwrap();
    accessors
        .entities("first", |s: &Aliases| {
            s.first
                .iter()
                .map(|e| (e.clone(), vec![("value".into(), e.value.clone())]))
                .collect()
        })
        .unwrap();
    accessors
        .entities("second", |s: &Aliases| {
            s.second
                .iter()
                .map(|e| (e.clone(), vec![("value".into(), e.value.clone())]))
                .collect()
        })
        .unwrap();
    accessors.range("values", |s| s.values.clone()).unwrap();
    for corrupt in [false, true] {
        let mut policy = BasicSolutionCloner::new(
            move |s: &Aliases, assignment: &Assignment<Arc<Value>>, score| {
                let a = Arc::new(Entity {
                    code: s.first[0].code,
                    value: assignment.values()[0].clone(),
                });
                let b = Arc::new(Entity {
                    code: s.first[1].code,
                    value: assignment.values()[1].clone(),
                });
                Aliases {
                    first: vec![a.clone(), b.clone(), a.clone()],
                    second: if corrupt { vec![a, b] } else { vec![b, a] },
                    values: s.values.clone(),
                    score: Some(score),
                }
            },
            |s| s.score,
        );
        let result = policy.solve(&accessors, &source, &ConstraintNetwork::new(2), config());
        if corrupt {
            assert!(matches!(
                result,
                Err(BasicSolutionCloneError::EntityLayoutChanged)
            ));
        } else {
            let result = result.unwrap();
            assert_eq!(result.binding.entities().len(), 2);
            assert!(Arc::ptr_eq(
                &result.solution.first[0],
                &result.solution.first[2]
            ));
            assert!(Arc::ptr_eq(
                &result.solution.first[1],
                &result.solution.second[0]
            ));
            assert!(!Arc::ptr_eq(&result.solution.first[0], &source.first[0]));
        }
    }
    assert!(source.first.iter().all(|e| e.value.is_none()));
}
