//! Original search/rollback integration; no upstream method credit.
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
struct Root;
struct Entity;
#[derive(Debug, PartialEq)]
struct Cost(i64); // Deliberately not Clone.
fn schema<A: 'static>() -> PlanningSchema {
    PlanningSchema::new::<Root>(vec![PlanningEntityDefinition::of::<Entity>(vec![
        PlanningVariableDefinition::basic::<A>("world", false),
        PlanningVariableDefinition::shadow::<i64>("cost"),
    ])])
    .unwrap()
}
fn graph<'a, A: Clone + Eq + 'static>(
    s: &'a PlanningSchema,
    objective: fn(&A) -> i64,
    fail: &'a Cell<u8>,
) -> (ShadowSession<'a, 'a, usize, A>, ShadowNode<i64>) {
    let entity = s.entity::<Entity>().unwrap();
    let mut b = ShadowSessionBuilder::<usize, A>::new(s);
    let input = b
        .input(entity.variable("world").unwrap(), 0, |a| Some(a.clone()))
        .unwrap();
    let source = input.clone();
    let cost = b
        .derive(entity.variable("cost").unwrap(), 0, move |_, i| {
            assert_ne!(fail.get(), 4, "supplier panic");
            Some(objective(i.get(&source).unwrap().unwrap()))
        })
        .unwrap();
    b.set_dependencies(&cost, &[input.erase()]).unwrap();
    (b.build(), cost)
}
fn list_cost(a: &ListAssignment<i32>) -> i64 {
    -a.lists()
        .iter()
        .flat_map(|row| {
            row.iter()
                .enumerate()
                .map(|(i, &v)| (i as i64 + 1) * i64::from(v))
        })
        .sum::<i64>()
}
type Lists<'a> = ShadowCalculator<
    'a,
    'a,
    usize,
    ListAssignment<i32>,
    ListDerivedShadows<i32, Cost>,
    SimpleScore,
    ModelShadowError<ListError, i32, &'static str>,
