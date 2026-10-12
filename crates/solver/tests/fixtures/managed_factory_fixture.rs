// Adapted TEST-only PlannerTestUtils two-value/two-entity and zero-score fixtures.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
#![allow(dead_code)]
use std::{error::Error, fmt, sync::Arc};
use timetuck::*;
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Value {
    pub code: String,
}
#[derive(Clone, Debug)]
pub struct Entity {
    pub code: String,
    pub value: Option<Arc<Value>>,
}
#[derive(Clone, Debug)]
pub struct Solution {
    pub code: String,
    pub values: Vec<Arc<Value>>,
    pub entities: Vec<Entity>,
    pub score: Option<SimpleScore>,
}
#[derive(Debug)]
pub enum Failure {
    ModeledAllocation(&'static str),
    Native(String),
}
impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ModeledAllocation(s) => f.write_str(s),
            Self::Native(s) => f.write_str(s),
        }
    }
}
impl Error for Failure {}
pub fn source(code: impl Into<String>) -> Solution {
    Solution {
        code: code.into(),
        values: (1..=2)
            .map(|i| {
                Arc::new(Value {
                    code: format!("v{i}"),
                })
            })
            .collect(),
        entities: (1..=2)
            .map(|i| Entity {
                code: format!("e{i}"),
                value: None,
            })
            .collect(),
        score: None,
    }
}
pub fn config() -> SolverConfig<'static> {
    SolverConfig {
        local_search: vec![LocalSearchConfig {
            max_steps: 10,
            ..Default::default()
        }],
        ..Default::default()
    }
}
fn projected(
    source: &Solution,
    assignment: &Assignment<Arc<Value>>,
    score: Option<SimpleScore>,
) -> Solution {
    let mut out = source.clone();
    for (e, v) in out.entities.iter_mut().zip(assignment.values()) {
        e.value = v.clone();
    }
    out.score = score;
    out
}
pub fn run(
    source: Solution,
    config: SolverSequence<'_, SimpleScore>,
    progress: SolverProgress<Solution>,
) -> Result<Solution, Failure> {
    let problem = FiniteProblem::new(
        source
            .entities
            .iter()
            .map(|e| {
                let v = Variable::new(source.values.clone());
                e.value
                    .as_ref()
                    .map_or(v.clone(), |initial| v.with_initial(initial.clone()))
            })
            .collect(),
    )
    .map_err(|e| Failure::Native(e.to_string()))?;
    // The TEST helper uses DummySimpleScoreEasyScoreCalculator returning zero.
    let network = ConstraintNetwork::<_, SimpleScore>::new(source.entities.len());
    let result = solve_with_observer(&problem, &network, config, |event| {
        progress.publish(projected(&source, event.assignment, Some(event.score)));
    })
    .map_err(|e| Failure::Native(e.to_string()))?;
    progress.record_work(result.work);
    Ok(projected(&source, &result.assignment, result.score))
}
pub fn factory() -> SolverFactory<Solution, Solution, Failure> {
    SolverFactory::new(config(), run).unwrap()
}
