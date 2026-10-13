// Complete pinned Community TEST adaptations; Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;
struct Solution;
struct Entity;
struct WeightSolution;
struct PinnedSolution;
struct IndexPinnedSolution;
struct ListEntity;
fn schema() -> PlanningSchema {
    PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Entity>(vec![
        PlanningVariableDefinition::basic::<i32>("value", false),
    ])])
    .unwrap()
}
#[test]
fn build_score_director() {
    let s = schema();
    let factory = EasyScoreFactory::new(
        &s,
        CalculatorProperties::default(),
        |_: &CalculatorProperties| Ok::<_, ()>(|_: &Assignment<i32>| SimpleScore::new(-10)),
    );
    let p = FiniteProblem::<i32>::new(vec![]).unwrap();
    let session = factory.build(p.initial_assignment()).unwrap();
    let network = session.network(0).unwrap();
    let director = ScoreDirector::new(&p, &network, p.initial_assignment()).unwrap();
    assert_eq!(director.score(), SimpleScore::new(-10));
    assert_eq!(director.score().structural(), 0);
}
struct Custom {
    string_property: String,
    int_property: i32,
}
impl EasyScoreCalculator<Assignment<i32>> for Custom {
    type Score = SimpleScore;
    fn calculate_score(&self, _: &Assignment<i32>) -> SimpleScore {
        SimpleScore::new(0)
    }
}
#[test]
fn easy_score_calculator_with_custom_properties() {
    let s = schema();
    let properties = CalculatorProperties::new([
        ("stringProperty".into(), "string 1".into()),
        ("intProperty".into(), "7".into()),
    ]);
    let factory = EasyScoreFactory::new(&s, properties, |p: &CalculatorProperties| {
        Ok::<_, CalculatorPropertyError>(Custom {
            string_property: p.required("stringProperty")?.into(),
            int_property: p.parse("intProperty")?,
        })
    });
    let director = factory.build(Assignment::<i32>::new(vec![])).unwrap();
    assert_eq!(director.calculator().string_property, "string 1");
    assert_eq!(director.calculator().int_property, 7);
    assert_eq!(director.calculate_score(), SimpleScore::new(0));
}
#[test]
fn score_director_factories_use_the_given_solution_descriptor() {
    let weight =
        PlanningSchema::new::<WeightSolution>(vec![PlanningEntityDefinition::of::<Entity>(vec![
            PlanningVariableDefinition::basic::<i32>("value", false),
        ])])
        .unwrap();
    let pinned =
        PlanningSchema::new::<PinnedSolution>(vec![PlanningEntityDefinition::of::<ListEntity>(
            vec![PlanningVariableDefinition::list::<i32>("values", false)],
        )])
        .unwrap();
    let index = PlanningSchema::new::<IndexPinnedSolution>(vec![PlanningEntityDefinition::of::<
        ListEntity,
    >(vec![
        PlanningVariableDefinition::list::<i32>("values", false),
    ])])
    .unwrap();
    for s in [&weight, &pinned, &index] {
        let factory = EasyScoreFactory::new(
            s,
            CalculatorProperties::default(),
            |_: &CalculatorProperties| Ok::<_, ()>(|_: &Assignment<i32>| SimpleScore::new(0)),
        );
        assert!(std::ptr::eq(factory.schema(), s));
        let session = factory.build(Assignment::<i32>::new(vec![])).unwrap();
        assert!(std::ptr::eq(session.schema(), s));
    }
}
