//! Original real solving, calculator independence and property failures.
use std::cell::Cell;
use timetuck::*;
struct Solution;
struct Entity;
fn schema() -> PlanningSchema {
    PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Entity>(vec![
        PlanningVariableDefinition::basic::<i32>("value", false),
    ])])
    .unwrap()
}
struct Sum {
    offset: i64,
    calls: Cell<usize>,
}
impl EasyScoreCalculator<Assignment<i32>> for Sum {
    type Score = SimpleScore;
    fn calculate_score(&self, a: &Assignment<i32>) -> SimpleScore {
        self.calls.set(self.calls.get() + 1);
        SimpleScore::new(
            self.offset
                + a.values()
                    .iter()
                    .flatten()
                    .map(|&v| i64::from(v))
                    .sum::<i64>(),
        )
    }
}
fn build(p: &CalculatorProperties) -> Result<Sum, CalculatorPropertyError> {
    Ok(Sum {
        offset: p.parse("offset")?,
        calls: Cell::new(0),
    })
}
#[test]
fn independent_calculators_and_native_directors_execute_moves_and_configured_solving() {
    let s = schema();
    let factory = EasyScoreFactory::new(
        &s,
        CalculatorProperties::new([("offset".into(), "7".into())]),
        build,
    );
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]).with_initial(0); 2]).unwrap();
    let mut first = factory.build(p.initial_assignment()).unwrap();
    let second = factory.build(p.initial_assignment()).unwrap();
    assert_eq!(first.calculate_score(), SimpleScore::new(7));
    assert_eq!(first.calculator().calls.get(), 1);
    assert_eq!(second.calculator().calls.get(), 0);
    *first.working_mut() = Assignment::new(vec![Some(2), Some(0)]);
    assert_eq!(first.calculate_score(), SimpleScore::new(9));
    assert_eq!(second.calculate_score(), SimpleScore::new(7));
    let n1 = first.network(2).unwrap();
    let n2 = second.network(2).unwrap();
    let mut d1 = ScoreDirector::new(&p, &n1, first.working().clone()).unwrap();
    let d2 = ScoreDirector::new(&p, &n2, second.working().clone()).unwrap();
    d1.apply(&Move::Change {
        variable: 1,
        value: Some(2),
    })
    .unwrap()
    .commit();
    assert_eq!(d1.score(), SimpleScore::new(11));
    assert_eq!(d2.score(), SimpleScore::new(7));
    d1.assert_consistent().unwrap();
    let result = solve_with_extensions(
        &p,
        &n2,
        SolverSequence {
            phases: vec![SolverPhase::LocalSearch(LocalSearchConfig {
                max_steps: 4,
                max_evaluations: 100,
                assert_scores: true,
                ..Default::default()
            })],
            limits: SolverLimits::default(),
        },
        SolverExtensions::default(),
    )
    .unwrap();
    assert_eq!(result.assignment.values(), &[Some(2), Some(2)]);
    assert_eq!(second.working(), &p.initial_assignment());
    assert!(std::ptr::eq(first.schema(), factory.schema()));
    assert!(std::ptr::eq(second.schema(), factory.schema()));
}
#[test]
fn bad_properties_fail_before_creating_sessions_and_valid_builder_can_retry() {
    let s = schema();
    let missing = EasyScoreFactory::new(&s, CalculatorProperties::default(), build);
    let error = match missing.build(Assignment::<i32>::new(vec![])) {
        Err(e) => e,
        Ok(_) => panic!("missing property accepted"),
    };
    assert_eq!(error, CalculatorPropertyError::Missing("offset".into()));
    let invalid = EasyScoreFactory::new(
        &s,
        CalculatorProperties::new([("offset".into(), "7x".into())]),
        build,
    );
    let error = match invalid.build(Assignment::<i32>::new(vec![])) {
        Err(e) => e,
        Ok(_) => panic!("bad integer accepted"),
    };
    assert_eq!(
        error,
        CalculatorPropertyError::Invalid {
            name: "offset".into(),
            value: "7x".into()
        }
    );
    let valid = EasyScoreFactory::new(
        &s,
        CalculatorProperties::new([("offset".into(), "-7".into())]),
        build,
    );
    for _ in 0..2 {
        let session = valid.build(Assignment::<i32>::new(vec![])).unwrap();
        assert_eq!(session.calculator().calls.get(), 0);
        assert_eq!(session.calculate_score(), SimpleScore::new(-7));
    }
}
