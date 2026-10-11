//! Original phase-reuse checks; no upstream method credit.
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;

fn configs() -> Vec<LocalSearchConfig<'static>> {
    [
        Acceptance::StrictImprovement,
        Acceptance::HillClimbing,
        Acceptance::VariableTabu {
            tenure: 2,
            aspiration: true,
        },
        Acceptance::SimulatedAnnealing {
            starting_temperature: SimpleScore::new(100),
            seed: 7,
            cooling_steps: 30,
        },
    ]
    .into_iter()
    .flat_map(|acceptance| {
        [0, 1, 9, 100].map(|max_evaluations| LocalSearchConfig {
            acceptance,
            max_evaluations,
            max_steps: 6,
            selection: MoveSelection::FirstAccepted,
            ..Default::default()
        })
    })
    .collect()
}
fn basic_graph(calls: &Cell<u128>) -> (StreamGraph<'_, SimpleScore>, PlanningStreamSource<i32>) {
    let mut b = StreamGraphBuilder::new();
    let s = b.source::<usize, (usize, Option<i32>)>();
    b.score("value", s.port(), |(i, v)| {
        calls.set(calls.get() + 1);
        SimpleScore::new((*i as i64 + 1) * i64::from(v.unwrap_or(0)))
    })
    .unwrap();
    (b.build(), s)
}
fn list_graph(calls: &Cell<u128>) -> (StreamGraph<'_, SimpleScore>, ListStreamSources<i32>) {
    let mut b = StreamGraphBuilder::new();
    let s = ListStreamSources::<i32>::new(&mut b);
    b.score("order", s.values.port(), |v| {
        calls.set(calls.get() + 1);
        SimpleScore::new(
            v.position
                .map_or(0, |p| i64::from(v.value) * (1 + p.entity + p.index) as i64),
        )
    })
    .unwrap();
    (b.build(), s)
}
fn mixed_graph(calls: &Cell<u128>) -> (StreamGraph<'_, SimpleScore>, MixedStreamSources<i32, i32>) {
    let mut b = StreamGraphBuilder::new();
    let s = MixedStreamSources::<i32, i32>::new(&mut b);
    let joined = b
        .join_equal(
            s.basic.port(),
            s.lists.entities.port(),
            |(i, _)| *i,
            |(i, _)| *i,
        )
        .unwrap();
    b.score("joint", &joined, |((_, v), (_, list))| {
        calls.set(calls.get() + 1);
        SimpleScore::new(
            i64::from(v.unwrap_or(0)) * list.iter().map(|v| i64::from(*v)).sum::<i64>(),
        )
    })
    .unwrap();
    (b.build(), s)
}
macro_rules! same_search {
    ($d:ident, $actual:ident, $expected:ident) => {
        assert_eq!($actual.best, $expected.best);
        assert_eq!($actual.evaluations, $expected.evaluations);
        assert_eq!($actual.accepted_moves, $expected.accepted_moves);
        assert_eq!($actual.completed_steps, $expected.completed_steps);
        assert_eq!($actual.unimproved_steps, $expected.unimproved_steps);
        assert_eq!($actual.termination, $expected.termination);
        if let Some(best) = &$actual.best {
            assert_eq!($d.assignment(), &best.assignment);
            assert_eq!($d.score(), best.score);
            assert!($actual.constraint_evaluations < $expected.constraint_evaluations);
        }
    };
}
#[test]
fn basic_construction_and_repeated_search_match_fresh_phases_without_reinitialization() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]); 3]).unwrap();
    let calls = Cell::new(0);
    let fresh_calls = Cell::new(0);
    let (g, s) = basic_graph(&calls);
    let mut d = StreamScoreDirector::new(&p, g, s, p.initial_assignment()).unwrap();
    let before = calls.get();
    let actual = d.run_construction(ConstructionConfig::default()).unwrap();
    let (g, s) = basic_graph(&fresh_calls);
    let expected = greedy_stream_construction(&p, g, s, ConstructionConfig::default()).unwrap();
    assert_eq!(actual.assignment, expected.assignment);
    assert_eq!(actual.score, expected.score);
    assert_eq!(actual.evaluations, expected.evaluations);
    assert_eq!(calls.get() - before, actual.constraint_evaluations);
    assert_eq!(
        actual.constraint_evaluations + 3,
        expected.constraint_evaluations
    );
    let mut accepted = 0;
    for c in configs() {
        let initial = d.assignment().clone();
        let before = calls.get();
        let (g, s) = basic_graph(&fresh_calls);
        let expected = stream_local_search_from(&p, g, s, initial, c.clone()).unwrap();
        let actual = d.run_local_search(c).unwrap();
        same_search!(d, actual, expected);
        assert_eq!(calls.get() - before, actual.constraint_evaluations);
        accepted += actual.accepted_moves;
    }
    assert!(accepted > 0);
    d.assert_consistent().unwrap();
}
#[test]
fn list_phases_preserve_sources_and_best_shadow_rows() {
    let p = ListProblem::new(
        vec![1, 2, 3, 4],
        vec![
            ListEntity::new(vec![1]).with_pinned_prefix(1),
            ListEntity::new(vec![]),
        ],
    )
    .unwrap();
    let calls = Cell::new(0);
    let fresh_calls = Cell::new(0);
    let (g, s) = list_graph(&calls);
    let inspect = s.clone();
    let mut d = ListStreamScoreDirector::new(&p, g, s, p.initial_assignment()).unwrap();
    let before = calls.get();
    let actual = d.run_construction(ConstructionConfig::default()).unwrap();
    let (g, s) = list_graph(&fresh_calls);
    let expected =
        greedy_list_stream_construction(&p, g, s, ConstructionConfig::default()).unwrap();
    assert_eq!(actual.assignment, expected.assignment);
    assert_eq!(actual.score, expected.score);
    assert_eq!(actual.evaluations, expected.evaluations);
    assert_eq!(calls.get() - before, actual.constraint_evaluations);
    assert_eq!(
        actual.constraint_evaluations + 4,
        expected.constraint_evaluations
    );
    for c in configs() {
        let initial = d.assignment().clone();
        let before = calls.get();
        let (g, s) = list_graph(&fresh_calls);
        let expected = list_stream_local_search_from(&p, g, s, initial, c.clone()).unwrap();
        let actual = d.run_local_search(c).unwrap();
        same_search!(d, actual, expected);
        assert_eq!(calls.get() - before, actual.constraint_evaluations);
        for row in d.graph().source_rows(&inspect.values).unwrap().values() {
            let element = d.assignment().element(&row.value).unwrap();
            assert_eq!(row.position, Some(element.position));
            assert_eq!(row.previous, element.previous.copied());
            assert_eq!(row.next, element.next.copied());
        }
    }
    d.assert_consistent().unwrap();
}
#[test]
fn mixed_phases_reuse_joined_graph_and_restore_best_between_searches() {
    let p = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]); 2]).unwrap(),
        ListProblem::new(
            vec![1, 2, 3],
            vec![ListEntity::new(vec![]), ListEntity::new(vec![])],
        )
        .unwrap(),
    );
    let calls = Cell::new(0);
    let fresh_calls = Cell::new(0);
    let (g, s) = mixed_graph(&calls);
    let mut d = MixedStreamScoreDirector::new(&p, g, s, p.initial_assignment()).unwrap();
    let before = calls.get();
    let actual = d
        .run_construction(MixedConstructionConfig::default())
        .unwrap();
    let (g, s) = mixed_graph(&fresh_calls);
    let expected =
        greedy_mixed_stream_construction(&p, g, s, MixedConstructionConfig::default()).unwrap();
    assert_eq!(actual.assignment, expected.assignment);
    assert_eq!(actual.score, expected.score);
    assert_eq!(actual.evaluations, expected.evaluations);
    assert_eq!(calls.get() - before, actual.constraint_evaluations);
    assert_eq!(
        actual.constraint_evaluations + 2,
        expected.constraint_evaluations
    );
    for search in configs() {
        let c = MixedLocalSearchConfig {
            search,
            joint_moves: true,
        };
        let initial = d.assignment().clone();
        let before = calls.get();
        let (g, s) = mixed_graph(&fresh_calls);
        let expected = mixed_stream_local_search_from(&p, g, s, initial, c.clone()).unwrap();
        let actual = d.run_local_search(c).unwrap();
        same_search!(d, actual, expected);
        assert_eq!(calls.get() - before, actual.constraint_evaluations);
    }
    d.assert_consistent().unwrap();
}
#[test]
fn cancellation_errors_and_callback_panics_leave_a_reusable_director() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]).with_initial(0)]).unwrap();
    let mode = Cell::new(0);
    let calls = Cell::new(0);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, (usize, Option<i32>)>();
    b.score("score", s.port(), |(_, v)| {
        calls.set(calls.get() + 1);
        if *v == Some(2) {
            match mode.get() {
                1 => return SimpleScore::ZERO.with_structural(-1),
                2 => panic!("callback"),
                _ => (),
            }
        }
        SimpleScore::new(i64::from(v.unwrap_or(0)))
    })
    .unwrap();
    let mut d = StreamScoreDirector::new(&p, b.build(), s, p.initial_assignment()).unwrap();
    let token = CancellationToken::new();
    token.cancel();
    let before = calls.get();
    assert_eq!(
        d.run_construction(ConstructionConfig {
            cancellation: Some(&token),
            ..Default::default()
        })
        .unwrap()
        .evaluations,
        0
    );
    assert_eq!(
        d.run_local_search(LocalSearchConfig {
            cancellation: Some(&token),
            ..Default::default()
        })
        .unwrap()
        .evaluations,
        0
    );
    assert_eq!(calls.get(), before);
    mode.set(1);
    assert!(d.run_local_search(LocalSearchConfig::default()).is_err());
    mode.set(2);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = d.run_local_search(LocalSearchConfig::default());
        }))
        .is_err()
    );
    assert_eq!(d.assignment().values(), &[Some(1)]);
    assert_eq!(d.score(), SimpleScore::new(1));
    mode.set(0);
    let result = d.run_local_search(LocalSearchConfig::default()).unwrap();
    assert_eq!(result.best.unwrap().score, SimpleScore::new(2));
    d.assert_consistent().unwrap();
}

