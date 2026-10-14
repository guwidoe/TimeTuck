// Adapted Community TEST-only Quarkus/Spring supplier solve methods and fixtures.
// Copyright Timefold BV and contributors. Apache-2.0; see THIRD_PARTY_NOTICES.md.
// Native factory/manager replaces JVM injection; no framework bootstrap parity.
use std::{sync::Arc, time::Duration};
use timetuck::*;

#[derive(Clone, Debug)]
struct Entity {
    value1: Option<String>,
    value2: Option<String>,
    concatenated: Option<String>,
}
#[derive(Debug)]
struct Solution {
    values: Vec<String>,
    entities: Vec<Arc<Entity>>,
    score: Option<SimpleScore>,
}
fn schema() -> PlanningSchema {
    PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Entity>(vec![
        PlanningVariableDefinition::basic::<String>("value1", false),
        PlanningVariableDefinition::basic::<String>("value2", false),
        PlanningVariableDefinition::shadow::<String>("concatenated"),
    ])])
    .unwrap()
}
type SupplierBuilder<'a> = ShadowSessionBuilder<'a, 'a, usize, Vec<Option<String>>>;
fn supplier_builder(schema: &PlanningSchema) -> (SupplierBuilder<'_>, ShadowNode<String>) {
    let fields = schema.entity::<Entity>().unwrap();
    let mut builder = ShadowSessionBuilder::<usize, Vec<Option<String>>>::new(schema);
    let a = builder
        .input(fields.variable("value1").unwrap(), 0, |w| w[0].clone())
        .unwrap();
    let b = builder
        .input(fields.variable("value2").unwrap(), 0, |w| w[1].clone())
        .unwrap();
    let left = a.clone();
    let right = b.clone();
    let concatenated = builder
        .derive(
            fields.variable("concatenated").unwrap(),
            0,
            move |_, inputs| {
                Some(format!(
                    "{}{}",
                    inputs.get(&left).unwrap()?,
                    inputs.get(&right).unwrap()?
                ))
            },
        )
        .unwrap();
    builder
        .set_dependencies(&concatenated, &[a.erase(), b.erase()])
        .unwrap();
    (builder, concatenated)
}
fn supplier(schema: &PlanningSchema, world: &[Option<String>]) -> Entity {
    let (builder, concatenated) = supplier_builder(schema);
    let mut session = builder.build();
    session.update(&world.to_vec()).unwrap();
    Entity {
        value1: world[0].clone(),
        value2: world[1].clone(),
        concatenated: session.read(&concatenated).unwrap().value.cloned(),
    }
}
fn filter(left: &Entity, right: &Entity) -> bool {
    // Source filter deliberately fails when a joined entity has no supplier result.
    left.concatenated
        .as_ref()
        .expect("left supplier must precede filter")
        == right
            .concatenated
            .as_ref()
            .expect("right supplier must precede filter")
}
fn objective(entity: &Entity) -> SimpleScore {
    // forEach excludes the entity until BOTH genuine variables are initialized.
    if entity.value1.is_none() || entity.value2.is_none() {
        return SimpleScore::new(0);
    }
    // The source joins the one entity to itself using value1 == value2.
    SimpleScore::new(-i64::from(
        entity.value1 == entity.value2 && filter(entity, entity),
    ))
}
fn stream(
    schema: &PlanningSchema,
) -> Result<(StreamGraph<'_, SimpleScore>, PlanningStreamSource<String>), GraphError> {
    let mut graph = StreamGraphBuilder::new();
    let source = graph.source::<usize, (usize, Option<String>)>();
    let rows = graph.collect_list(source.port(), |row| row.clone())?;
    let world = graph.map(&rows, |rows| {
        let mut out = vec![None; 2];
        for (slot, value) in rows.iter() {
            out[*slot] = value.clone();
        }
        out
    })?;
    // Native graph owns the ShadowSession and refreshes both source fields before
    // publishing derived state to complete-entity selection and equality join.
    let (builder, concatenated) = supplier_builder(schema);
    let entities = graph.map_shadows(&world, builder, move |world, session| {
        Ok(Entity {
            value1: world[0].clone(),
            value2: world[1].clone(),
            concatenated: session.read(&concatenated)?.value.cloned(),
        })
    })?;
    let complete = graph.filter(&entities, |e| e.value1.is_some() && e.value2.is_some())?;
    let pairs = graph.join_equal(
        &complete,
        &complete,
        |e| e.value1.clone(),
        |e| e.value2.clone(),
    )?;
    let matches = graph.filter(&pairs, |(left, right)| filter(left, right))?;
    graph.score("Don't assign 2 entities the same value.", &matches, |_| {
        SimpleScore::new(-1)
    })?;
    Ok((graph.build(), source))
}
fn run(stream_backend: bool, exact_zero: bool) {
    let entity = Arc::new(Entity {
        value1: None,
        value2: None,
        concatenated: None,
    });
    let problem = Arc::new(Solution {
        values: vec!["a".into(), "b".into()],
        entities: vec![entity.clone()],
        score: None,
    });
    let config = SolverConfig {
        limits: SolverLimits {
            max_evaluations: 64,
            best_score_limit: Some(SimpleScore::new(0)),
            ..Default::default()
        },
        local_search: vec![LocalSearchConfig {
            max_steps: 8,
            max_evaluations: 32,
            ..Default::default()
        }],
        ..Default::default()
    };
    let factory = SolverFactory::new(config, move |source: Arc<Solution>, config, progress| {
        let schema = schema();
        let problem = FiniteProblem::new(vec![Variable::new(source.values.clone()); 2]).unwrap();
        let result = if stream_backend {
            solve_stream(&problem, || stream(&schema), config)
        } else {
            let mut network = ConstraintNetwork::new(2);
            network
                .add_constraint(
                    "Don't assign 2 entities the same value.",
                    vec![0, 1],
                    |values| {
                        objective(&supplier(
                            &schema,
                            &values.iter().map(|v| v.cloned()).collect::<Vec<_>>(),
                        ))
                    },
                )
                .unwrap();
            solve(&problem, &network, config)
        }
        .map_err(|e| e.to_string())?;
        assert!(result.complete);
        assert!(result.work.evaluations <= 64);
        let score = result.score.ok_or_else(|| "missing score".to_string())?;
        let entity = supplier(&schema, result.assignment.values());
        assert_eq!(score, objective(&entity));
        progress.record_work(result.work);
        // A real native owned solution clone projects the retained assignment;
        // source Arc allocations are retained by the caller for identity checks.
        Ok::<_, String>(Arc::new(Solution {
            values: source.values.clone(),
            entities: vec![Arc::new(entity)],
            score: Some(score),
        }))
    })
    .unwrap();
    let mut manager = factory.build_manager(1).unwrap();
    let job = manager
        .job_builder(1)
        .with_problem(problem.clone())
        .run()
        .unwrap();
    let completed = job.wait_timeout(Duration::from_secs(10)).unwrap();
    let solution = completed.as_ref().as_ref().unwrap();
    assert!(!Arc::ptr_eq(solution, &problem));
    assert!(!Arc::ptr_eq(&solution.entities[0], &entity));
    let score = solution.score.expect("source non-null solution and score");
    if exact_zero {
        assert_eq!(score, SimpleScore::new(0));
    } else {
        assert!(score >= SimpleScore::new(0));
    }
    assert_eq!(
        solution.entities[0].concatenated,
        Some(format!(
            "{}{}",
            solution.entities[0].value1.as_ref().unwrap(),
            solution.entities[0].value2.as_ref().unwrap()
        ))
    );
    assert!(entity.value1.is_none() && entity.value2.is_none() && entity.concatenated.is_none());
    assert!(problem.score.is_none());
    manager.shutdown().unwrap();
}
#[test]
fn quarkus_supplier_shadow_solution_simple_solve() {
    for backend in [false, true] {
        run(backend, true);
    }
}
#[test]
fn spring_single_solver_solve_supplier_variables() {
    for backend in [false, true] {
        run(backend, false);
    }
}
#[test]
fn supplier_objective_preserves_partial_exclusion_self_join_and_null_failure() {
    let schema = schema();
    for values in [
        vec![None, None],
        vec![Some("a".into()), None],
        vec![None, Some("b".into())],
    ] {
        let entity = supplier(&schema, &values);
        assert!(entity.concatenated.is_none());
        assert_eq!(objective(&entity), SimpleScore::new(0));
    }
    for a in ["a", "b"] {
        for b in ["a", "b"] {
            let entity = supplier(&schema, &[Some(a.into()), Some(b.into())]);
            assert_eq!(objective(&entity), SimpleScore::new(-i64::from(a == b)));
        }
    }
    let broken = Entity {
        value1: Some("a".into()),
        value2: Some("a".into()),
        concatenated: None,
    };
    assert!(std::panic::catch_unwind(|| objective(&broken)).is_err());
}
