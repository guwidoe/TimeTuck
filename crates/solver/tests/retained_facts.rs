//! Original dynamic-fact integration checks, not adapted upstream methods.
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;

struct Graph {
    graph: StreamGraph<'static, SimpleScore>,
    planning: MixedStreamSources<i32, i32>,
    costs: StreamSource<i32, (i32, i64)>,
    labels: StreamSource<usize, String>,
    fixed_calls: Rc<Cell<usize>>,
}
fn graph(costs: &[(i32, i64)]) -> Graph {
    let mut b = StreamGraphBuilder::new();
    let planning = MixedStreamSources::<i32, i32>::new(&mut b);
    let source = b.source::<i32, (i32, i64)>();
    let basic = b
        .join_batched(
            planning.basic.port(),
            source.port(),
            |(_, v)| *v,
            |(v, _)| Some(*v),
            |_, _| true,
        )
        .unwrap();
    b.score("basic", &basic, |(_, (_, cost))| SimpleScore::new(*cost))
        .unwrap();
    let lists = b
        .join_batched(
            planning.lists.values.port(),
            source.port(),
            |r| r.value,
            |(v, _)| *v,
            |_, _| true,
        )
        .unwrap();
    b.score("lists", &lists, |(r, (_, cost))| {
        SimpleScore::new(r.position.map_or(0, |p| (p.index + 1) as i64 * cost))
    })
    .unwrap();
    let labels = b.source::<usize, String>();
    b.score("labels", labels.port(), |label| {
        assert_ne!(label, "panic");
        SimpleScore::new(label.len() as i64)
    })
    .unwrap();
    let fixed = b.source::<usize, i64>();
    let fixed_calls = Rc::new(Cell::new(0));
    let calls = fixed_calls.clone();
    b.score("fixed", fixed.port(), move |v| {
        calls.set(calls.get() + 1);
        SimpleScore::new(*v)
    })
    .unwrap();
    let mut graph = b.build();
    let mut tx = graph.transaction();
    for &(id, value) in costs {
        tx.apply(&source, TupleChange::put(id, (id, value)))
            .unwrap();
    }
    tx.apply(&fixed, TupleChange::put(0, 7)).unwrap();
    tx.commit().unwrap();
    Graph {
        graph,
        planning,
        costs: source,
        labels,
        fixed_calls,
    }
}
fn basic() -> FiniteProblem<i32> {
    FiniteProblem::new(vec![
        Variable::new(vec![1, 2]).with_initial(1),
        Variable::new(vec![1, 2]),
    ])
    .unwrap()
}
fn lists() -> ListProblem<i32> {
    ListProblem::new(vec![1, 2, 3], vec![ListEntity::new(vec![1, 2])]).unwrap()
}
fn change(
    tx: &mut FactTransaction<'_, '_, SimpleScore>,
    costs: &StreamSource<i32, (i32, i64)>,
    labels: &StreamSource<usize, String>,
    value: i64,
) {
    let mut batch = StreamBatch::new();
    batch.push(costs, TupleChange::put(1, (1, value))).unwrap();
    batch
        .push(labels, TupleChange::put(0, "new".to_owned()))
        .unwrap();
    tx.apply_batch(batch).unwrap();
}
#[test]
fn all_models_update_joined_facts_atomically_without_replaying_unrelated_sources() {
    let bp = basic();
    let lp = lists();
    let mp = MixedProblem::new(bp.clone(), lp.clone());
    // One partial basic variable and one partial list value preserve structural scores.
    macro_rules! check {
        ($director:ident, $problem:expr, $source:expr, $g:ident, $expected:expr) => {{
            let mut d =
                $director::new($problem, $g.graph, $source, $problem.initial_assignment()).unwrap();
            let original = d.score();
            let assignment = d.assignment().clone();
            for value in [-30, 0, 20] {
                let mut tx = d.edit_facts();
                change(&mut tx, &$g.costs, &$g.labels, value);
                assert_eq!(tx.score().unwrap(), $expected(value));
                assert_eq!($g.fixed_calls.get(), 1, "unrelated source was replayed");
                drop(tx);
                assert_eq!(d.score(), original);
                assert_eq!(d.assignment(), &assignment);
                assert_eq!($g.fixed_calls.get(), 1, "rollback replayed a callback");
            }
            let mut tx = d.edit_facts();
            change(&mut tx, &$g.costs, &$g.labels, 20);
            tx.commit().unwrap();
            assert_eq!(d.score(), $expected(20));
            assert_eq!(d.assignment(), &assignment);
            assert_eq!($g.fixed_calls.get(), 1);
            d.assert_consistent().unwrap();
            let mut tx = d.edit_facts();
            tx.apply(&$g.costs, TupleChange::retract(1)).unwrap();
            tx.apply(&$g.labels, TupleChange::retract(0)).unwrap();
            tx.commit().unwrap();
            assert_eq!(
                d.score(),
                ($expected(0) - SimpleScore::new(3)).with_structural(original.structural())
            );
        }};
    }
    // Retractions preserve structure too; the expected subtraction restores it.
    let g = graph(&[(1, 10), (2, -2)]);
    check!(StreamScoreDirector, &bp, g.planning.basic, g, |v: i64| {
        SimpleScore::new(10 + v).with_structural(-1)
    });
    let g = graph(&[(1, 10), (2, -2)]);
    check!(
        ListStreamScoreDirector,
        &lp,
        g.planning.lists,
        g,
        |v: i64| { SimpleScore::new(6 + v).with_structural(-1) }
    );
    let g = graph(&[(1, 10), (2, -2)]);
    check!(MixedStreamScoreDirector, &mp, g.planning, g, |v: i64| {
        SimpleScore::new(6 + 2 * v).with_structural(-2)
    });
}

