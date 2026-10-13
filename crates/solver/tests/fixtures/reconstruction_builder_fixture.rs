// Builder TEST setup mapped to actual native construction metadata/provider sessions.
// Apache-2.0; see compat/provenance.json.
#![allow(dead_code)]
use timetuck::*;
pub struct Solution;
pub struct Entity;
pub type Provider = ConstructionFactoryProvider<i32>;
pub type Recipe<'s> = ConstructionPlacerRecipe<'s, i32>;
pub fn schema() -> PlanningSchema {
    PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Entity>(vec![
        PlanningVariableDefinition::basic::<i32>("value", false),
    ])])
    .unwrap()
}
pub fn context(mode: EnvironmentMode) -> ReconstructionContext<i32> {
    let problem = FiniteProblem::new(vec![Variable::new(vec![1, 2]); 2]).unwrap();
    let initial = problem.initial_assignment();
    ReconstructionContext::new(problem, initial, EnvironmentMode::PhaseAssert, mode).unwrap()
}
pub fn placer(
    schema: &PlanningSchema,
) -> RebuildablePlacer<Provider, Recipe<'_>, ConstructionFactoryError> {
    let plan = ConstructionPlacerPlan::infer(
        schema,
        ConstructionPlacerKind::QueuedEntity,
        None,
        &[],
        ConstructionEntitySort::Original,
    )
    .unwrap();
    let recipe = ConstructionPlacerRecipe {
        plan,
        problem: FiniteProblem::new(vec![Variable::new(vec![1, 2]); 2]).unwrap(),
        bindings: vec![ConstructionFieldBinding {
            name: "value".into(),
            slots: vec![0, 1],
        }],
        sorter: None,
    };
    RebuildablePlacer::build(recipe, |recipe: &Recipe<'_>| recipe.build_provider()).unwrap()
}
pub fn network() -> ConstraintNetwork<'static, i32, SimpleScore> {
    let mut n = ConstraintNetwork::new(2);
    n.add_constraint("sum", vec![0, 1], |v| {
        SimpleScore::new(v.iter().flatten().map(|&&v| i64::from(v)).sum())
    })
    .unwrap();
    n
}
pub fn fresh(a: &Assignment<i32>) -> Result<SimpleScore, String> {
    Ok(SimpleScore::new(
        a.values().iter().flatten().map(|v| i64::from(*v)).sum(),
    ))
}