>;
fn list_calculator<'a>(
    s: &'a PlanningSchema,
    p: &'a ListProblem<i32>,
    calls: &'a Cell<u64>,
    fail: &'a Cell<u8>,
) -> Lists<'a> {
    let (graph, cost) = graph(s, list_cost, fail);
    let manager = list_declarative_solution_manager(
        p,
        graph,
        move |_, g| {
            if fail.get() == 1 {
                return Err("projection");
            }
            Ok(Cost(*g.read(&cost).unwrap().value.unwrap()))
        },
        |_, _| Ok(()),
        move |_, h| {
            calls.set(calls.get() + 1);
            if fail.get() == 2 {
                return Err("scoring");
            }
            assert_ne!(fail.get(), 3, "scoring panic");
            let score = SimpleScore::new(h.derived.0);
            Ok(if fail.get() == 5 {
                score.with_structural(-1)
            } else {
                score
            })
        },
    );
    ShadowCalculator::new(manager, || ListDerivedShadows {
        builtin: vec![],
        derived: Cost(0),
    })
}
fn problem() -> ListProblem<i32> {
    ListProblem::new(
        vec![1, 2, 3],
        vec![ListEntity::new(vec![1, 2, 3]), ListEntity::new(vec![])],
    )
    .unwrap()
}
fn relocation() -> ListMove<i32> {
    ListMove::Change {
        source: ListPosition {
            entity: 0,
            index: 2,
        },
        destination: ListPosition {
            entity: 1,
            index: 0,
        },
    }
}
#[test]
fn forward_inverse_and_commit_notifications_keep_graph_score_consistent() {
    let s = schema::<ListAssignment<i32>>();
    let p = problem();
    let calls = Cell::new(0);
    let fail = Cell::new(0);
    let factory = || list_calculator(&s, &p, &calls, &fail);
    let mut d =
        ListCalculatorScoreDirector::new(&p, p.initial_assignment(), factory, true).unwrap();
    assert!(!d.registry().unwrap().is_enabled());
    assert_eq!(d.score().unwrap(), SimpleScore::new(-14));
    {
        let trial = d.apply(&relocation()).unwrap();
        assert_eq!(trial.score(), SimpleScore::new(-8));
    }
    assert_eq!(calls.get(), 2, "inverse refresh must not score");
    assert_eq!(d.assignment(), &p.initial_assignment());
    assert_eq!(d.score().unwrap(), SimpleScore::new(-14));
    d.assert_consistent().unwrap();
    d.apply(&relocation()).unwrap().commit();
    assert_eq!(d.score().unwrap(), SimpleScore::new(-8));
    d.assert_consistent().unwrap();
}
#[test]
fn local_search_replays_winner_without_rescoring_and_exact_search_finds_oracle_optimum() {
    let s = schema::<ListAssignment<i32>>();
    let p = problem();
    let calls = Cell::new(0);
    let fail = Cell::new(0);
    let factory = || list_calculator(&s, &p, &calls, &fail);
    let result = list_calculator_local_search_from(
        &p,
        p.initial_assignment(),
        factory,
        LocalSearchConfig {
            max_steps: 1,
            max_evaluations: 100,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(calls.get(), result.evaluations); // Includes initial scoring.
    assert!(result.accepted_moves > 0);
    let best = result.best.unwrap();
    assert_eq!(best.score, SimpleScore::new(list_cost(&best.assignment)));
    let result = solve_list_calculator(
        &p,
        &factory,
        SolverSequence {
            phases: vec![
                SolverPhase::LocalSearch(LocalSearchConfig {
                    max_steps: 5,
                    max_evaluations: 100,
                    assert_scores: true,
                    ..Default::default()
                }),
                SolverPhase::Exact(ExactPhaseConfig::default()),
            ],
            ..Default::default()
        },
    )
    .unwrap();
    // Three positive values occupy two rows. Base cost is 6, and at least one
    // value has index >=1; putting value1 there attains the lower bound of 7.
    assert!(result.is_optimal());
    assert_eq!(result.score, Some(SimpleScore::new(-7)));
    assert_eq!(list_cost(&result.assignment), -7);
}
#[test]
fn failures_and_panics_hide_partial_state_until_successful_reset() {
    let s = schema::<ListAssignment<i32>>();
    let p = problem();
    let calls = Cell::new(0);
    let fail = Cell::new(0);
    for mode in 1..=5 {
        fail.set(0);
        let mut session = CalculatorSession::new(
            p.initial_assignment(),
            list_calculator(&s, &p, &calls, &fail),
            true,
        )
        .unwrap();
        assert!(session.calculator().solution().unwrap().score.is_none());
        assert_eq!(session.score().unwrap(), SimpleScore::new(-14));
        fail.set(mode);
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            session.change(&vec![0, 1], |a| {
                p.apply(a, &relocation()).unwrap().commit();
            })?;
            assert!(session.calculator().solution().unwrap().score.is_none());
            session.score()
        }));
        if mode == 3 || mode == 4 {
            assert!(outcome.is_err());
        } else {
            assert!(outcome.unwrap().is_err());
        }
        assert!(matches!(
            session.calculator().solution(),
            Err(MatchRegistryError::NeedsReset)
        ));
        assert_eq!(session.score(), Err(MatchRegistryError::NeedsReset));
        fail.set(1);
        assert!(session.reset(p.initial_assignment()).is_err());
        assert!(session.calculator().solution().is_err());
        fail.set(0);
        session.reset(p.initial_assignment()).unwrap();
        assert_eq!(session.score().unwrap(), SimpleScore::new(-14));
    }
}
fn simple<'a, A: Eq + Clone + 'static, C>(
    s: &'a PlanningSchema,
    objective: fn(&A) -> i64,
    fail: &'a Cell<u8>,
) -> ShadowCalculator<'a, 'a, usize, A, Cost, SimpleScore, &'static str, C> {
    let (g, cost) = graph(s, objective, fail);
    let m = ShadowSolutionManager::new(
        g,
        move |_, g| Ok(Cost(*g.read(&cost).unwrap().value.unwrap())),
        |_, _| Ok(()),
        |_, h| Ok(SimpleScore::new(h.0)),
    );
    ShadowCalculator::new(m, || Cost(0))
}
#[test]
fn basic_and_mixed_searches_use_derived_scores_and_director_structural_penalties() {
    let fail = Cell::new(0);
    let p = FiniteProblem::new(vec![
        Variable::new(vec![1, 2, 3]),
        Variable::new(vec![1, 2, 3]),
    ])
    .unwrap();
    let s = schema::<Assignment<i32>>();
    let objective = |a: &Assignment<i32>| {
        -a.values()
            .iter()
            .flatten()
            .map(|&v| i64::from(3 - v).pow(2))
            .sum::<i64>()
    };
    let factory = || simple(&s, objective, &fail);
    let d = CalculatorScoreDirector::new(&p, p.initial_assignment(), factory, false).unwrap();
    assert_eq!(d.score().unwrap().structural(), -2);
    let result = solve_calculator(
        &p,
        &factory,
        SolverSequence {
            phases: vec![
                SolverPhase::Construction(ConstructionConfig::default()),
                SolverPhase::Exact(ExactPhaseConfig::default()),
            ],
            ..Default::default()
        },
    )
    .unwrap();
    assert!(result.is_optimal());
    assert_eq!(result.assignment.values(), &[Some(3), Some(3)]);
    assert_eq!(result.score, Some(SimpleScore::ZERO));
    let p = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![1, 2]).with_initial(2)]).unwrap(),
        ListProblem::new(vec![1, 2], vec![ListEntity::new(vec![1, 2])]).unwrap(),
    );
    let s = schema::<MixedAssignment<i32, i32>>();
    let objective = |a: &MixedAssignment<i32, i32>| {
        list_cost(a.lists()) * i64::from(a.basic().values()[0].unwrap_or(1))
    };
    let factory = || simple(&s, objective, &fail);
    let result = solve_mixed_calculator(
        &p,
        &factory,
        MixedSolverSequence {
            phases: vec![MixedSolverPhase::Exact(ExactPhaseConfig::default())],
            ..Default::default()
        },
    )
    .unwrap();
    assert!(result.is_optimal());
    assert_eq!(result.score, Some(SimpleScore::new(-4)));
    assert_eq!(result.assignment.basic().values(), &[Some(1)]);
    assert_eq!(result.assignment.lists().list(0), Some(&[2, 1][..]));
}