#[test]
fn protected_planning_sources_and_foreign_handles_poison_whole_fact_transaction() {
    let p = MixedProblem::new(basic(), lists());
    let g = graph(&[(1, 10), (2, -2)]);
    let s = g.planning.clone();
    let mut d =
        MixedStreamScoreDirector::new(&p, g.graph, g.planning, p.initial_assignment()).unwrap();
    let before = d.score();
    for source in 0..3 {
        let mut tx = d.edit_facts();
        tx.apply(&g.costs, TupleChange::put(1, (1, 100))).unwrap();
        let mut batch = StreamBatch::new();
        batch
            .push(&g.labels, TupleChange::put(0, "valid".to_owned()))
            .unwrap();
        match source {
            0 => batch.push(&s.basic, TupleChange::retract(0)).unwrap(),
            1 => batch
                .push(&s.lists.entities, TupleChange::retract(0))
                .unwrap(),
            _ => batch
                .push(&s.lists.values, TupleChange::retract(0))
                .unwrap(),
        }
        assert_eq!(
            tx.apply_batch(batch),
            Err(GraphError::ProtectedPlanningSource)
        );
        assert!(matches!(tx.graph(), Err(GraphError::FailedTransaction)));
        assert_eq!(tx.commit(), Err(GraphError::FailedTransaction));
        assert_eq!(d.score(), before);
    }
    let mut tx = d.edit_facts();
    assert_eq!(
        tx.apply(&s.basic, TupleChange::retract(0)),
        Err(GraphError::ProtectedPlanningSource)
    );
    assert_eq!(tx.score(), Err(GraphError::FailedTransaction));
    drop(tx);
    let other = graph(&[]);
    for batch in [false, true] {
        let mut tx = d.edit_facts();
        tx.apply(&g.costs, TupleChange::put(1, (1, 100))).unwrap();
        let result = if batch {
            let mut changes = StreamBatch::new();
            changes.push(&other.costs, TupleChange::retract(1)).unwrap();
            tx.apply_batch(changes)
        } else {
            tx.apply(&other.costs, TupleChange::retract(1))
        };
        assert_eq!(result, Err(GraphError::ForeignPort));
        assert_eq!(tx.commit(), Err(GraphError::FailedTransaction));
        assert_eq!(d.score(), before);
    }
    d.assert_consistent().unwrap();
}

#[test]
fn caught_callback_panic_rolls_back_prior_fact_changes_and_allows_next_transaction() {
    let p = basic();
    let g = graph(&[(1, 10)]);
    let mut d =
        StreamScoreDirector::new(&p, g.graph, g.planning.basic, p.initial_assignment()).unwrap();
    let original = d.score();
    let mut tx = d.edit_facts();
    tx.apply(&g.costs, TupleChange::put(1, (1, 100))).unwrap();
    assert!(
        catch_unwind(AssertUnwindSafe(
            || tx.apply(&g.labels, TupleChange::put(0, "panic".to_owned()))
        ))
        .is_err()
    );
    assert_eq!(tx.commit(), Err(GraphError::FailedTransaction));
    assert_eq!(d.score(), original);
    let mut tx = d.edit_facts();
    change(&mut tx, &g.costs, &g.labels, 20);
    tx.commit().unwrap();
    assert_eq!(d.score(), SimpleScore::new(30).with_structural(-1));
    d.assert_consistent().unwrap();
}