#[test]
fn accepted_worse_step_restores_best_cache_without_replaying_facts_or_scores() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 2]).with_initial(2)]).unwrap();
    let facts = Cell::new(0);
    let planning = Cell::new(0);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, (usize, Option<i32>)>();
    let f = b.source::<usize, i64>();
    b.score("fact", f.port(), |v| {
        facts.set(facts.get() + 1);
        SimpleScore::new(*v)
    })
    .unwrap();
    b.score("planning", s.port(), |(_, v)| {
        planning.set(planning.get() + 1);
        SimpleScore::new(i64::from(v.unwrap_or(0)))
    })
    .unwrap();
    let mut g = b.build();
    let mut tx = g.transaction();
    tx.apply(&f, TupleChange::put(0, 10)).unwrap();
    tx.commit().unwrap();
    let inspect = s.clone();
    let mut d = StreamScoreDirector::new(&p, g, s, p.initial_assignment()).unwrap();
    for _ in 0..8 {
        let before = planning.get();
        let result = d
            .run_local_search(LocalSearchConfig {
                max_steps: 1,
                max_evaluations: 2,
                selection: MoveSelection::FirstAccepted,
                acceptance: Acceptance::SimulatedAnnealing {
                    starting_temperature: SimpleScore::new(1000),
                    seed: 7,
                    cooling_steps: 10,
                },
                ..Default::default()
            })
            .unwrap();
        assert_eq!(result.accepted_moves, 1);
        assert_eq!(result.completed_steps, 1);
        assert_eq!(result.constraint_evaluations, 1);
        assert_eq!(planning.get() - before, 1);
        assert_eq!(facts.get(), 1);
        assert_eq!(d.score(), SimpleScore::new(12));
        assert_eq!(d.assignment().values(), &[Some(2)]);
        assert_eq!(d.graph().source_rows(&inspect).unwrap()[&0], (0, Some(2)));
    }
    d.assert_consistent().unwrap();
}
