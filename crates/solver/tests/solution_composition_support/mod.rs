// Test-domain declarations, generators and scoring adapted from the Community
// solution-inheritance fixtures. Apache-2.0; see compat/provenance.json.
use std::sync::Arc;
use timetuck::*;
pub struct Root<const N: usize>;
pub struct Parent<const N: usize>;
pub struct Grand<const N: usize>;
pub struct BaseEntity<const N: usize> {
    // Source success fixtures leave the base singleton absent; originals fill it.
    #[allow(dead_code)]
    pub code: String,
}
pub struct ChildEntity<const N: usize> {
    pub code: String,
}
// Source and original targets exercise different subsets of these fixtures.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Shape {
    Both,
    Abstract,
    BaseOnly,
    ChildOnly,
    Replaced,
    Multiple,
}
pub fn t<T: 'static>() -> PlanningType {
    PlanningType::of::<T>()
}
fn value() -> PlanningVariableDefinition {
    PlanningVariableDefinition::basic::<String>("value", false)
}
fn base_members<const N: usize>() -> Vec<SolutionMemberDefinition> {
    vec![
        SolutionMemberDefinition::value_range::<String>("valueList", "valueRange"),
        SolutionMemberDefinition::entity_collection::<ChildEntity<N>>("entityList"),
        SolutionMemberDefinition::score::<SimpleScore>("score"),
    ]
}
pub fn declarations<const N: usize>(shape: Shape) -> (SolutionComposition, PlanningComposition) {
    let mut base = base_members::<N>();
    let mut child = vec![];
    if shape == Shape::Both {
        base.insert(
            1,
            SolutionMemberDefinition::value_range::<String>("subValueList", "subValueRange"),
        );
        child.extend([
            SolutionMemberDefinition::entity::<BaseEntity<N>>("entity"),
            SolutionMemberDefinition::entity::<ChildEntity<N>>("subEntity"),
            SolutionMemberDefinition::entity_collection::<ChildEntity<N>>("subEntityList"),
            SolutionMemberDefinition::entity_collection::<ChildEntity<N>>("objectEntityList"),
            SolutionMemberDefinition::entity_collection::<ChildEntity<N>>("rawEntityList"),
        ]);
    } else if matches!(shape, Shape::Abstract | Shape::ChildOnly) {
        child.push(
            SolutionMemberDefinition::entity_collection::<ChildEntity<N>>("secondEntityList"),
        );
    } else if shape == Shape::Replaced {
        child.push(SolutionMemberDefinition::entity_collection::<ChildEntity<N>>("entityList"));
    }
    let solution_components = if shape == Shape::Multiple {
        vec![
            PlanningSolutionComponent::registered::<Grand<N>>(vec![base.remove(0)]),
            PlanningSolutionComponent::registered::<Parent<N>>(base).with_parent::<Grand<N>>(),
            PlanningSolutionComponent::registered::<Root<N>>(child).with_parent::<Parent<N>>(),
        ]
    } else {
        vec![
            if shape == Shape::ChildOnly {
                PlanningSolutionComponent::plain::<Parent<N>>(base)
            } else {
                PlanningSolutionComponent::registered::<Parent<N>>(base)
            },
            if shape == Shape::BaseOnly {
                PlanningSolutionComponent::plain::<Root<N>>(child).with_parent::<Parent<N>>()
            } else {
                PlanningSolutionComponent::registered::<Root<N>>(child).with_parent::<Parent<N>>()
            },
        ]
    };
    let direct = matches!(shape, Shape::Replaced | Shape::Multiple);
    let child_vars = if shape == Shape::Both {
        vec![PlanningVariableDefinition::basic::<String>(
            "subValue", false,
        )]
    } else if direct {
        vec![value()]
    } else {
        vec![]
    };
    let child_entity = if shape == Shape::BaseOnly {
        PlanningComponent::plain::<ChildEntity<N>>(child_vars)
    } else {
        PlanningComponent::registered::<ChildEntity<N>>(child_vars)
    };
    (
        SolutionComposition::new(solution_components).unwrap(),
        PlanningComposition::new(vec![
            PlanningComponent::registered::<BaseEntity<N>>(vec![value()]),
            if direct {
                child_entity
            } else {
                child_entity.with_parent::<BaseEntity<N>>(PlanningParentKind::Owned)
            },
        ])
        .unwrap(),
    )
}
pub fn compile<const N: usize>(
    shape: Shape,
    policy: SolutionCompositionPolicy,
) -> Result<ComposedPlanningSolution, SolutionCompositionError> {
    let (solution, entities) = declarations::<N>(shape);
    let mut roots = vec![t::<ChildEntity<N>>()];
    if shape == Shape::Both {
        roots.push(t::<BaseEntity<N>>());
    }
    solution.compile::<Root<N>>(
        &entities,
        &roots,
        policy,
        PlanningCompositionPolicy::default(),
    )
}
pub struct Fixture<const N: usize> {
    pub actual_shape: Shape,
    pub collections: Vec<SolutionCollection<String>>,
    pub ranges: Vec<SolutionRange<String>>,
    pub fields: Vec<SolutionFieldRange>,
}
/// Source 3-value/2-entity generators. Both has four collections (8 distinct
/// child objects), Abstract two (4), and BaseOnly one (2). Both's nullable
/// singleton properties stay empty. All genuine fields begin unassigned.
pub fn generate<const N: usize>(shape: Shape) -> Fixture<N> {
    let names = match shape {
        Shape::Both => vec![
            "entityList",
            "subEntityList",
            "objectEntityList",
            "rawEntityList",
        ],
        Shape::Abstract => vec!["entityList", "secondEntityList"],
        _ => vec!["entityList"],
    };
    let mut collections = vec![];
    for (column, name) in names.iter().enumerate() {
        let entities = (0..2)
            .map(|i| {
                let object = Arc::new(ChildEntity::<N> {
                    code: (i * names.len() + column).to_string(),
                });
                let mut values = vec![("value".into(), None)];
                if shape == Shape::Both {
                    values.push(("subValue".into(), None));
                }
                SolutionEntityRecord::new(object, values)
            })
            .collect();
        collections.push(SolutionCollection {
            member: (*name).into(),
            entities,
        });
    }
    if shape == Shape::Both {
        collections.extend([
            SolutionCollection {
                member: "entity".into(),
                entities: vec![],
            },
            SolutionCollection {
                member: "subEntity".into(),
                entities: vec![],
            },
        ]);
    }
    let mut ranges = vec![SolutionRange {
        member: "valueList".into(),
        values: (0..3).map(|i| format!("Generated Value {i}")).collect(),
    }];
    let mut fields = vec![SolutionFieldRange::new::<ChildEntity<N>>(
        "value",
        vec!["valueRange".into()],
    )];
    if shape == Shape::Both {
        ranges.push(SolutionRange {
            member: "subValueList".into(),
            values: (0..3).map(|i| format!("Generated Subvalue {i}")).collect(),
        });
        fields.extend([
            SolutionFieldRange::new::<ChildEntity<N>>("subValue", vec!["subValueRange".into()]),
            SolutionFieldRange::new::<BaseEntity<N>>("value", vec!["valueRange".into()]),
        ]);
    }
    Fixture {
        actual_shape: shape,
        collections,
        ranges,
        fields,
    }
}
pub fn bind<const N: usize>(
    layout: &ComposedPlanningSolution,
    fixture: Fixture<N>,
) -> BoundBasicSolution<String> {
    layout
        .bind_basic(fixture.collections, fixture.ranges, fixture.fields)
        .unwrap()
}
pub fn dense<const N: usize>(
    bound: &BoundBasicSolution<String>,
    rewards: usize,
) -> ConstraintNetwork<'static, String, SimpleScore> {
    let mut network = ConstraintNetwork::new(bound.problem().variables().len());
    for (row, entity) in bound.entities().iter().enumerate() {
        let row_rewards = if entity.object_ref::<ChildEntity<N>>().is_some() {
            rewards
        } else {
            rewards.min(1)
        };
        for reward in 0..row_rewards {
            network
                .add_constraint(
                    format!("Constraint{reward}/row{row}"),
                    entity.slots().iter().map(|(_, s)| *s).collect(),
                    |values| SimpleScore::new(i64::from(values.iter().all(Option::is_some))),
                )
                .unwrap();
        }
    }
    network
}
pub fn stream<const N: usize>(
    bound: &BoundBasicSolution<String>,
    rewards: usize,
) -> Result<
    (
        StreamGraph<'static, SimpleScore>,
        PlanningStreamSource<String>,
    ),
    GraphError,