#[test]
fn subsequent_solves_use_new_fact_objective_on_all_models() {
    let bp = basic();
    let lp = lists();
    let mp = MixedProblem::new(bp.clone(), lp.clone());
    macro_rules! check {
        ($director:ident, $problem:expr, $source:expr, $g:ident, $config:expr, $fresh_source:expr, $fresh:ident) => {{
            let mut d =
                $director::new($problem, $g.graph, $source, $problem.initial_assignment()).unwrap();
            d.run_solver($config).unwrap();
            for weights in [[(1, -20), (2, 30)], [(1, 50), (2, -10)]] {
                let mut tx = d.edit_facts();
                for (id, value) in weights {
                    tx.apply(&$g.costs, TupleChange::put(id, (id, value)))
                        .unwrap();
                }
                tx.commit().unwrap();
                let $fresh = graph(&weights);
                let mut fresh = $director::new(
                    $problem,
                    $fresh.graph,
                    $fresh_source,
                    d.assignment().clone(),
                )
                .unwrap();
                let expected = fresh.run_solver($config).unwrap();
                let actual = d.run_solver($config).unwrap();
                assert_eq!(actual.assignment, expected.assignment);
                assert_eq!(actual.score, expected.score);
                assert_eq!(actual.work.evaluations, expected.work.evaluations);
                assert_eq!(d.score(), fresh.score());
                d.assert_consistent().unwrap();
            }
        }};
    }
    let config = || SolverConfig {
        local_search: vec![LocalSearchConfig {
            max_steps: 20,
            ..Default::default()
        }],
        ..Default::default()
    };
    let g = graph(&[(1, 10), (2, -2)]);
    check!(
        StreamScoreDirector,
        &bp,
        g.planning.basic,
        g,
        config(),
        fresh.planning.basic,
        fresh
    );
    let g = graph(&[(1, 10), (2, -2)]);
    check!(
        ListStreamScoreDirector,
        &lp,
        g.planning.lists,
        g,
        config(),
        fresh.planning.lists,
        fresh
    );
    let g = graph(&[(1, 10), (2, -2)]);
    check!(
        MixedStreamScoreDirector,
        &mp,
        g.planning,
        g,
        MixedSolverConfig {
            local_search: vec![MixedLocalSearchConfig {
                search: LocalSearchConfig {
                    max_steps: 20,
                    ..Default::default()
                },
                ..Default::default()
            }],
            ..Default::default()
        },
        fresh.planning,
        fresh
    );
}

#[test]
fn basic_and_list_directors_protect_their_bound_sources() {
    let p = basic();
    let g = graph(&[]);
    let source = g.planning.basic.clone();
    let mut d =
        StreamScoreDirector::new(&p, g.graph, g.planning.basic, p.initial_assignment()).unwrap();
    let mut tx = d.edit_facts();
    assert_eq!(
        tx.apply(&source, TupleChange::retract(0)),
        Err(GraphError::ProtectedPlanningSource)
    );
    assert_eq!(tx.commit(), Err(GraphError::FailedTransaction));
    d.assert_consistent().unwrap();
    let p = lists();
    let g = graph(&[]);
    let sources = g.planning.lists.clone();
    let mut d = ListStreamScoreDirector::new(&p, g.graph, g.planning.lists, p.initial_assignment())
        .unwrap();
    for entities in [false, true] {
        let mut tx = d.edit_facts();
        let result = if entities {
            tx.apply(&sources.entities, TupleChange::retract(0))
        } else {
            tx.apply(&sources.values, TupleChange::retract(0))
        };
        assert_eq!(result, Err(GraphError::ProtectedPlanningSource));
        assert_eq!(tx.commit(), Err(GraphError::FailedTransaction));
    }
    d.assert_consistent().unwrap();
}

#[test]
fn overflow_rolls_back_multi_source_edits_without_callback_replay() {
    let p = basic();
    let g = graph(&[(1, 10)]);
    let mut d =
        StreamScoreDirector::new(&p, g.graph, g.planning.basic, p.initial_assignment()).unwrap();
    let before = d.score();
    let mut tx = d.edit_facts();
    tx.apply(&g.labels, TupleChange::put(0, "changed".to_owned()))
        .unwrap();
    assert!(matches!(
        tx.apply(&g.costs, TupleChange::put(1, (1, i64::MAX))),
        Err(GraphError::ScoreOverflow { .. })
    ));
    assert_eq!(tx.commit(), Err(GraphError::FailedTransaction));
    assert_eq!(d.score(), before);
    assert_eq!(g.fixed_calls.get(), 1);
    assert!(d.graph().source_rows(&g.labels).unwrap().is_empty());
    d.assert_consistent().unwrap();
}
