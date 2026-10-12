// Adapted Community TEST-only custom-cloner fixtures, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
#![allow(dead_code)]
use std::sync::Arc;
use timetuck::*;
#[derive(Debug, PartialEq, Eq)]
pub struct Value(pub &'static str);
pub struct Entity {
    pub code: &'static str,
    pub value: Option<Arc<Value>>,
}
pub struct Domain {
    pub entity: Arc<Entity>,
    pub values: Vec<Arc<Value>>,
    pub score: Option<SimpleScore>,
    pub cloned: bool,
}
#[derive(Clone, Copy)]
pub enum Policy {
    Correct,
    MissingScore,
    WrongScore,
    SharedEntity,
}
pub fn source(policy: Policy) -> Domain {
    Domain {
        entity: Arc::new(Entity {
            code: "A",
            value: None,
        }),
        values: match policy {
            Policy::Correct => vec![Arc::new(Value("1")), Arc::new(Value("2"))],
            Policy::MissingScore => vec![Arc::new(Value("1"))],
            Policy::WrongScore | Policy::SharedEntity => vec![],
        },
        score: None,
        cloned: false,
    }
}
pub fn accessors() -> SolutionBasicAccessors<'static, Domain, Arc<Value>> {
    let entities = PlanningComposition::new(vec![PlanningComponent::registered::<Entity>(vec![
        PlanningVariableDefinition::basic::<Arc<Value>>("value", false),
    ])])
    .unwrap();
    let layout = Arc::new(
        SolutionComposition::new(vec![PlanningSolutionComponent::registered::<Domain>(vec![
            SolutionMemberDefinition::entity::<Entity>("entity"),
            SolutionMemberDefinition::value_range::<Arc<Value>>("values", "range")
                .as_problem_fact_collection(),
            SolutionMemberDefinition::score::<SimpleScore>("score"),
        ])])
        .unwrap()
        .compile::<Domain>(
            &entities,
            &[PlanningType::of::<Entity>()],
            SolutionCompositionPolicy::default(),
            PlanningCompositionPolicy::default(),
        )
        .unwrap(),
    );
    let mut a = SolutionBasicAccessors::new(
        layout,
        vec![SolutionFieldRange::new::<Entity>(
            "value",
            vec!["range".into()],
        )],
    )
    .unwrap();
    a.entities("entity", |s: &Domain| {
        vec![(
            s.entity.clone(),
            vec![("value".into(), s.entity.value.clone())],
        )]
    })
    .unwrap();
    a.range("values", |s| s.values.clone()).unwrap();
    a.facts_mut()
        .collection("values", |s| {
            s.values.iter().cloned().map(Arc::new).collect::<Vec<_>>()
        })
        .unwrap();
    a
}
pub fn cloner(policy: Policy) -> BasicSolutionCloner<'static, Domain, Arc<Value>, SimpleScore> {
    BasicSolutionCloner::new(
        move |s: &Domain, a: &Assignment<Arc<Value>>, score: SimpleScore| Domain {
            entity: match policy {
                Policy::SharedEntity => s.entity.clone(),
                _ => Arc::new(Entity {
                    code: s.entity.code,
                    value: a.values()[0].clone(),
                }),
            },
            values: s.values.clone(),
            score: match policy {
                Policy::MissingScore => None,
                Policy::WrongScore => Some(SimpleScore::new(score.score() - 1)),
                _ => Some(score),
            },
            cloned: matches!(policy, Policy::Correct),
        },
        |s| s.score,
    )
}
pub fn run(
    policy: Policy,
) -> Result<ClonedBasicSolution<Domain, Arc<Value>, SimpleScore>, BasicSolutionCloneError> {
    let s = source(policy);
    let a = accessors();
    let n = ConstraintNetwork::new(1);
    cloner(policy).solve(
        &a,
        &s,
        &n,
        SolverConfig {
            local_search: vec![LocalSearchConfig {
                max_steps: 2,
                max_evaluations: 32,
                ..Default::default()
            }],
            ..Default::default()
        },
    )
}