> {
    let mut slot_rows = vec![0usize; bound.problem().variables().len()];
    let lengths: Vec<_> = bound.entities().iter().map(|e| e.slots().len()).collect();
    for (row, entity) in bound.entities().iter().enumerate() {
        for (_, slot) in entity.slots() {
            slot_rows[*slot] = row;
        }
    }
    let children: Vec<_> = bound
        .entities()
        .iter()
        .map(|e| e.object_ref::<ChildEntity<N>>().is_some())
        .collect();
    let mut builder = StreamGraphBuilder::<SimpleScore>::new();
    let source = builder.source::<usize, (usize, Option<String>)>();
    let rows = builder.group_list(source.port(), move |(slot, value)| {
        (slot_rows[*slot], value.clone())
    })?;
    for reward in 0..rewards {
        let lengths = lengths.clone();
        let children = children.clone();
        builder.score(
            format!("Constraint{reward}"),
            &rows,
            move |(row, values)| {
                SimpleScore::new(i64::from(
                    (reward == 0 || children[*row])
                        && values.len() == lengths[*row]
                        && values.iter().all(Option::is_some),
                ))
            },
        )?;
    }
    Ok((builder.build(), source))
}
pub fn solve_fixture<const N: usize>(
    bound: &BoundBasicSolution<String>,
    rewards: usize,
    move_limit: u64,
) -> Vec<SolverResult<Assignment<String>, SimpleScore>> {
    let sequence = SolverSequence {
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
    vec![
        solve(
            bound.problem(),
            &dense::<N>(bound, rewards),
            sequence.clone(),
        )
        .unwrap(),
        solve_stream(bound.problem(), || stream::<N>(bound, rewards), sequence).unwrap(),
    ]
}
