//! Original actual command execution and whole-configuration preflight checks.
use std::{cell::Cell, rc::Rc};
use timetuck::*;
struct Solution;
struct Entity;
fn schema(mixed: bool) -> PlanningSchema {
    let mut vars = vec![
        PlanningVariableDefinition::basic::<i32>("value", false),
        PlanningVariableDefinition::shadow::<usize>("owner"),
    ];
    if mixed {
        vars.push(PlanningVariableDefinition::list::<i32>("list", false));
    }
    PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Entity>(vars)]).unwrap()
}
fn problem() -> FiniteProblem<i32> {
    FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]).with_initial(0)]).unwrap()
}
fn network() -> ConstraintNetwork<'static, i32, SimpleScore> {
    let mut n = ConstraintNetwork::new(1);
    n.add_constraint("value", vec![0], |v| {
        SimpleScore::new(i64::from(*v[0].unwrap()))
    })
    .unwrap();
    n
}
fn sequence() -> SolverSequence<'static> {
    SolverSequence {
        phases: vec![SolverPhase::Custom(CustomPhaseConfig::default())],
        ..Default::default()
    }
}
type Registry = PhaseCommandRegistry<Assignment<i32>, Move<i32>, SimpleScore, ScoringError>;
type Command = Box<NativePhaseCommand<Assignment<i32>, Move<i32>, SimpleScore, ScoringError>>;
#[test]
fn default_basic_configuration_builds_and_runs_real_solver() {
    let settings = SolverBuildSettings::default();
    let factory = settings
        .build(&schema(false), None, |sequence, _| {
            SolverFactory::new(sequence, |p: FiniteProblem<i32>, config, progress| {
                let result = solve(&p, &network(), config).map_err(|e| e.to_string())?;
                progress.record_work(result.work);
                Ok::<_, String>(result.score.unwrap())
            })
            .map_err(|e| SolverPreflightError(e.to_string()))
        })
        .unwrap();
    let mut manager = factory.build_manager(1).unwrap();
    let result = manager
        .job_builder(1)
        .with_problem(problem())
        .run()
        .unwrap()
        .wait();
    assert_eq!(result.as_ref().as_ref().unwrap(), &SimpleScore::new(2));
    manager.shutdown().unwrap();
}
#[test]
fn exhaustive_nested_coverage_requires_real_fields_and_ignores_shadows() {
    let all = SolverBuildSettings {
        exhaustive_selection: Some(ExhaustiveFieldSelection::Cartesian(vec![
            ExhaustiveFieldSelection::BasicChange(None),
            ExhaustiveFieldSelection::All,
        ])),
        ..Default::default()
    };
    assert!(all.prepare(&schema(true), Some(sequence())).is_ok());
    for field in ["missing", "owner", "list"] {
        let bad = SolverBuildSettings {
            exhaustive_selection: Some(ExhaustiveFieldSelection::BasicChange(Some(
                PlanningFieldKey::of::<Entity>(field),
            ))),
            ..Default::default()
        };
        assert!(bad.validate(&schema(true)).is_err());
    }
    let mut empty = sequence();
    empty.phases.clear();
    assert!(
        SolverBuildSettings::default()
            .prepare(&schema(true), Some(empty))
            .is_err()
    );
    let mut deep = ExhaustiveFieldSelection::All;
    for _ in 0..130 {
        deep = ExhaustiveFieldSelection::Cartesian(vec![deep]);
    }
    assert!(
        SolverBuildSettings {
            exhaustive_selection: Some(deep),
            ..Default::default()
        }
        .validate(&schema(false))
        .is_err()
    );
}
#[test]
fn model_ranges_select_valid_cache_scope_and_unknown_fields_fail() {
    let p = ListProblem::new(
        vec![0_i32, 1],
        vec![ListEntity::new(vec![]).with_value_range(vec![0, 1])],
    )
    .unwrap();
    let field = PlanningFieldKey::of::<Entity>("list");
    for cache in [SelectionCacheScope::Step, SelectionCacheScope::Uncached] {
        let settings = SolverBuildSettings {
            selectors: vec![ConstructionSelectorDeclaration::for_list_problem(
                field.clone(),
                &p,
                true,
                cache,
                false,
            )],
            ..Default::default()
        };
        assert!(settings.validate(&schema(true)).is_ok());
    }
    let unknown = SolverBuildSettings {
        selectors: vec![ConstructionSelectorDeclaration::for_list_problem(
            PlanningFieldKey::of::<Entity>("missing"),
            &p,
            false,
            SelectionCacheScope::Step,
            false,
        )],
        ..Default::default()
    };
    assert!(unknown.validate(&schema(true)).is_err());
    let solver_cache = SolverBuildSettings {
        selectors: vec![ConstructionSelectorDeclaration::for_list_problem(
            field,
            &p,
            true,
            SelectionCacheScope::Solve,
            false,
        )],
        ..Default::default()
    };
    assert!(
        solver_cache
            .validate(&schema(true))
            .unwrap_err()
            .to_string()
            .contains("(SOLVER)")
    );
}
#[test]
fn command_registry_preflights_all_names_and_creates_independent_instances() {
    let made = Rc::new(Cell::new(0));
    let mut registry = Registry::default();
    let instances = made.clone();
    registry
        .register("advance", move || {
            instances.set(instances.get() + 1);
            let mut runs = 0;
            Box::new(move |context| {
                runs += 1;
                context.execute(&Move::Change {
                    variable: 0,
                    value: Some(runs),
                })?;
                Ok(())
            })
        })
        .unwrap();
    let names = CustomCommandNames::new(["advance".into(), "missing".into()]);
    assert!(registry.bind(&sequence(), &[(0, names)]).is_err());
    assert_eq!(made.get(), 0);
    assert!(
        registry
            .register("advance", || Box::new(|_| Ok(())))
            .is_err()
    );
    assert!(registry.bind(&sequence(), &[]).is_err());
    assert!(
        registry
            .bind(&sequence(), &[(1, CustomCommandNames::default())])
            .is_err()
    );
    for _ in 0..2 {
        let mut bound = registry
            .bind(
                &sequence(),
                &[(0, CustomCommandNames::new(["advance".into()]))],
            )
            .unwrap();
        let result = solve_with_extensions(
            &problem(),
            &network(),
            sequence(),
            SolverExtensions {
                command: Some(&mut |phase, ctx| {
                    let result = bound.execute(phase, ctx);
                    assert_eq!(ctx.work().committed_moves, 1);
                    result
                }),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(result.assignment.values(), &[Some(1)]);
    }
    assert_eq!(made.get(), 2);
}
#[test]
fn direct_commands_share_budgets_and_restore_best_before_later_commands() {
    let later = Rc::new(Cell::new(false));
    let observed = later.clone();
    let commands: Vec<Option<Command>> = vec![
        Some(Box::new(|ctx| {
            ctx.execute(&Move::Change {
                variable: 0,
                value: Some(2),
            })?;
            ctx.execute(&Move::Change {
                variable: 0,
                value: Some(0),
            })?;
            Ok(())
        })),
        Some(Box::new(move |ctx| {
            assert_eq!(ctx.assignment().values(), &[Some(2)]);
            observed.set(true);
            Ok(())
        })),
    ];
    let mut bound = CustomPhaseCommands::from_optional(commands)
        .unwrap()
        .into_bound(&sequence(), 0)
        .unwrap();
    let result = solve_with_extensions(
        &problem(),
        &network(),
        sequence(),
        SolverExtensions {
            command: Some(&mut |phase, ctx| {
                let result = bound.execute(phase, ctx);
                assert_eq!(ctx.work().restorations, 1);
                result
            }),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(later.get());
    assert_eq!(result.assignment.values(), &[Some(2)]);
    let mut limited = sequence();
    let SolverPhase::Custom(ref mut config) = limited.phases[0] else {
        unreachable!()
    };
    config.max_evaluations = 1;
    let invoked = Rc::new(Cell::new(false));
    let calls = invoked.clone();
    let command: Box<NativePhaseCommand<Assignment<i32>, Move<i32>, SimpleScore, ScoringError>> =
        Box::new(move |_| {
            calls.set(true);
            Ok(())
        });
    let mut bound = CustomPhaseCommands::from_optional([Some(command)])
        .unwrap()
        .into_bound(&limited, 0)
        .unwrap();
    let _ = solve_with_extensions(
        &problem(),
        &network(),
        limited,
        SolverExtensions {
            command: Some(&mut |phase, ctx| bound.execute(phase, ctx)),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(!invoked.get());
}
