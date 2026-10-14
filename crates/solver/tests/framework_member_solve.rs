// Adapted pinned Community Quarkus getter/setter and private-constructor TESTs
// and their TEST-only fixtures. Apache-2.0; see THIRD_PARTY_NOTICES.md.
// Owned native factories/accessors replace JVM reflection and Quarkus injection.
use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};
use timetuck::*;

#[derive(Debug, Default)]
struct GetterEntity {
    value: Option<String>,
    getter_calls: AtomicU64,
    setter_calls: u64,
}
impl Clone for GetterEntity {
    fn clone(&self) -> Self {
        Self {
            value: self.value.clone(),
            getter_calls: AtomicU64::new(self.getter_calls.load(Ordering::Relaxed)),
            setter_calls: self.setter_calls,
        }
    }
}
impl GetterEntity {
    fn get_value(&self) -> Option<String> {
        self.getter_calls.fetch_add(1, Ordering::Relaxed);
        self.value.clone()
    }
    fn set_value(&mut self, value: Option<String>) {
        self.setter_calls += 1;
        self.value = value;
    }
}
#[derive(Clone, Debug)]
struct GetterSolution {
    values: Vec<String>,
    entities: Vec<GetterEntity>,
    score: Option<SimpleScore>,
}
fn getter_accessor() -> ExecutableMemberDescriptor<GetterEntity, Option<String>> {
    ExecutableMemberDescriptor::private_field_with_accessors(
        "value",
        |_: &GetterEntity| panic!("private field read bypassed getter"),
        |_, _| panic!("private field write bypassed setter"),
        GetterEntity::get_value,
        GetterEntity::set_value,
    )
    .unwrap()
}

fn config() -> SolverConfig<'static> {
    SolverConfig {
        limits: SolverLimits {
            max_evaluations: 256,
            max_move_evaluations: Some(128),
            max_time: Some(Duration::from_secs(5)),
            best_score_limit: Some(SimpleScore::ZERO),
            assert_phase_scores: true,
            ..Default::default()
        },
        construction: ConstructionConfig {
            max_evaluations: 128,
            assert_scores: true,
            ..Default::default()
        },
        local_search: vec![LocalSearchConfig {
            max_evaluations: 128,
            max_steps: 16,
            assert_scores: true,
            ..Default::default()
        }],
    }
}

// The getter fixture joins both entity directions and excludes self pairs.
// The private-constructor fixture uses unique pairs. Missing values are excluded.
fn equality_penalty(values: &[Option<&String>], directed: bool) -> SimpleScore {
    let mut penalty = 0;
    for left in 0..values.len() {
        for right in left + 1..values.len() {
            if let (Some(a), Some(b)) = (values[left], values[right])
                && a == b
            {
                penalty += if directed { 2 } else { 1 };
            }
        }
    }
    SimpleScore::new(-penalty)
}

fn solve_members(
    model: &FiniteProblem<String>,
    stream: bool,
    directed: bool,
    settings: SolverSequence<'_, SimpleScore>,
) -> Result<SolverResult<Assignment<String>, SimpleScore>, String> {
    let size = model.variables().len();
    let result = if stream {
        solve_stream(
            model,
            || {
                let mut builder = StreamGraphBuilder::new();
                let source = builder.source::<usize, (usize, Option<String>)>();
                let all = builder.collect_list(source.port(), |row| row.clone())?;
                builder.score("value collisions", &all, move |rows| {
                    let mut values = vec![None; size];
                    for (index, value) in rows.iter() {
                        values[*index] = value.as_ref();
                    }
                    equality_penalty(&values, directed)
                })?;
                Ok((builder.build(), source))
            },
            settings,
        )
    } else {
        let mut network = ConstraintNetwork::new(size);
        network
            .add_constraint("value collisions", (0..size).collect(), move |values| {
                equality_penalty(values, directed)
            })
            .map_err(|error| error.to_string())?;
        solve(model, &network, settings)
    }
    .map_err(|error| error.to_string())?;
    assert!(result.complete && model.is_complete(&result.assignment));
    model.validate(&result.assignment).unwrap();
    assert_eq!(result.score, Some(SimpleScore::ZERO));
    assert_eq!(result.termination, SolverTermination::BestScoreLimit);
    assert!(result.work.evaluations <= 256);
    assert!(result.work.move_evaluations <= 128);
    Ok(result)
}

