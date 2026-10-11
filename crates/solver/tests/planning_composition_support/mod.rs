// Test-only inheritance declarations and generators adapted from the pinned
// Community test domains; Apache-2.0. Full path attribution in provenance.
use timetuck::*;
pub struct Base<const N: usize>;
pub struct Middle<const N: usize>;
pub struct Child<const N: usize>;
pub struct Projected<const N: usize>;
pub struct Solution<const N: usize>;
// The original regression target deliberately reuses a subset of these fixtures.
#[allow(dead_code)]
#[derive(Clone, Copy)]
pub enum Shape {
    BothOwned,
    BothProjected,
    BaseOwned,
    BaseProjected,
    OnlyChildOwned,
    OnlyChildProjected,
    AddOwned,
    AddProjected,
    ReplaceOwned,
    ReplaceProjected,
    DeepBothOwned,
    DeepBothProjected,
    DeepBaseOwned,
    DeepBaseProjected,
    Mixed,
}
pub struct Fixture {
    pub fields: Vec<&'static str>,
    pub ranges: Vec<Vec<String>>,
    pub rows: Vec<Vec<Option<String>>>,
    pub scored: Vec<&'static str>,
    pub composition: PlanningComposition,
    pub actual_class_fixture: bool,
}
fn field(name: &str) -> PlanningVariableDefinition {
    PlanningVariableDefinition::basic::<String>(name, false)
}
fn component<T: 'static>(registered: bool, names: &[&str]) -> PlanningComponent {
    let vars = names.iter().map(|name| field(name)).collect();
    if registered {
        PlanningComponent::registered::<T>(vars)
    } else {
        PlanningComponent::plain::<T>(vars)
    }
}
pub fn generate<const N: usize>(shape: Shape) -> Fixture {
    use Shape::*;
    let projected = matches!(
        shape,
        BothProjected
            | BaseProjected
            | OnlyChildProjected
            | AddProjected
            | ReplaceProjected
            | DeepBothProjected
            | DeepBaseProjected
    );
    let kind = if projected {
        PlanningParentKind::Projection
    } else {
        PlanningParentKind::Owned
    };
    let base_registered = !matches!(shape, OnlyChildOwned | OnlyChildProjected);
    let child_registered = !matches!(
        shape,
        BaseOwned | BaseProjected | DeepBaseOwned | DeepBaseProjected
    );
    let mut base_names = vec!["value"];
    let mut child_names = vec![];
    let mut middle_names = vec![];
    match shape {
        BothOwned => child_names.push("subValue"),
        AddOwned | AddProjected => child_names.push("value2"),
        ReplaceOwned | ReplaceProjected => child_names.push("value"),
        DeepBothOwned => child_names.push("value2"),
        DeepBothProjected => middle_names.push("value2"),
        Mixed => {
            base_names = vec!["value2"];
            child_names.push("value3");
        }
        _ => {}
    }
    let base = component::<Base<N>>(base_registered, &base_names);
    let mut child = component::<Child<N>>(child_registered, &child_names);
    let mut declarations = vec![base];
    if matches!(
        shape,
        DeepBothOwned | DeepBothProjected | DeepBaseOwned | DeepBaseProjected
    ) {
        let middle = component::<Middle<N>>(
            matches!(shape, DeepBothOwned | DeepBothProjected),
            &middle_names,
        )
        .with_parent::<Base<N>>(kind);
        child = child.with_parent::<Middle<N>>(kind);
        declarations.push(middle);
    } else {
        child = child.with_parent::<Base<N>>(kind);
    }
    if matches!(shape, Mixed) {
        declarations.push(component::<Projected<N>>(true, &["value"]));
        child = child.with_parent::<Projected<N>>(PlanningParentKind::Projection);
    }
    declarations.push(child);
    let fields = match shape {
        BothOwned => vec!["value", "subValue"],
        AddOwned | AddProjected | DeepBothOwned | DeepBothProjected => vec!["value", "value2"],
        Mixed => vec!["value", "value2", "value3"],
        _ => vec!["value"],
    };
    let ranges = fields
        .iter()
        .map(|name| {
            (0..3)
                .map(|i| match (shape, *name) {
                    (BothOwned, "value") => format!("Generated value {i}"),
                    (BothOwned, "subValue") => format!("Generated Subvalue {i}"),
                    (_, "value") => format!("Generated Value {i}"),
                    (_, "value2") => format!("Generated Value2 {i}"),
                    (_, "value3") => format!("Generated Value3 {i}"),
                    _ => unreachable!(),
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let initialized = matches!(
        shape,
        OnlyChildOwned | OnlyChildProjected | DeepBaseOwned | DeepBaseProjected
    );
    let rows = (0..2)
        .map(|i| {
            ranges
                .iter()
                .map(|range| initialized.then(|| range[i % range.len()].clone()))
                .collect()
        })
        .collect();
    let scored = if matches!(
        shape,
        AddOwned | AddProjected | DeepBothOwned | DeepBothProjected
    ) {
        vec!["value", "value2"]
    } else if matches!(shape, Mixed) {
        vec!["value", "value2", "value3"]
    } else {
        vec!["value"]
    };
    Fixture {
        fields,
        ranges,
        rows,
        scored,
        composition: PlanningComposition::new(declarations).unwrap(),
        actual_class_fixture: matches!(shape, OnlyChildProjected),
    }
}
pub fn compile<const N: usize>(
    fixture: &Fixture,
    policy: PlanningCompositionPolicy,
) -> Result<ComposedPlanningSchema, PlanningCompositionError> {
    fixture
        .composition
        .compile::<Solution<N>>(&[PlanningType::of::<Child<N>>()], policy)
}
pub fn problem<const N: usize>(
    fixture: &Fixture,
    composed: &ComposedPlanningSchema,
) -> FiniteProblem<String> {
    let entity = composed.schema().genuine_entity::<Child<N>>().unwrap();
    let mut vars = vec![];
    for (i, &name) in fixture.fields.iter().enumerate() {
        let ranges = entity
            .genuine_variable(name)
            .unwrap()
            .shared_ranges(vec![fixture.ranges[i].clone()], fixture.rows.len())
            .unwrap();
        for (row, values) in fixture.rows.iter().enumerate() {
            let mut variable = ranges.variable(row).unwrap();
            if let Some(value) = &values[i] {
                variable = variable.with_initial(value.clone());
            }
            vars.push(variable);
        }
    }
    FiniteProblem::new(vars).unwrap()
}
pub fn scoring_slots(fixture: &Fixture) -> Vec<usize> {
    fixture
        .scored
        .iter()
        .flat_map(|name| {
            let field = fixture.fields.iter().position(|n| n == name).unwrap();
            (0..fixture.rows.len()).map(move |row| field * fixture.rows.len() + row)
        })
        .collect()
}
pub fn solve_fixture<const N: usize>(
    fixture: &Fixture,
    composed: &ComposedPlanningSchema,
    move_limit: u64,
) -> Vec<SolverResult<Assignment<String>, SimpleScore>> {
    let p = problem::<N>(fixture, composed);
    let network = dense_network(fixture);
    let plan = SolverSequence {
        limits: SolverLimits {
            max_evaluations: 1000,
            max_move_evaluations: Some(move_limit),
            assert_phase_scores: true,
            ..Default::default()
        },
        phases: vec![
            SolverPhase::Construction(ConstructionConfig::default()),
            SolverPhase::LocalSearch(LocalSearchConfig {
                max_steps: 10,
                ..Default::default()
            }),
        ],
    };
    let dense = solve(&p, &network, plan.clone()).unwrap();
    let streamed = solve_stream(&p, || stream_graph(fixture), plan).unwrap();
    vec![dense, streamed]
}

/// forEach excludes a row until every genuine field is assigned, even when
/// the scoring provider only refers to a subset of those fields.
pub fn dense_network(fixture: &Fixture) -> ConstraintNetwork<'static, String, SimpleScore> {
    let entities = fixture.rows.len();
    let mut network = ConstraintNetwork::new(entities * fixture.fields.len());
    for slot in scoring_slots(fixture) {
        let row = slot % entities;
        let dependencies = (0..fixture.fields.len())
            .map(|field| field * entities + row)
            .collect();
        network
            .add_constraint(format!("assigned {slot}"), dependencies, |values| {
                SimpleScore::new(i64::from(values.iter().all(|value| value.is_some())))
            })
            .unwrap();
    }
    network
}
pub fn stream_graph(
    fixture: &Fixture,
) -> Result<
    (
        StreamGraph<'static, SimpleScore>,
        PlanningStreamSource<String>,
    ),
    GraphError,
> {
    let mut builder = StreamGraphBuilder::<SimpleScore>::new();
    let source = builder.source::<usize, (usize, Option<String>)>();
    let entities = fixture.rows.len();
    let fields = fixture.fields.len();
    let rewards = fixture.scored.len() as i64;
    let rows = builder.group_list(source.port(), move |(slot, value)| {
        (slot % entities, (slot / entities, value.clone()))
    })?;
    builder.score("assigned", &rows, move |(_, values)| {
        if values.len() == fields && values.iter().all(|(_, value)| value.is_some()) {
            SimpleScore::new(rewards)
        } else {
            SimpleScore::ZERO
        }
    })?;
    Ok((builder.build(), source))
}
