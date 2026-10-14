// Pinned Community TEST helper adaptations, Apache-2.0; see provenance/NOTICE.
#![allow(dead_code)]
use std::sync::Arc;
use timetuck::*;
pub const NAME: &str = "Don't assign 2 entities the same value.";
pub struct PairProvider;
// The abstract TEST provider and its concrete override use the same ordered
// pairs. This native shared recipe is callable only through a concrete value.
pub trait OrderedPairRecipe {
    fn graph(&self) -> Result<CompiledConstraintGraph<String, SimpleScore>, GraphError> {
        let mut b = StreamGraphBuilder::new();
        let source = b.source::<usize, (usize, Option<String>)>();
        let assigned = b.filter(source.port(), |row| row.1.is_some())?;
        let joined = b.join_equal(
            &assigned,
            &assigned,
            |row| row.1.clone(),
            |row| row.1.clone(),
        )?;
        let pairs = b.filter(&joined, |(a, b)| a.0 != b.0)?;
        b.score_weighted(NAME, &pairs, SimpleScore::new(1), |_| -1)?;
        Ok(CompiledConstraintGraph {
            graph: b.build(),
            source,
        })
    }
}
impl OrderedPairRecipe for PairProvider {}
impl ReadOnlyConstraintProvider<String, SimpleScore> for PairProvider {
    fn compile(&self) -> Result<CompiledConstraintGraph<String, SimpleScore>, GraphError> {
        self.graph()
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Solution {
    pub values: Vec<String>,
    pub entities: Vec<Option<String>>,
    pub score: Option<SimpleScore>,
}
pub fn source() -> Solution {
    Solution {
        values: (1..3).map(|i| format!("v{i}")).collect(),
        entities: vec![None, None],
        score: None,
    }
}
pub fn model(s: &Solution) -> FiniteProblem<String> {
    FiniteProblem::new(
        s.entities
            .iter()
            .map(|e| {
                let v = Variable::new(s.values.clone());
                e.as_ref().map_or(v.clone(), |e| v.with_initial(e.clone()))
            })
            .collect(),
    )
    .unwrap()
}
pub fn resource() -> Arc<ScoreFactoryResource<String, SimpleScore>> {
    Arc::new(ScoreFactoryResource::new(Arc::new(PairProvider)).unwrap())
}
pub type Factory = SharedConstraintSolverFactory<Solution, Solution, String, String, SimpleScore>;
#[derive(Clone, Copy)]
pub enum Backend {
    Dense,
    Stream,
}
pub fn factory(backend: Backend) -> Arc<Factory> {
    factory_with(resource(), backend)
}
pub fn factory_with(
    resource: Arc<ScoreFactoryResource<String, SimpleScore>>,
    backend: Backend,
) -> Arc<Factory> {
    SharedConstraintSolverFactory::new(
        resource,
        SolverConfig {
            local_search: vec![LocalSearchConfig {
                max_steps: 10,
                ..Default::default()
            }],
            ..Default::default()
        },
        move |recipe, solution: Solution, configuration, progress| {
            run(recipe, solution, configuration, progress, backend)
        },
    )
    .unwrap()
}
pub fn run(
    recipe: &Arc<ScoreFactoryResource<String, SimpleScore>>,
    mut solution: Solution,
    configuration: SolverSequence<'_, SimpleScore>,
    progress: SolverProgress<Solution>,
    backend: Backend,
) -> Result<Solution, String> {
    let p = model(&solution);
    let result = match backend {
        Backend::Dense => {
            let mut n = ConstraintNetwork::new(p.variables().len());
            // The dense engine evaluates the canonical recipe for each
            // candidate; structural penalties are added by its own director.
            n.add_constraint(NAME, (0..p.variables().len()).collect(), |rows| {
                let a = Assignment::new(rows.iter().map(|v| v.cloned()).collect());
                recipe.score(&p, &a).unwrap().with_structural(0)
            })
            .map_err(|e| e.to_string())?;
            solve(&p, &n, configuration).map_err(|e| e.to_string())?
        }
        Backend::Stream => solve_stream(
            &p,
            || {
                recipe
                    .compile()
                    .map(|c| (c.graph, c.source))
                    .map_err(|e| GraphError::Dependency(e.to_string()))
            },
            configuration,
        )
        .map_err(|e| e.to_string())?,
    };
    progress.record_work(result.work);
    solution.entities = result.assignment.values().to_vec();
    solution.score = result.score;
    Ok(solution)
}

pub fn independent_score(s: &Solution) -> SimpleScore {
    let pairs = s
        .entities
        .iter()
        .enumerate()
        .map(|(i, a)| {
            s.entities
                .iter()
                .enumerate()
                .filter(|(j, b)| i != *j && a.is_some() && a == *b)
                .count()
        })
        .sum::<usize>();
    SimpleScore::new(-(pairs as i64))
        .with_structural(-(s.entities.iter().filter(|v| v.is_none()).count() as i64))
}
pub fn check_verifier(f: &Arc<Factory>) {
    let mut s = source();
    s.entities = vec![Some("v1".into()), Some("v1".into())];
    let verifier = f.constraint_verifier();
    let p = model(&s);
    verifier
        .verify_score(&p, &p.initial_assignment(), &SimpleScore::new(-2))
        .unwrap();
    s.entities[1] = Some("v2".into());
    let p = model(&s);
    verifier
        .verify_score(&p, &p.initial_assignment(), &SimpleScore::ZERO)
        .unwrap();
}