fn getter_factory(stream: bool) -> SolverFactory<GetterSolution, GetterSolution, String> {
    SolverFactory::new(
        config(),
        move |mut input: GetterSolution, settings, progress| {
            let accessor = getter_accessor();
            let variables = input
                .entities
                .iter()
                .map(|entity| {
                    let mut variable = Variable::new(input.values.clone());
                    if let Some(value) = accessor.read(entity).unwrap() {
                        variable = variable.with_initial(value);
                    }
                    variable
                })
                .collect();
            let model = FiniteProblem::new(variables).map_err(|error| error.to_string())?;
            let result = solve_members(&model, stream, true, settings)?;
            for (entity, value) in input.entities.iter_mut().zip(result.assignment.values()) {
                accessor
                    .write(entity, value.clone())
                    .map_err(|error| error.to_string())?;
            }
            input.score = result.score;
            progress.record_work(result.work);
            Ok(input)
        },
    )
    .unwrap()
}

#[test]
fn use_getters_setters_solve() {
    for stream in [false, true] {
        let factory = getter_factory(stream);
        let mut manager = factory.build_manager(1).unwrap();
        let problem = GetterSolution {
            values: (1..3).map(|i| format!("v{i}")).collect(),
            entities: (1..3).map(|_| GetterEntity::default()).collect(),
            score: None,
        };
        let job = manager.job_builder(1).with_problem(problem).run().unwrap();
        let finished = job.wait_timeout(Duration::from_secs(10)).unwrap();
        let solution = finished.as_ref().as_ref().unwrap(); // assertNotNull(solution).
        assert!(solution.score.unwrap().score() >= 0);
        for entity in &solution.entities {
            // Check returned counters before supplementary independent rescore.
            assert!(entity.getter_calls.load(Ordering::Relaxed) > 0);
            assert!(entity.setter_calls > 0);
        }
        let actual: Vec<_> = solution
            .entities
            .iter()
            .map(GetterEntity::get_value)
            .collect();
        assert!(
            actual
                .iter()
                .all(|value| solution.values.contains(value.as_ref().unwrap()))
        );
        assert_ne!(actual[0], actual[1]);
        assert_eq!(solution.score, Some(SimpleScore::ZERO));
        manager.shutdown().unwrap();
    }
}

mod private_fixture {
    use super::*;
    #[derive(Clone, Debug)]
    pub struct Entity {
        pub id: String,
        value: Option<String>,
    }
    impl Entity {
        fn empty() -> Self {
            Self {
                id: String::new(),
                value: None,
            }
        }
        pub fn identified(id: String) -> Self {
            Self {
                id,
                ..Self::empty()
            }
        }
        pub fn get_value(&self) -> Option<String> {
            self.value.clone()
        }
        pub fn set_value(&mut self, value: Option<String>) {
            self.value = value;
        }
    }
    #[derive(Clone, Debug)]
    pub struct Solution {
        pub entities: Vec<Entity>,
        pub score: Option<SimpleScore>,
        pub some_field: i32,
    }
    impl Solution {
        fn empty() -> Self {
            Self {
                entities: vec![],
                score: None,
                some_field: 1,
            }
        }
        pub fn with_entities(entities: Vec<Entity>) -> Self {
            Self {
                entities,
                score: None,
                some_field: 2,
            }
        }
        pub fn default_field_for_control() -> i32 {
            Self::empty().some_field
        }
        pub fn value_range(&self) -> Vec<String> {
            (1..=3).map(|i| i.to_string()).collect()
        }
    }
    pub struct Provider;
    impl Provider {
        fn new() -> Self {
            Self
        }
        // Explicit native construction bridge lives in the owning module.
        pub fn load() -> Self {
            Self::new()
        }
        pub fn score(&self, values: &[Option<&String>]) -> SimpleScore {
            equality_penalty(values, false)
        }
    }
}

