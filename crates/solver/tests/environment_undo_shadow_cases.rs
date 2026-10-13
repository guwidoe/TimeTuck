// Adapted from Community EnvironmentModeTest.corruptedUndoShadowVariable.
// Apache-2.0; see compat/provenance.json. Native IDs/callbacks replace annotations.
#[path = "fixtures/corrupted_undo_shadow_fixture.rs"]
mod fixture;
use fixture::*;
use std::{io, rc::Rc};
use timetuck::*;
type Model = FiniteProblem<i64>;
type Factory = Box<dyn Fn() -> Calculator>;
struct Domain {
    model: Rc<Model>,
}
struct Prepared<'a> {
    domain: &'a Domain,
    mode: EnvironmentMode,
}
struct Resource<'a> {
    director: CalculatorScoreDirector<'a, i64, Calculator, Factory>,
}
struct Work(SolverResult<Assignment<i64>, SimpleScore>);
impl ContextScoreWork for Work {
    fn context_calculations(&self) -> u64 {
        self.0.work.evaluations
    }
}
impl ContextScoreResource for Resource<'_> {
    type Score = SimpleScore;
    type Error = io::Error;
    fn context_score(&self) -> SimpleScore {
        self.director.score().unwrap()
    }
    fn check_context(&mut self) -> Result<(), io::Error> {
        self.director.assert_consistent().map_err(io::Error::other)
    }
    fn context_totals(&self) -> Vec<(String, SimpleScore)> {
        vec![]
    }
}
#[test]
fn corrupted_undo_shadow_variable() {
    use EnvironmentMode::*;
    for mode in [
        TrackedFullAssert,
        FullAssert,
        StepAssert,
        PhaseAssert,
        NoAssert,
        NonReproducible,
        NonIntrusiveFullAssert,
    ] {
        let domain = Domain {
            model: Rc::new(
                Model::new(vec![
                    Variable::new(vec![0, 1]).with_initial(0),
                    Variable::new(vec![0, 1]).with_initial(1),
                ])
                .unwrap(),
            ),
        };
        let schema = Rc::new(
            PlanningSchema::new::<Model>(vec![
                PlanningEntityDefinition::of::<CorruptedUndoShadowEntity>(vec![
                    PlanningVariableDefinition::basic::<i64>("value", false),
                    PlanningVariableDefinition::shadow::<i64>("valueClone"),
                ]),
                PlanningEntityDefinition::of::<CorruptedUndoShadowValue>(vec![
                    PlanningVariableDefinition::shadow::<Vec<usize>>("entities"),
                ]),
            ])
            .unwrap(),
        );
        let prepare = |mode| {
            Ok(Prepared {
                domain: &domain,
                mode,
            })
        };
        let build = |prepared: &Prepared<'_>, settings: FactoryDirectorSettings, assignment| {
            assert_eq!(prepared.mode, settings.requested_mode);
            assert!(Rc::ptr_eq(&prepared.domain.model, &domain.model));
            let mode = settings.requested_mode;
            let factory: Factory = Box::new(move || Calculator::new(mode));
            Ok(Resource {
                director: CalculatorScoreDirector::new(&domain.model, assignment, factory, false)
                    .map_err(io::Error::other)?,
            })
        };
        let copy = |r: &Resource<'_>| Ok(r.director.assignment().clone());
        let declaration = ScoringFactoryDeclaration::default().with_scoring_recipe(
            ScoringFactoryRecipe::incremental_recipe(prepare, build, copy),
        );
        let mut config = SolverAssemblyConfiguration::for_factory_model(domain.model.clone());
        config.schema = Some(schema);
        config.environment = EnvironmentProfile::new(Some(mode), vec![None]);
        config.phases = vec![SolverPhase::LocalSearch(LocalSearchConfig {
            max_steps: u64::MAX,
            max_evaluations: 10,
            ..Default::default()
        })];
        config.run_limits.max_evaluations = Some(10);
        let factory = declaration
            .assemble_solver_factory(
                config,
                Rc::new(FactoryScoreShape::for_score::<SimpleScore>()),
            )
            .unwrap();
        let mut session = factory
            .start_assembled_run(domain.model.initial_assignment())
            .unwrap();
        let result = session.run_next_assembled_phase(|r, phase| {
            r.director
                .run_solver(SolverSequence {
                    phases: vec![phase.clone()],
                    ..Default::default()
                })
                .map(Work)
                .map_err(io::Error::other)
        });
        match mode {
            TrackedFullAssert | FullAssert | StepAssert => {
                let e = result
                    .err()
                    .unwrap_or_else(|| panic!("{mode:?} failed to detect undo corruption"));
                let text = match &e.primary {
                    FactoryExecutionError::Build(e) => e.to_string(),
                    FactoryExecutionError::Context(e) => e.to_string(),
                };
                assert!(text.contains("corrupted undoMove"), "{mode:?}: {text}");
                if mode == TrackedFullAssert {
                    assert!(
                        text.contains("Variables that are different between before and undo"),
                        "{text}"
                    );
                    assert!(text.contains("Actual value (v2) of variable valueClone on CorruptedUndoShadowEntity entity (CorruptedUndoShadowEntity) differs from expected (v1)"), "{text}");
                }
            }
            _ => {
                let work = result
                    .unwrap_or_else(|e| panic!("{mode:?}: {:?}", e.primary))
                    .unwrap();
                assert!(work.context_calculations() <= 10);
                session
                    .inspect_assembled_resource(|r| {
                        let c = r.director.calculator().unwrap();
                        for v in &c.solution.values {
                            let mut expected: Vec<_> = c
                                .solution
                                .entities
                                .iter()
                                .enumerate()
                                .filter(|(_, e)| e.value == Some(v.value))
                                .map(|(i, _)| i)
                                .collect();
                            let mut actual = v.entities.clone();
                            actual.sort_unstable();
                            expected.sort_unstable();
                            assert_eq!(actual, expected, "{mode:?} {}", name(Some(v.value)));
                        }
                        for (i, e) in c.solution.entities.iter().enumerate() {
                            assert_eq!(e.value, r.director.assignment().values()[i]);
                        }
                    })
                    .unwrap();
                let (_, _, count) = session.finish_assembled_state().unwrap();
                assert!(count <= 10);
            }
        }
    }
}
