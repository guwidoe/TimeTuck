//! Original opt-in notification/search, lifecycle, and callback work regressions.
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;
struct Model;
struct Entity;
type Event = (Vec<Option<i64>>, Vec<Option<i64>>, Vec<usize>);
#[derive(Default)]
struct Stats {
    reads: Cell<usize>,
    resolves: Cell<usize>,
    supplies: Cell<usize>,
    scores: Cell<u64>,
    events: RefCell<Vec<Event>>,
}
fn schema() -> PlanningSchema {
    PlanningSchema::new::<Model>(vec![PlanningEntityDefinition::of::<Entity>(vec![
        PlanningVariableDefinition::basic::<i64>("input", true),
        PlanningVariableDefinition::shadow::<i64>("cost"),
    ])])
    .unwrap()
}
fn calculator<'a, A: Clone, C>(
    schema: &'a PlanningSchema,
    count: usize,
    read: impl Fn(&A, usize) -> Option<i64> + Copy + 'a,
    affected: impl Fn(&C) -> Vec<usize> + 'a,
    stats: Rc<Stats>,
    notified: bool,
) -> ShadowCalculator<'a, 'a, usize, A, Vec<i64>, SimpleScore, &'static str, C> {
    let entity = schema.entity::<Entity>().unwrap();
    let mut builder = ShadowSessionBuilder::<usize, A>::new(schema);
    let inputs: Vec<_> = (0..count)
        .map(|i| {
            let stats = stats.clone();
            builder
                .input(entity.variable("input").unwrap(), i, move |a| {
                    stats.reads.set(stats.reads.get() + 1);
                    read(a, i)
                })
                .unwrap()
        })
        .collect();
    let costs: Vec<_> = inputs
        .iter()
        .enumerate()
        .map(|(i, source)| {
            let source = source.clone();
            let stats = stats.clone();
            builder
                .derive(entity.variable("cost").unwrap(), i, move |_, values| {
                    stats.supplies.set(stats.supplies.get() + 1);
                    Some(-values.get(&source).unwrap().copied().unwrap_or(0).pow(2))
                })
                .unwrap()
        })
        .collect();
    for (input, cost) in inputs.iter().zip(&costs) {
        let input = input.erase();
        let stats = stats.clone();
        builder
            .bind_dependencies(cost, move |_| {
                stats.resolves.set(stats.resolves.get() + 1);
                Ok(vec![input.clone()])
            })
            .unwrap();
    }
    let scored = stats.clone();
    let manager = ShadowSolutionManager::new(
        builder.build(),
        move |_, session| {
            Ok(costs
                .iter()
                .map(|cost| *session.read(cost).unwrap().value.unwrap())
                .collect::<Vec<_>>())
        },
        |_, _| Ok(()),
        move |_, values| {
            scored.scores.set(scored.scores.get() + 1);
            Ok(SimpleScore::new(values.iter().sum()))
        },
    );
    let calc = ShadowCalculator::new(manager, Vec::new);
    if notified {
        calc.with_change_notifier(move |before, after, change, session| {
            let indices = affected(change);
            stats.events.borrow_mut().push((
                (0..count).map(|i| read(before, i)).collect(),
                (0..count).map(|i| read(after, i)).collect(),
                indices.clone(),
            ));
            for i in indices {
                session.mark_dirty(&inputs[i]).map_err(|_| "notification")?;
            }
            Ok(())
        })
    } else {
        calc
    }
}
fn basic(a: &Assignment<i32>, i: usize) -> Option<i64> {
    a.values()[i].map(i64::from)
}
fn lists(a: &ListAssignment<i32>, i: usize) -> Option<i64> {
    Some(a.lists()[i].len() as i64)
}
fn mixed(a: &MixedAssignment<i32, i32>, i: usize) -> Option<i64> {
    if i == 0 {
        basic(a.basic(), 0)
    } else {
        lists(a.lists(), i - 1)
    }
}
#[test]
fn sparse_native_moves_notify_forward_inverse_and_commit_without_full_polling() {
    let schema = schema();
    let problem =
        FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]).with_initial(2); 128]).unwrap();
    let stats = Rc::new(Stats::default());
    let factory = || {
        calculator(
            &schema,
            128,
            basic,
            |c: &Vec<usize>| c.clone(),
            stats.clone(),
            true,
        )
    };
    let mut director =
        CalculatorScoreDirector::new(&problem, problem.initial_assignment(), factory, false)
            .unwrap();
    assert_eq!(
        (
            stats.reads.get(),
            stats.resolves.get(),
            stats.supplies.get()
        ),
        (128, 128, 128)
    );
    stats.reads.set(0);
    stats.resolves.set(0);
    stats.supplies.set(0);
    {
        let trial = director
            .apply(&Move::Change {
                variable: 17,
                value: Some(0),
            })
            .unwrap();
        assert_eq!(trial.score(), SimpleScore::new(-508));
    }
    assert_eq!(director.score().unwrap(), SimpleScore::new(-512));
    assert_eq!(
        (
            stats.reads.get(),
            stats.resolves.get(),
            stats.supplies.get()
        ),
        (2, 0, 2)
    );
    let events = stats.events.borrow();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].0, events[1].1);
    assert_eq!(events[0].1, events[1].0);
    assert_eq!(events[0].2, [17]);
    assert_eq!(events[1].2, [17]);
    drop(events);
    director
        .apply(&Move::Change {
            variable: 18,
            value: Some(1),
        })
        .unwrap()
        .commit();
    assert_eq!(director.score().unwrap(), SimpleScore::new(-509));
    assert_eq!(
        (
            stats.reads.get(),
            stats.resolves.get(),
            stats.supplies.get()
        ),
        (3, 0, 3)
    );
    let notifications = stats.events.borrow().len();
    director.reset(problem.initial_assignment()).unwrap();
    assert_eq!(
        stats.events.borrow().len(),
        notifications,
        "reset does not call notifier"
    );
    assert_eq!(
        (
            stats.reads.get(),
            stats.resolves.get(),
            stats.supplies.get()
        ),
        (131, 128, 131)
    );
    let full_stats = Rc::new(Stats::default());
    let full_factory = || {
        calculator(
            &schema,
            128,
            basic,
            |c: &Vec<usize>| c.clone(),
            full_stats.clone(),
            false,
        )
    };
    let mut full =
        CalculatorScoreDirector::new(&problem, problem.initial_assignment(), full_factory, false)
            .unwrap();
    full_stats.reads.set(0);
    full_stats.resolves.set(0);
    {
        let trial = full
            .apply(&Move::Change {
                variable: 17,
                value: Some(0),
            })
            .unwrap();
        assert_eq!(trial.score(), SimpleScore::new(-508));
    }
    assert_eq!(
        (full_stats.reads.get(), full_stats.resolves.get()),
        (256, 256)
    );
}
#[test]
fn all_three_search_backends_match_polling_and_replay_without_extra_scores() {
    let schema = schema();
    let problem =
        FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]).with_initial(2); 2]).unwrap();
    let stats = Rc::new(Stats::default());
    let factory = || {
        calculator(
            &schema,
            2,
            basic,
            |c: &Vec<usize>| c.clone(),
            stats.clone(),
            true,
        )
    };
    let full = || {
        calculator(
            &schema,
            2,
            basic,
            |c: &Vec<usize>| c.clone(),
            Rc::new(Stats::default()),
            false,
        )
    };
    let config = LocalSearchConfig {
        max_steps: 1,
        max_evaluations: 100,
        ..Default::default()
    };
    let result =
        calculator_local_search_from(&problem, problem.initial_assignment(), factory, config)
            .unwrap();
    let expected =
        calculator_local_search_from(&problem, problem.initial_assignment(), full, config).unwrap();
    assert_eq!(
        result.best.as_ref().unwrap().assignment,
        expected.best.as_ref().unwrap().assignment
    );
    assert_eq!(
        result.best.as_ref().unwrap().score,
        expected.best.as_ref().unwrap().score
    );
    assert_eq!(
        stats.scores.get(),
        result.evaluations,
        "winner replay must refresh without scoring"
    );
    assert!(result.accepted_moves > 0);
    let exact = solve_calculator(
        &problem,
        &factory,
        SolverSequence {
            phases: vec![SolverPhase::Exact(ExactPhaseConfig::default())],
            ..Default::default()
        },
    )
    .unwrap();
    assert!(exact.is_optimal());
    assert_eq!(exact.score, Some(SimpleScore::ZERO));

    let problem = ListProblem::new(
        vec![1, 2],
        vec![ListEntity::new(vec![1, 2]), ListEntity::new(vec![])],
    )
    .unwrap();
    let stats = Rc::new(Stats::default());
    let factory = || {
        calculator(
            &schema,
            2,
            lists,
            |c: &Vec<usize>| c.clone(),
            stats.clone(),
            true,
        )
    };
    let full = || {
        calculator(
            &schema,
            2,
            lists,
            |c: &Vec<usize>| c.clone(),
            Rc::new(Stats::default()),
            false,
        )
    };
    let result =
        list_calculator_local_search_from(&problem, problem.initial_assignment(), factory, config)
            .unwrap();
    let expected =
        list_calculator_local_search_from(&problem, problem.initial_assignment(), full, config)
            .unwrap();
    assert_eq!(
        result.best.as_ref().unwrap().assignment,
        expected.best.as_ref().unwrap().assignment
    );
    assert_eq!(
        result.best.as_ref().unwrap().score,
        expected.best.as_ref().unwrap().score
    );
    assert_eq!(stats.scores.get(), result.evaluations);
    assert!(result.accepted_moves > 0);
    let exact = solve_list_calculator(
        &problem,
        &factory,
        SolverSequence {
            phases: vec![SolverPhase::Exact(ExactPhaseConfig::default())],
            ..Default::default()
        },
    )
    .unwrap();
    assert!(exact.is_optimal());
    assert_eq!(exact.score, Some(SimpleScore::new(-2)));

    let problem = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]).with_initial(2)]).unwrap(),
        problem,
    );
    let stats = Rc::new(Stats::default());
    let affected = |c: &MixedCalculatorChange| {
        c.basic_variables
            .iter()
            .copied()
            .chain(c.list_entities.iter().map(|i| i + 1))
            .collect()
    };
    let factory = || calculator(&schema, 3, mixed, affected, stats.clone(), true);
    let full = || {
        calculator(
            &schema,
            3,
            mixed,
            affected,
            Rc::new(Stats::default()),
            false,
        )
    };
    let config = MixedLocalSearchConfig {
        search: config,
        ..Default::default()
    };
    let result =
        mixed_calculator_local_search_from(&problem, problem.initial_assignment(), factory, config)
            .unwrap();
    let expected =
        mixed_calculator_local_search_from(&problem, problem.initial_assignment(), full, config)
            .unwrap();
    assert_eq!(
        result.best.as_ref().unwrap().assignment,
        expected.best.as_ref().unwrap().assignment
    );
    assert_eq!(
        result.best.as_ref().unwrap().score,
        expected.best.as_ref().unwrap().score
    );
    assert_eq!(stats.scores.get(), result.evaluations);
    let exact = solve_mixed_calculator(
        &problem,
        &factory,
        MixedSolverSequence {
            phases: vec![MixedSolverPhase::Exact(ExactPhaseConfig::default())],
            ..Default::default()
        },
    )
    .unwrap();
    assert!(exact.is_optimal());
    assert_eq!(exact.score, Some(SimpleScore::new(-2)));
}
#[test]
fn notifier_failures_restore_graph_edits_and_full_reset_recovers_external_facts() {
    for panic in [false, true] {
        let schema = schema();
        let entity = schema.entity::<Entity>().unwrap();
        let mut builder = ShadowSessionBuilder::<usize, i64>::new(&schema);
        let out = builder
            .derive(entity.variable("cost").unwrap(), 0, |world, _| Some(*world))
            .unwrap();
        let projected = out.clone();
        let fail = Cell::new(true);
        let manager = ShadowSolutionManager::new(
            builder.build(),
            move |_, s| Ok::<_, &'static str>(*s.read(&projected).unwrap().value.unwrap()),
            |_, _| Ok(()),
            |_, v| Ok(SimpleScore::new(*v)),
        );
        let calls = Cell::new(0);
        let calc = ShadowCalculator::new(manager, || 0).with_change_notifier(
            |before, after, change: &(), session| {
                assert_eq!((*before, *after, *change), (3, 7, ()));
                calls.set(calls.get() + 1);
                if fail.get() {
                    session.set_dependencies(&out, &[out.erase()]).unwrap();
                    session.mark_dirty(&out).unwrap();
                    if panic {
                        panic!("notification panic");
                    }
                    return Err("notification error");
                }
                session.mark_dirty(&out).unwrap();
                Ok(())
            },
        );
        let mut session = CalculatorSession::new(3, calc, false).unwrap();
        assert_eq!(session.score().unwrap(), SimpleScore::new(3));
        let result = catch_unwind(AssertUnwindSafe(|| session.change(&(), |world| *world = 7)));
        if panic {
            assert!(result.is_err());
        } else {
            assert!(result.unwrap().is_err());
        }
        assert!(session.calculator().solution().is_err());
        assert_eq!(session.score(), Err(MatchRegistryError::NeedsReset));
        fail.set(false);
        // Full invalidation re-evaluates the external-fact supplier, and succeeds
        // only if the failed notifier's self-cycle was rolled back.
        session.reset(11).unwrap();
        assert_eq!(session.score().unwrap(), SimpleScore::new(11));
        assert_eq!(calls.get(), 1);
    }
}