fn private_factory(
    stream: bool,
) -> SolverFactory<private_fixture::Solution, private_fixture::Solution, String> {
    SolverFactory::new(
        config(),
        move |mut input: private_fixture::Solution, settings, progress| {
            let accessor = ExecutableMemberDescriptor::method(
                "getValue",
                MemberExposure::Public,
                private_fixture::Entity::get_value,
            )
            .unwrap()
            .with_method_setter("setValue", private_fixture::Entity::set_value)
            .unwrap();
            let variables = input
                .entities
                .iter()
                .map(|entity| {
                    let mut variable = Variable::new(input.value_range());
                    if let Some(value) = accessor.read(entity).unwrap() {
                        variable = variable.with_initial(value);
                    }
                    variable
                })
                .collect();
            let model = FiniteProblem::new(variables).map_err(|error| error.to_string())?;
            let provider = private_fixture::Provider::load();
            let result = if stream {
                solve_stream(
                    &model,
                    || {
                        let mut builder = StreamGraphBuilder::new();
                        let source = builder.source::<usize, (usize, Option<String>)>();
                        let rows = builder.collect_list(source.port(), |row| row.clone())?;
                        builder.score("Same value", &rows, |rows| {
                            let mut values = vec![None; 3];
                            for (index, value) in rows.iter() {
                                values[*index] = value.as_ref();
                            }
                            provider.score(&values)
                        })?;
                        Ok((builder.build(), source))
                    },
                    settings,
                )
                .map_err(|error| error.to_string())?
            } else {
                let mut network = ConstraintNetwork::new(3);
                network
                    .add_constraint("Same value", vec![0, 1, 2], |values| provider.score(values))
                    .unwrap();
                solve(&model, &network, settings).map_err(|error| error.to_string())?
            };
            assert!(result.complete && model.is_complete(&result.assignment));
            assert!(result.work.evaluations <= 256);
            for (entity, value) in input.entities.iter_mut().zip(result.assignment.values()) {
                accessor
                    .write(entity, value.clone())
                    .map_err(|error| error.to_string())?;
            }
            input.score = result.score;
            progress.record_work(result.work);
            Ok(input)
        },
    )
    .unwrap()
}

#[test]
fn can_construct_beans_with_private_constructors() {
    assert_eq!(private_fixture::Solution::default_field_for_control(), 1);
    for stream in [false, true] {
        let factory = private_factory(stream);
        let mut manager = factory.build_manager(1).unwrap();
        let problem = private_fixture::Solution::with_entities(
            (1..=3)
                .map(|i| private_fixture::Entity::identified(i.to_string()))
                .collect(),
        );
        let job = manager.job_builder(1).with_problem(problem).run().unwrap();
        let finished = job.wait_timeout(Duration::from_secs(10)).unwrap();
        let solution = finished.as_ref().as_ref().unwrap();
        assert_eq!(solution.score.unwrap().score(), 0);
        assert_eq!(solution.some_field, 2);
        let mut values: Vec<_> = solution
            .entities
            .iter()
            .map(|e| e.get_value().unwrap())
            .collect();
        values.sort();
        assert_eq!(values, vec!["1", "2", "3"]);
        assert_eq!(
            solution
                .entities
                .iter()
                .map(|e| e.id.as_str())
                .collect::<Vec<_>>(),
            vec!["1", "2", "3"]
        );
        manager.shutdown().unwrap();
    }
}