#[test]
fn mutation_panic_and_out_of_order_callbacks_invalidate_the_owned_projection() {
    let s = schema::<ListAssignment<i32>>();
    let p = problem();
    let calls = Cell::new(0);
    let fail = Cell::new(0);
    let mut session = CalculatorSession::new(
        p.initial_assignment(),
        list_calculator(&s, &p, &calls, &fail),
        false,
    )
    .unwrap();
    assert!(
        catch_unwind(AssertUnwindSafe(
            || session.change(&vec![0], |_| panic!("mutation"))
        ))
        .is_err()
    );
    assert!(session.calculator().solution().is_err());
    session.reset(p.initial_assignment()).unwrap();
    assert_eq!(session.score().unwrap(), SimpleScore::new(-14));
    let mut raw = list_calculator(&s, &p, &calls, &fail);
    let mut registry = MatchRegistry::new(false);
    raw.reset(&p.initial_assignment(), &mut registry).unwrap();
    assert_eq!(
        raw.after_change(&p.initial_assignment(), &vec![], &mut registry),
        Err(MatchRegistryError::NeedsReset)
    );
    assert!(raw.solution().is_err());
    raw.reset(&p.initial_assignment(), &mut registry).unwrap();
    raw.before_change(&p.initial_assignment(), &vec![], &mut registry)
        .unwrap();
    assert_eq!(
        raw.calculate_score_with_matches(&mut registry),
        Err(MatchRegistryError::NeedsReset)
    );
    assert_eq!(
        raw.after_change(&p.initial_assignment(), &vec![], &mut registry),
        Err(MatchRegistryError::NeedsReset)
    );
    raw.reset(&p.initial_assignment(), &mut registry).unwrap();
    assert_eq!(
        raw.calculate_score_with_matches(&mut registry).unwrap(),
        SimpleScore::new(-14)
    );
}
#[test]
fn tracked_undo_captures_real_nonclone_shadow_projection_in_fresh_search() {
    let s = schema::<ListAssignment<i32>>();
    let p = problem();
    let calls = Cell::new(0);
    let fail = Cell::new(0);
    let captures = Cell::new(0);
    let factory = || {
        list_calculator(&s, &p, &calls, &fail)
            .with_undo_diagnostics(CalculatorUndoDiagnostics::Tracked)
            .with_undo_state_capture(|_, h| {
                captures.set(captures.get() + 1);
                let mut snapshot = CalculatorStateSnapshot::new();
                snapshot.capture(
                    CalculatorUndoField {
                        entity_key: "0".into(),
                        entity_type: "Entity".into(),
                        entity_description: "entity0".into(),
                        variable: "cost".into(),
                    },
                    h.derived.0,
                    h.derived.0.to_string(),
                )?;
                Ok(snapshot)
            })
    };
    let mut provider = |_: &ListProblem<i32>, _: &ListAssignment<i32>, _: u64| vec![relocation()];
    let result = list_calculator_local_search_with_provider(
        &p,
        p.initial_assignment(),
        factory,
        LocalSearchConfig {
            max_steps: 1,
            max_evaluations: 2,
            ..Default::default()
        },
        &mut provider,
    )
    .unwrap();
    assert_eq!(captures.get(), 2);
    assert_eq!(result.evaluations, 2);
}
#[test]
fn fresh_scoring_reads_actual_nonclone_projection_without_running_suppliers() {
    let s = schema::<ListAssignment<i32>>();
    let p = problem();
    let supplier_calls = Cell::new(0);
    let supplier_calls = &supplier_calls;
    for corrupt_scorer in [false, true] {
        let factory = || {
            let entity = s.entity::<Entity>().unwrap();
            let mut b = ShadowSessionBuilder::<usize, ListAssignment<i32>>::new(&s);
            let input = b
                .input(entity.variable("world").unwrap(), 0, |a| Some(a.clone()))
                .unwrap();
            let source = input.clone();
            let history = Cell::new(0_i64);
            let cost = b
                .derive(entity.variable("cost").unwrap(), 0, move |_, i| {
                    supplier_calls.set(supplier_calls.get() + 1);
                    history.set(history.get() + 1);
                    Some(list_cost(i.get(&source).unwrap().unwrap()) + history.get())
                })
                .unwrap();
            b.set_dependencies(&cost, &[input.erase()]).unwrap();
            let score_calls = Cell::new(0_i64);
            let manager = list_declarative_solution_manager(
                &p,
                b.build(),
                move |_, g| Ok::<_, &'static str>(Cost(*g.read(&cost).unwrap().value.unwrap())),
                |_, _| Ok(()),
                move |_, h| {
                    score_calls.set(score_calls.get() + 1);
                    Ok(SimpleScore::new(
                        h.derived.0 + if corrupt_scorer { score_calls.get() } else { 0 },
                    ))
                },
            );
            ShadowCalculator::new(manager, || ListDerivedShadows {
                builtin: vec![],
                derived: Cost(0),
            })
            .with_live_shadow_score_recomputation()
        };
        let mut d =
            ListCalculatorScoreDirector::new(&p, p.initial_assignment(), factory, false).unwrap();
        if !corrupt_scorer {
            d.apply(&relocation()).unwrap().commit();
            assert_eq!(d.score().unwrap(), SimpleScore::new(-6));
        }
        let before = supplier_calls.get();
        let result = d.assert_consistent();
        assert_eq!(
            supplier_calls.get(),
            before,
            "fresh scoring must not reset/refresh shadows"
        );
        if corrupt_scorer {
            assert!(
                matches!(result, Err(ListScoringError::ScoreCorruption(_))),
                "{result:?}"
            );
        } else {
            result.unwrap();
            assert_eq!(
                d.calculator().unwrap().solution().unwrap().shadows.derived,
                Cost(-6)
            );
        }
    }
}
