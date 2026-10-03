//! Original regressions; no upstream-method credit.
use std::{
    cell::Cell,
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
#[test]
fn replacements_reorders_empty_and_nullable_rows_keep_occurrence_identity() {
    let calls = Cell::new(0);
    let mut node = FlatMapNode::new(|v: &Vec<Option<i32>>| {
        calls.set(calls.get() + 1);
        v.clone()
    });
    let mut rows = BTreeMap::new();
    for (id, value) in [
        (1, Some(vec![None, Some(2), Some(2)])),
        (2, Some(vec![Some(2)])),
        (1, Some(vec![Some(2), None])),
        (1, Some(vec![])),
        (1, None),
        (1, None),
        (2, None),
    ] {
        for change in node.apply(TupleChange {
            id,
            value: value.clone(),
        }) {
            if let Some(v) = change.value {
                rows.insert(change.id, v);
            } else {
                rows.remove(&change.id);
            }
        }
        match (id, value) {
            (1, Some(v)) if v.len() == 3 => assert_eq!(
                rows,
                BTreeMap::from([((1, 0), None), ((1, 1), Some(2)), ((1, 2), Some(2))])
            ),
            (1, Some(v)) if v.len() == 2 => assert_eq!(
                rows,
                BTreeMap::from([((1, 0), Some(2)), ((1, 1), None), ((2, 0), Some(2))])
            ),
            (1, _) => assert_eq!(rows, BTreeMap::from([((2, 0), Some(2))])),
            (2, None) => assert!(rows.is_empty()),
            _ => assert_eq!(rows.len(), 4),
        }
    }
    assert_eq!(calls.get(), 4);
}
#[test]
fn mapping_and_iterator_panics_leave_standalone_lengths_unchanged() {
    let fault = Cell::new(0);
    let mut node = FlatMapNode::new(|n: &usize| {
        assert_ne!(fault.get(), 1, "mapping panic");
        let fault = &fault;
        (0..*n).inspect(move |&i| {
            assert!(!(fault.get() == 2 && i == 2), "iterator panic");
        })
    });
    node.apply(TupleChange::put(3, 4));
    for mode in [1, 2] {
        fault.set(mode);
        assert!(catch_unwind(AssertUnwindSafe(|| node.apply(TupleChange::put(3, 6)))).is_err());
    }
    // Retraction must not evaluate even a failing mapping.
    assert_eq!(
        node.apply(TupleChange::retract(3)),
        (0..4)
            .map(|i| TupleChange::retract((3, i)))
            .collect::<Vec<_>>()
    );
    fault.set(0);
    assert_eq!(
        node.apply(TupleChange::put(3, 1)),
        vec![TupleChange::put((3, 0), 0)]
    );
}
fn next(seed: &mut u64) -> u64 {
    *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
    *seed >> 32
}
#[test]
fn generated_graph_updates_match_full_multisets_through_commit_undo_and_rebuild() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, Vec<i64>>();
    let expanded = b.flat_map(source.port(), |v| v.clone()).unwrap();
    let raw = b.score("raw", &expanded, |v| SimpleScore::new(*v)).unwrap();
    let distinct = b.distinct(&expanded).unwrap();
    let unique = b
        .score("unique", &distinct, |_| SimpleScore::new(-1))
        .unwrap();
    let nested = b.flat_map(&expanded, |v| [(*v, 0), (*v, 1)]).unwrap();
    let nested = b.score("nested", &nested, |_| SimpleScore::ZERO).unwrap();
    let mut g = b.build();
    let mut input = BTreeMap::<usize, Vec<i64>>::new();
    let mut seed = 20261109;
    for step in 0..2000 {
        let id = (next(&mut seed) % 24) as usize;
        let value = (!next(&mut seed).is_multiple_of(5)).then(|| {
            (0..next(&mut seed) % 8)
                .map(|_| (next(&mut seed) % 9) as i64 - 4)
                .collect::<Vec<_>>()
        });
        let mut candidate = input.clone();
        if let Some(v) = &value {
            candidate.insert(id, v.clone());
        } else {
            candidate.remove(&id);
        }
        let previous = g.constraint_matches(&raw).unwrap().clone();
        let check = |graph: &StreamGraph<'_, SimpleScore>| {
            let expected = candidate
                .iter()
                .flat_map(|(&id, rows)| {
                    rows.iter()
                        .enumerate()
                        .map(move |(i, &v)| ((id, i), (v, SimpleScore::new(v))))
                })
                .collect::<BTreeMap<_, _>>();
            assert_eq!(
                graph.constraint_matches(&raw).unwrap(),
                &expected,
                "step {step}"
            );
            let unique_expected = candidate
                .values()
                .flatten()
                .map(|&v| (v, (v, SimpleScore::new(-1))))
                .collect::<BTreeMap<_, _>>();
            assert_eq!(graph.constraint_matches(&unique).unwrap(), &unique_expected);
            let nested_expected = expected
                .iter()
                .flat_map(|(&(id, pos), &(v, _))| {
                    [
                        (((id, pos), 0), ((v, 0), SimpleScore::ZERO)),
                        (((id, pos), 1), ((v, 1), SimpleScore::ZERO)),
                    ]
                })
                .collect::<BTreeMap<_, _>>();
            assert_eq!(graph.constraint_matches(&nested).unwrap(), &nested_expected);
            assert_eq!(
                graph.score(),
                SimpleScore::new(
                    candidate.values().flatten().sum::<i64>() - unique_expected.len() as i64
                )
            );
        };
        let mut t = g.transaction();
        t.apply(&source, TupleChange { id, value }).unwrap();
        check(t.graph().unwrap());
        if step % 3 == 0 {
            drop(t);
            assert_eq!(g.constraint_matches(&raw).unwrap(), &previous);
        } else {
            t.commit().unwrap();
            input = candidate;
        }
        if step % 29 == 0 {
            g.assert_consistent().unwrap();
        }
    }
    g.assert_consistent().unwrap();
}
#[test]
fn downstream_errors_panics_and_rebuild_failures_restore_lengths_and_matches() {
    let fault = Cell::new(0);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, usize>();
    let expanded = b
        .flat_map(source.port(), |n| {
            assert_ne!(fault.get(), 3, "expansion panic");
            0..*n
        })
        .unwrap();
    let terminal = b
        .score("rows", &expanded, |&v| {
            assert!(!(fault.get() == 2 && v == 2), "weight panic");
            SimpleScore::new(-1).with_structural(if fault.get() == 1 && v == 2 { -1 } else { 0 })
        })
        .unwrap();
    let mut g = b.build();
    let mut t = g.transaction();
    t.apply(&source, TupleChange::put(0, 4)).unwrap();
    t.commit().unwrap();
    let before = g.constraint_matches(&terminal).unwrap().clone();
    for mode in [1, 2, 3] {
        fault.set(mode);
        let result = catch_unwind(AssertUnwindSafe(|| {
            let mut t = g.transaction();
            let error = t.apply(&source, TupleChange::put(0, 6));
            assert!(error.is_err());
            assert!(t.commit().is_err());
        }));
        assert_eq!(result.is_err(), mode != 1);
        fault.set(0);
        assert_eq!(g.constraint_matches(&terminal).unwrap(), &before);
        g.assert_consistent().unwrap();
    }
    fault.set(3);
    assert!(catch_unwind(AssertUnwindSafe(|| g.assert_consistent())).is_err());
    fault.set(0);
    assert_eq!(g.constraint_matches(&terminal).unwrap(), &before);
    let mut t = g.transaction();
    t.apply(&source, TupleChange::retract(0)).unwrap();
    t.commit().unwrap();
    assert!(g.constraint_matches(&terminal).unwrap().is_empty());
    assert_eq!(g.score(), SimpleScore::ZERO);
    g.assert_consistent().unwrap();
}
#[test]
fn foreign_flat_map_port_rejected_before_mapping() {
    let mut a = StreamGraphBuilder::<SimpleScore>::new();
    let source = a.source::<usize, usize>();
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    assert!(matches!(
        b.flat_map(source.port(), |_| -> Vec<()> { panic!("must not run") }),
        Err(GraphError::ForeignPort)
    ));
}
#[test]
fn construction_and_search_use_expanded_demands_with_independent_score() {
    let build = || {
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let source = b.source::<usize, (usize, Option<usize>)>();
        let demands = b
            .flat_map(source.port(), |&(id, value)| {
                value
                    .into_iter()
                    .flat_map(|v| (0..=v).map(move |d| (id, d)))
                    .collect::<Vec<_>>()
            })
            .unwrap();
        b.score("demands", &demands, |_| SimpleScore::new(-1))
            .unwrap();
        (b.build(), source)
    };
    let problem =
        FiniteProblem::new((0..4).map(|_| Variable::new(vec![0, 1, 2])).collect()).unwrap();
    let (g, s) = build();
    let constructed = greedy_stream_construction(
        &problem,
        g,
        s,
        ConstructionConfig {
            assert_scores: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(constructed.complete);
    assert_eq!(constructed.score, Some(SimpleScore::new(-4)));
    let problem = FiniteProblem::new(
        (0..4)
            .map(|_| Variable::new(vec![0, 1, 2]).with_initial(2))
            .collect(),
    )
    .unwrap();
    let (g, s) = build();
    let result = stream_local_search(
        &problem,
        g,
        s,
        LocalSearchConfig {
            max_evaluations: 200,
            max_steps: 20,
            assert_scores: true,
            ..Default::default()
        },
    )
    .unwrap()
    .best
    .unwrap();
    assert!(problem.is_complete(&result.assignment));
    assert_eq!(result.score, SimpleScore::new(-4));
    assert_eq!(
        result.score,
        SimpleScore::new(
            -result
                .assignment
                .values()
                .iter()
                .map(|v| v.unwrap() as i64 + 1)
                .sum::<i64>()
        )
    );
}
