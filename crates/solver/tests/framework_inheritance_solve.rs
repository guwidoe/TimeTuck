// Adapted pinned Timefold Community Quarkus inheritance TEST/helpers/resources.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
// Native owned composition and registered XML keys replace JVM scanning/CDI.
use timetuck::*;

#[derive(Clone, Debug, Default)]
struct BaseEntity {
    code: String,
    value: Option<String>,
}
#[derive(Clone, Debug, Default)]
struct ChildEntity {
    base: BaseEntity,
    sub_value: Option<String>,
}
#[derive(Clone, Debug, Default)]
struct BaseSolution {
    values: Vec<String>,
    sub_values: Vec<String>,
    entities: Vec<ChildEntity>,
    score: Option<SimpleScore>,
}
// Exactly the source's three concrete collections; no raw collection exists.
#[derive(Clone, Debug, Default)]
struct Solution {
    base: BaseSolution,
    entity: Option<BaseEntity>,
    sub_entity: Option<ChildEntity>,
    sub_entities: Vec<ChildEntity>,
    objects: Vec<ChildEntity>,
}
fn generate_solution(value_count: usize, entity_count: usize) -> Solution {
    let mut solution = Solution {
        base: BaseSolution {
            values: (0..value_count)
                .map(|id| format!("Generated Value {id}"))
                .collect(),
            sub_values: (0..value_count)
                .map(|id| format!("Generated Subvalue {id}"))
                .collect(),
            ..Default::default()
        },
        ..Default::default()
    };
    for id in 0..entity_count {
        for (offset, entities) in [
            &mut solution.base.entities,
            &mut solution.sub_entities,
            &mut solution.objects,
        ]
        .into_iter()
        .enumerate()
        {
            entities.push(ChildEntity {
                base: BaseEntity {
                    code: (id * 3 + offset).to_string(),
                    value: None,
                },
                sub_value: None,
            });
        }
    }
    solution
}
fn base_entity_binding() -> PlanningEntityBinding<BaseEntity, String> {
    PlanningEntityBinding::default()
        .member(
            "value",
            "valueRange",
            |entity: &BaseEntity| entity.value.clone(),
            |entity, value| entity.value = value,
        )
        .unwrap()
}
fn child_entity_binding() -> PlanningEntityBinding<ChildEntity, String> {
    PlanningEntityBinding::default()
        .inherit(
            base_entity_binding(),
            |entity: &ChildEntity| &entity.base,
            |entity| &mut entity.base,
        )
        .unwrap()
        .member(
            "subValue",
            "subValueRange",
            |entity| entity.sub_value.clone(),
            |entity, value| entity.sub_value = value,
        )
        .unwrap()
}
fn range(solution: &BaseSolution, name: &str) -> Option<Vec<String>> {
    match name {
        "valueRange" => Some(solution.values.clone()),
        "subValueRange" => Some(solution.sub_values.clone()),
        _ => None,
    }
}
fn binding() -> PlanningSolutionBinding<Solution, String> {
    let parent = PlanningSolutionBinding::default().collection(
        child_entity_binding(),
        |solution: &BaseSolution| &solution.entities,
        |solution| &mut solution.entities,
        range,
    );
    PlanningSolutionBinding::default()
        .inherit(
            parent,
            |solution: &Solution| &solution.base,
            |solution| &mut solution.base,
        )
        .collection(
            base_entity_binding(),
            |solution| solution.entity.as_slice(),
            |solution| solution.entity.as_mut_slice(),
            |solution, name| range(&solution.base, name),
        )
        .collection(
            child_entity_binding(),
            |solution| solution.sub_entity.as_slice(),
            |solution| solution.sub_entity.as_mut_slice(),
            |solution, name| range(&solution.base, name),
        )
        .collection(
            child_entity_binding(),
            |solution| &solution.sub_entities,
            |solution| &mut solution.sub_entities,
            |solution, name| range(&solution.base, name),
        )
        .collection(
            child_entity_binding(),
            |solution| &solution.objects,
            |solution| &mut solution.objects,
            |solution, name| range(&solution.base, name),
        )
}
// The two TEST providers reward the inherited value independently. forEach
// excludes a child until *both* its genuine fields are assigned; subValue is
// required for eligibility but does not itself contribute a reward.
fn eligible(values: &[Option<&String>]) -> i64 {
    values
        .chunks_exact(2)
        .filter(|entity| entity.iter().all(Option::is_some))
        .count() as i64
}
type ScoringGraph = (
    StreamGraph<'static, SimpleScore>,
    StreamSource<usize, (usize, Option<String>)>,
);
fn graph() -> Result<ScoringGraph, GraphError> {
    let mut builder = StreamGraphBuilder::new();
    let source = builder.source::<usize, (usize, Option<String>)>();
    let rows = builder.collect_list(source.port(), |row| row.clone())?;
    let counts = builder.map(&rows, |rows| {
        let mut values = vec![None; 12];
        for (id, value) in rows.iter() {
            values[*id] = value.as_ref();
        }
        eligible(&values)
    })?;
    builder.score("Constraint", &counts, |&count| SimpleScore::new(count))?;
    builder.score("Constraint2", &counts, |&count| SimpleScore::new(count))?;
    Ok((builder.build(), source))
}
fn factory(stream: bool) -> Result<SolverFactory<Solution, Solution, String>, SolverFactoryError> {
    let config = SolverConfig {
        limits: SolverLimits {
            max_evaluations: 1000,
            best_score_limit: Some(SimpleScore::ZERO),
            assert_phase_scores: true,
            ..Default::default()
        },
        construction: ConstructionConfig {
            max_steps: Some(12),
            assert_scores: true,
            ..Default::default()
        },
        local_search: vec![LocalSearchConfig {
            max_steps: 32,
            max_evaluations: 100,
            move_order: MoveOrder::Original,
            assert_scores: true,
            ..Default::default()
        }],
    };
    SolverFactory::new(config, move |mut input: Solution, config, progress| {
        let layout = binding();
        let model = layout.open(&input).map_err(|error| error.to_string())?;
        assert_eq!(model.variables().len(), 12);
        let result = if stream {
            solve_stream(&model, graph, config).map_err(|error| error.to_string())?
        } else {
            let mut network = ConstraintNetwork::new(12);
            for name in ["Constraint", "Constraint2"] {
                network
                    .add_constraint(name, (0..12).collect(), |values| {
                        SimpleScore::new(eligible(values))
                    })
                    .map_err(|error| error.to_string())?;
            }
            solve(&model, &network, config).map_err(|error| error.to_string())?
        };
        assert!(result.complete);
        assert!(result.solution().is_some());
        assert!(result.work.evaluations <= 1000);
        layout
            .commit(&mut input, &result.assignment)
            .map_err(|error| error.to_string())?;
        input.base.score = result.score;
        progress.record_work(result.work);
        Ok::<_, String>(input)
    })
}
#[test]
fn both_classes_annotated_configured_xml_no_raw_list() {
    let xml = SolverXmlConfiguration::parse(include_str!(
        "fixtures/framework_inheritance_no_raw_config.xml"
    ))
    .unwrap();
    let signature = XmlFactorySignature::from_configuration(&xml).unwrap();
    for stream in [false, true] {
        let mut registry = XmlSolverFactoryRegistry::default();
        registry
            .register(signature.clone(), move |_| factory(stream))
            .unwrap();
        let factory = registry.prepare(&xml).unwrap();
        let mut manager = factory.build_manager(1).unwrap();
        let problem = generate_solution(3, 2);
        assert_eq!(
            [
                problem.base.entities.len(),
                problem.sub_entities.len(),
                problem.objects.len()
            ],
            [2; 3]
        );
        assert_eq!(
            [problem.base.values.len(), problem.base.sub_values.len()],
            [3; 2]
        );
        assert!(problem.entity.is_none() && problem.sub_entity.is_none());
        let job = manager.job_builder(1).with_problem(problem).run().unwrap();
        let result = job.wait();
        let solution = result.as_ref().as_ref().unwrap(); // Source nonnull result.
        assert_eq!(solution.base.score, Some(SimpleScore::new(12)));
        let collections = [
            &solution.base.entities,
            &solution.sub_entities,
            &solution.objects,
        ];
        let mut codes = Vec::new();
        let mut count = 0;
        for (group, entities) in collections.into_iter().enumerate() {
            assert_eq!(entities.len(), 2);
            for (index, entity) in entities.iter().enumerate() {
                assert_eq!(entity.base.code, (index * 3 + group).to_string());
                codes.push(entity.base.code.clone());
                assert!(
                    solution
                        .base
                        .values
                        .contains(entity.base.value.as_ref().unwrap())
                );
                assert!(
                    solution
                        .base
                        .sub_values
                        .contains(entity.sub_value.as_ref().unwrap())
                );
                // Independent rescore from the concrete objects committed by
                // actual inherited setters, rather than the search score.
                count += i64::from(entity.base.value.is_some() && entity.sub_value.is_some());
            }
        }
        codes.sort();
        assert_eq!(codes, (0..6).map(|id| id.to_string()).collect::<Vec<_>>());
        assert_eq!(solution.base.score, Some(SimpleScore::new(count + count)));
        assert!(solution.entity.is_none() && solution.sub_entity.is_none());
        manager.shutdown().unwrap();
    }
}
#[test]
fn both_fields_control_child_eligibility_but_only_base_value_is_rewarded() {
    let values = [
        "Generated Value 0".to_string(),
        "Generated Subvalue 0".to_string(),
    ];
    assert_eq!(eligible(&[Some(&values[0]), None]), 0);
    assert_eq!(eligible(&[None, Some(&values[1])]), 0);
    assert_eq!(eligible(&[Some(&values[0]), Some(&values[1])]), 1);
    let problem = binding().open(&generate_solution(3, 2)).unwrap();
    let (graph, source) = graph().unwrap();
    let mut director =
        StreamScoreDirector::new(&problem, graph, source, problem.initial_assignment()).unwrap();
    let first = director
        .apply(&Move::Change {
            variable: 0,
            value: Some(values[0].clone()),
        })
        .unwrap();
    assert_eq!(first.score().with_structural(0), SimpleScore::ZERO);
    first.commit();
    let second = director
        .apply(&Move::Change {
            variable: 1,
            value: Some(values[1].clone()),
        })
        .unwrap();
    assert_eq!(second.score().with_structural(0), SimpleScore::new(2));
    second.commit();
    director.assert_consistent().unwrap();
}
