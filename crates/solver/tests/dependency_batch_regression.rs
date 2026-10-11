//! Original final-state dependency batching regressions; no upstream method credit.
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;
type Row = (Option<usize>, i64);
fn compute(_: &usize, row: &Row, inputs: &[(&usize, &DerivedValue<i64>)]) -> Option<i64> {
    Some(row.1 + inputs.first().and_then(|(_, v)| v.value).unwrap_or(0))
}
fn contents(
    node: &DependencyNode<'_, usize, Row, i64>,
) -> BTreeMap<usize, (Row, DerivedValue<i64>)> {
    node.rows()
        .map(|(&k, (r, v))| (k, (*r, v.clone())))
        .collect()
}
#[test]
fn coordinated_rewires_evaluate_final_graph_and_propagate_to_unedited_rows() {
    let calls = RefCell::new(vec![]);
    let mut node = DependencyNode::new(
        0..5,
        |r: &Row| r.0.into_iter().collect(),
        |k, r, inputs| {
            calls.borrow_mut().push(*k);
            compute(k, r, inputs)
        },
    )
    .unwrap();
    node.apply_batch([
        TupleChange::put(0, (None, 1)),
        TupleChange::put(1, (Some(0), 2)),
        TupleChange::put(2, (Some(1), 4)),
        TupleChange::put(3, (None, 9)),
    ])
    .unwrap();
    assert_eq!(*calls.borrow(), [0, 1, 2, 3]);
    calls.borrow_mut().clear();
    let output = node
        .apply_batch([
            TupleChange::put(0, (Some(1), 1)),
            TupleChange::put(1, (None, 2)),
        ])
        .unwrap();
    assert_eq!(*calls.borrow(), [1, 0, 2]);
    assert_eq!(output.iter().map(|c| c.id).collect::<Vec<_>>(), [0, 1, 2]);
    assert_eq!(contents(&node)[&0].1.value, Some(3));
    assert_eq!(contents(&node)[&2].1.value, Some(6));
    assert!(contents(&node).values().all(|(_, v)| !v.inconsistent));
    calls.borrow_mut().clear();
    // A real final cycle still invalidates all its downstream consumers.
    let output = node
        .apply_batch([TupleChange::put(1, (Some(0), 2))])
        .unwrap();
    assert!(calls.borrow().is_empty());
    assert_eq!(output.iter().map(|c| c.id).collect::<Vec<_>>(), [0, 1, 2]);
    assert!(
        output
            .iter()
            .all(|c| c.value.as_ref().unwrap().1.inconsistent)
    );
    let output = node.apply_batch([TupleChange::retract(1)]).unwrap();
    assert_eq!(*calls.borrow(), [0, 2]);
    assert_eq!(output.iter().map(|c| c.id).collect::<Vec<_>>(), [0, 1, 2]);
    assert_eq!(output[1], TupleChange::retract(1));
    assert_eq!(contents(&node)[&0].1.value, Some(1));
}
#[test]
fn last_rows_win_and_absent_retractions_skip_dependencies_and_suppliers() {
    let dependencies = RefCell::new(vec![]);
    let calls = RefCell::new(vec![]);
    let mut node = DependencyNode::new(
        0..4,
        |r: &Row| {
            dependencies.borrow_mut().push(r.1);
            r.0.into_iter().collect()
        },
        |k, r, inputs| {
            calls.borrow_mut().push(*k);
            compute(k, r, inputs)
        },
    )
    .unwrap();
    assert!(node.apply_batch([]).unwrap().is_empty());
    assert!(
        node.apply_batch([TupleChange::put(3, (Some(99), 99)), TupleChange::retract(3)])
            .unwrap()
            .is_empty()
    );
    assert!(dependencies.borrow().is_empty());
    let output = node
        .apply_batch([
            TupleChange::put(0, (Some(99), 99)),
            TupleChange::put(1, (None, 2)),
            TupleChange::put(0, (None, 1)),
            TupleChange::retract(2),
            TupleChange::put(0, (None, 3)),
        ])
        .unwrap();
    assert_eq!(*dependencies.borrow(), [3, 2]);
    assert_eq!(*calls.borrow(), [0, 1]);
    assert_eq!(output.iter().map(|c| c.id).collect::<Vec<_>>(), [0, 1]);
    dependencies.borrow_mut().clear();
    calls.borrow_mut().clear();
    let output = node
        .apply_batch([
            TupleChange::retract(0),
            TupleChange::put(0, (None, 3)),
            TupleChange::retract(1),
        ])
        .unwrap();
    assert_eq!(output.iter().map(|c| c.id).collect::<Vec<_>>(), [0, 1]);
    assert_eq!(*dependencies.borrow(), [3]);
    assert_eq!(*calls.borrow(), [0]);
    assert_eq!(output[1], TupleChange::retract(1));
}
#[test]
fn metadata_errors_and_callback_panics_preserve_the_entire_node() {
    let dependencies = Cell::new(0);
    let suppliers = Cell::new(0);
    let panic_dependency = Cell::new(false);
    let panic_supplier = Cell::new(false);
    let mut node = DependencyNode::new(
        0..3,
        |r: &Row| {
            dependencies.set(dependencies.get() + 1);
            assert!(!panic_dependency.get() || r.1 != 7, "dependency panic");
            r.0.into_iter().collect()
        },
        |k, r, inputs| {
            suppliers.set(suppliers.get() + 1);
            assert!(!panic_supplier.get() || *k != 1, "supplier panic");
            compute(k, r, inputs)
        },
    )
    .unwrap();
    node.apply_batch([
        TupleChange::put(0, (None, 1)),
        TupleChange::put(1, (Some(0), 2)),
    ])
    .unwrap();
    let before = contents(&node);
    dependencies.set(0);
    suppliers.set(0);
    assert_eq!(
        node.apply_batch([TupleChange::put(0, (None, 3)), TupleChange::retract(99)]),
        Err(DependencyError::UnknownNode(99))
    );
    assert_eq!(dependencies.get(), 0);
    assert_eq!(suppliers.get(), 0);
    assert_eq!(contents(&node), before);
    assert_eq!(
        node.apply_batch([
            TupleChange::put(0, (None, 3)),
            TupleChange::put(1, (Some(99), 7))
        ]),
        Err(DependencyError::UnknownDependency {
            node: 1,
            dependency: 99
        })
    );
    assert_eq!(suppliers.get(), 0);
    assert_eq!(contents(&node), before);
    panic_dependency.set(true);
    assert!(
        catch_unwind(AssertUnwindSafe(|| node.apply_batch([
            TupleChange::put(0, (None, 3)),
            TupleChange::put(1, (None, 7))
        ])))
        .is_err()
    );
    assert_eq!(contents(&node), before);
    panic_dependency.set(false);
    panic_supplier.set(true);
    assert!(
        catch_unwind(AssertUnwindSafe(|| node.apply_batch([
            TupleChange::put(0, (None, 3)),
            TupleChange::put(1, (Some(0), 7))
        ])))
        .is_err()
    );
    assert_eq!(contents(&node), before);
    panic_supplier.set(false);
    node.apply_batch([
        TupleChange::put(0, (Some(1), 3)),
        TupleChange::put(1, (None, 7)),
    ])
    .unwrap();
    assert_eq!(contents(&node)[&0].1.value, Some(10));
}

fn stream_batch(
    source: &StreamSource<usize, Row>,
    changes: impl IntoIterator<Item = TupleChange<usize, Row>>,
) -> StreamBatch {
    let mut batch = StreamBatch::new();
    for change in changes {
        batch.push(source, change).unwrap();
    }
    batch
}
#[test]
fn graph_batch_boundary_preserves_immediate_mode_and_transaction_rollback() {
    for batched in [false, true] {
        let observations = RefCell::new(vec![]);
        let mut builder = StreamGraphBuilder::<SimpleScore>::new();
        let source = builder.source::<usize, Row>();
        let derived = if batched {
            builder.derive_dependencies_batched(
                source.port(),
                0..3,
                |r| r.0.into_iter().collect(),
                compute,
            )
        } else {
            builder.derive_dependencies(source.port(), 0..3, |r| r.0.into_iter().collect(), compute)
        }
        .unwrap();
        let constraint = builder
            .score("derived", &derived, |(_, v)| {
                observations.borrow_mut().push(v.clone());
                SimpleScore::new(v.value.unwrap_or(-100))
            })
            .unwrap();
        let mut graph = builder.build();
        let mut tx = graph.transaction();
        tx.apply_batch(stream_batch(
            &source,
            [
                TupleChange::put(0, (None, 1)),
                TupleChange::put(1, (Some(0), 2)),
                TupleChange::put(2, (Some(1), 4)),
            ],
        ))
        .unwrap();
        tx.commit().unwrap();
        observations.borrow_mut().clear();
        let before = graph.constraint_matches(&constraint).unwrap().clone();
        {
            let mut tx = graph.transaction();
            tx.apply_batch(stream_batch(
                &source,
                [
                    TupleChange::put(0, (Some(1), 1)),
                    TupleChange::put(1, (None, 2)),
                ],
            ))
            .unwrap();
            assert_eq!(tx.score().unwrap(), SimpleScore::new(11));
            assert_eq!(
                observations.borrow().iter().any(|v| v.inconsistent),
                !batched
            );
            if batched {
                assert_eq!(observations.borrow().len(), 3);
            }
        }
        assert_eq!(graph.constraint_matches(&constraint).unwrap(), &before);
        observations.borrow_mut().clear();
        // Even the opted-in builder retains direct apply's immediate cycle.
        let mut tx = graph.transaction();
        tx.apply(&source, TupleChange::put(0, (Some(1), 1)))
            .unwrap();
        assert!(observations.borrow().iter().all(|v| v.inconsistent));
        assert_eq!(observations.borrow().len(), 3);
        drop(tx);
        assert_eq!(graph.constraint_matches(&constraint).unwrap(), &before);
        graph.assert_consistent().unwrap();
    }
}
#[test]
fn graph_errors_and_supplier_panics_restore_sources_and_cached_edges() {
    let panic_supplier = Cell::new(false);
    let mut builder = StreamGraphBuilder::<SimpleScore>::new();
    let source = builder.source::<usize, Row>();
    let derived = builder
        .derive_dependencies_batched(
            source.port(),
            0..3,
            |r| r.0.into_iter().collect(),
            |k, r, inputs| {
                assert!(!panic_supplier.get() || *k != 1, "supplier panic");
                compute(k, r, inputs)
            },
        )
        .unwrap();
    let constraint = builder
        .score("derived", &derived, |(_, v)| {
            SimpleScore::new(v.value.unwrap_or(0))
        })
        .unwrap();
    let mut graph = builder.build();
    let mut tx = graph.transaction();
    tx.apply_batch(stream_batch(
        &source,
        [
            TupleChange::put(0, (None, 1)),
            TupleChange::put(1, (Some(0), 2)),
        ],
    ))
    .unwrap();
    tx.commit().unwrap();
    let before = graph.constraint_matches(&constraint).unwrap().clone();
    let sources = graph.source_rows(&source).unwrap().clone();
    for bad in [TupleChange::retract(99), TupleChange::put(1, (Some(99), 3))] {
        let mut tx = graph.transaction();
        assert!(matches!(
            tx.apply_batch(stream_batch(
                &source,
                [TupleChange::put(0, (Some(1), 8)), bad]
            )),
            Err(GraphError::Dependency(_))
        ));
        assert_eq!(tx.score(), Err(GraphError::FailedTransaction));
        drop(tx);
        assert_eq!(graph.constraint_matches(&constraint).unwrap(), &before);
        assert_eq!(graph.source_rows(&source).unwrap(), &sources);
    }
    panic_supplier.set(true);
    {
        let mut tx = graph.transaction();
        assert!(
            catch_unwind(AssertUnwindSafe(|| tx.apply_batch(stream_batch(
                &source,
                [
                    TupleChange::put(0, (Some(1), 8)),
                    TupleChange::put(1, (None, 3))
                ]
            ))))
            .is_err()
        );
        assert_eq!(tx.score(), Err(GraphError::FailedTransaction));
    }
    panic_supplier.set(false);
    assert_eq!(graph.constraint_matches(&constraint).unwrap(), &before);
    assert_eq!(graph.source_rows(&source).unwrap(), &sources);
    graph.assert_consistent().unwrap();
}
#[test]
fn observed_batched_dependencies_enable_composite_move_and_edit_session_batching() {
    let problem = FiniteProblem::new(vec![
        Variable::new(vec![0usize, 1, 2]).with_initial(2),
        Variable::new(vec![0, 1, 2]).with_initial(0),
    ])
    .unwrap();
    let calls = Cell::new(0);
    let mut builder = StreamGraphBuilder::<SimpleScore>::new().with_activity_pruning();
    let source = builder.source::<usize, (usize, Option<usize>)>();
    let derived = builder
        .derive_dependencies_batched(
            source.port(),
            0..2,
            |r| r.1.filter(|&p| p < 2).into_iter().collect(),
            |_, r, inputs| {
                calls.set(calls.get() + 1);
                Some(r.0 as i64 + 1 + inputs.first().and_then(|(_, v)| v.value).unwrap_or(0))
            },
        )
        .unwrap();
    builder
        .score("derived", &derived, |(_, v)| {
            assert!(!v.inconsistent, "coordinated move exposed transient cycle");
            SimpleScore::new(v.value.unwrap())
        })
        .unwrap();
    let mut director = StreamScoreDirector::new(
        &problem,
        builder.build(),
        source,
        problem.initial_assignment(),
    )
    .unwrap();
    assert_eq!(director.score(), SimpleScore::new(4));
    assert_eq!(calls.get(), 2);
    calls.set(0);
    let movement = Move::Composite(vec![
        Move::Change {
            variable: 0,
            value: Some(1),
        },
        Move::Change {
            variable: 1,
            value: Some(2),
        },
    ]);
    let trial = director.apply(&movement).unwrap();
    assert_eq!(trial.score(), SimpleScore::new(5));
    assert_eq!(calls.get(), 2);
    assert_eq!(trial.constraint_evaluations(), 2);
    drop(trial);
    assert_eq!(director.score(), SimpleScore::new(4));
    {
        let mut session = director.edit_session().unwrap();
        session.apply(&movement).unwrap();
        assert_eq!(session.score(), SimpleScore::new(5));
    }
    assert_eq!(director.score(), SimpleScore::new(4));
    director.apply(&movement).unwrap().commit();
    director.assert_consistent().unwrap();
}
#[test]
fn list_and_mixed_reorders_settle_dependency_rows_before_scoring() {
    let lists = ListProblem::new(vec![0usize, 1], vec![ListEntity::new(vec![0, 1])]).unwrap();
    let movement = ListMove::Change {
        source: ListPosition {
            entity: 0,
            index: 0,
        },
        destination: ListPosition {
            entity: 0,
            index: 1,
        },
    };
    let mut builder = StreamGraphBuilder::<SimpleScore>::new();
    let sources = ListStreamSources::new(&mut builder);
    let derived = builder
        .derive_dependencies_batched(
            sources.values.port(),
            0..2,
            |r| r.previous.into_iter().collect(),
            |_, r, inputs| {
                Some(r.value as i64 + 1 + inputs.first().and_then(|(_, v)| v.value).unwrap_or(0))
            },
        )
        .unwrap();
    builder
        .score("derived", &derived, |(_, v)| {
            assert!(!v.inconsistent);
            SimpleScore::new(v.value.unwrap())
        })
        .unwrap();
    let mut director =
        ListStreamScoreDirector::new(&lists, builder.build(), sources, lists.initial_assignment())
            .unwrap();
    let trial = director.apply(&movement).unwrap();
    assert_eq!(trial.score(), SimpleScore::new(5));
    assert_eq!(trial.constraint_evaluations(), 2);
    drop(trial);
    assert_eq!(director.score(), SimpleScore::new(4));
    director.apply(&movement).unwrap().commit();
    director.assert_consistent().unwrap();
    let basic = FiniteProblem::new(vec![Variable::new(vec![0usize, 1]).with_initial(0)]).unwrap();
    let problem = MixedProblem::new(basic, lists);
    let mut builder = StreamGraphBuilder::<SimpleScore>::new();
    let sources = MixedStreamSources::new(&mut builder);
    let derived = builder
        .derive_dependencies_batched(
            sources.lists.values.port(),
            0..2,
            |r| r.previous.into_iter().collect(),
            |_, r, inputs| {
                Some(r.value as i64 + 1 + inputs.first().and_then(|(_, v)| v.value).unwrap_or(0))
            },
        )
        .unwrap();
    builder
        .score("derived", &derived, |(_, v)| {
            assert!(!v.inconsistent);
            SimpleScore::new(v.value.unwrap())
        })
        .unwrap();
    builder
        .score("basic", sources.basic.port(), |r| {
            SimpleScore::new(r.1.unwrap_or(0) as i64)
        })
        .unwrap();
    let mut director = MixedStreamScoreDirector::new(
        &problem,
        builder.build(),
        sources,
        problem.initial_assignment(),
    )
    .unwrap();
    let movement = MixedMove {
        basic: Move::Change {
            variable: 0,
            value: Some(1),
        },
        lists: movement,
    };
    let trial = director.apply(&movement).unwrap();
    assert_eq!(trial.score(), SimpleScore::new(6));
    assert_eq!(trial.constraint_evaluations(), 3);
    drop(trial);
    assert_eq!(director.score(), SimpleScore::new(4));
    director.apply(&movement).unwrap().commit();
    director.assert_consistent().unwrap();
}

#[test]
fn all_four_row_parent_assignments_match_an_independent_reachability_oracle() {
    let mut node = DependencyNode::new(0..4, |r: &Row| r.0.into_iter().collect(), compute).unwrap();
    for code in 0..1296 {
        let mut digits = code;
        let rows: [Option<Row>; 4] = std::array::from_fn(|i| {
            let choice = digits % 6;
            digits /= 6;
            if choice == 5 {
                None
            } else {
                Some(((choice < 4).then_some(choice), i as i64 + 1))
            }
        });
        let changes = rows.iter().enumerate().flat_map(|(i, row)| {
            [
                TupleChange::put(i, (Some(99), 99)),
                TupleChange { id: i, value: *row },
            ]
        });
        node.apply_batch(changes).unwrap();
        let mut reach = [[false; 4]; 4];
        for (i, row) in rows.iter().enumerate() {
            if let Some((Some(j), _)) = row {
                reach[i][*j] = true;
            }
        }
        for k in 0..4 {
            for i in 0..4 {
                for j in 0..4 {
                    reach[i][j] |= reach[i][k] && reach[k][j];
                }
            }
        }
        let bad: [bool; 4] = std::array::from_fn(|i| (0..4).any(|j| reach[i][j] && reach[j][j]));
        let mut values = [None; 4];
        for _ in 0..4 {
            for (i, row) in rows.iter().enumerate() {
                if bad[i] {
                    continue;
                }
                if let Some((parent, weight)) = row {
                    values[i] = match parent {
                        Some(j) if rows[*j].is_some() => values[*j].map(|v| v + weight),
                        _ => Some(*weight),
                    };
                }
            }
        }
        let expected = rows
            .into_iter()
            .enumerate()
            .filter_map(|(i, row)| {
                row.map(|row| {
                    (
                        i,
                        (
                            row,
                            DerivedValue {
                                value: values[i],
                                inconsistent: bad[i],
                            },
                        ),
                    )
                })
            })
            .collect::<BTreeMap<_, _>>();
        assert_eq!(contents(&node), expected, "assignment {code}");
    }
}

#[test]
fn output_clone_panic_after_evaluation_does_not_install_the_batch() {
    struct RowWithCloneFailure {
        row: Row,
        clones: Rc<Cell<usize>>,
        panic_at: Rc<Cell<Option<usize>>>,
    }
    impl Clone for RowWithCloneFailure {
        fn clone(&self) -> Self {
            let count = self.clones.get() + 1;
            self.clones.set(count);
            assert_ne!(self.panic_at.get(), Some(count), "output clone panic");
            Self {
                row: self.row,
                clones: self.clones.clone(),
                panic_at: self.panic_at.clone(),
            }
        }
    }
    let clones = Rc::new(Cell::new(0));
    let panic_at = Rc::new(Cell::new(None));
    let row = |inner| RowWithCloneFailure {
        row: inner,
        clones: clones.clone(),
        panic_at: panic_at.clone(),
    };
    let mut node = DependencyNode::new(
        0..3,
        |r: &RowWithCloneFailure| r.row.0.into_iter().collect(),
        |k, r, inputs| compute(k, &r.row, inputs),
    )
    .unwrap();
    node.apply_batch([
        TupleChange::put(0, row((None, 1))),
        TupleChange::put(1, row((Some(0), 2))),
    ])
    .unwrap();
    let before = node
        .rows()
        .map(|(&k, (r, v))| (k, (r.row, v.clone())))
        .collect::<BTreeMap<_, _>>();
    clones.set(0);
    panic_at.set(Some(2));
    assert!(
        catch_unwind(AssertUnwindSafe(|| node.apply_batch([
            TupleChange::put(0, row((Some(1), 3))),
            TupleChange::put(1, row((None, 7)))
        ])))
        .is_err()
    );
    panic_at.set(None);
    assert_eq!(
        node.rows()
            .map(|(&k, (r, v))| (k, (r.row, v.clone())))
            .collect::<BTreeMap<_, _>>(),
        before
    );
    node.apply_batch([TupleChange::put(0, row((None, 4)))])
        .unwrap();
    assert_eq!(
        node.rows().find(|(k, _)| **k == 1).unwrap().1.1.value,
        Some(6)
    );
}

#[test]
fn only_observed_dependency_batch_nodes_enable_automatic_batching() {
    for observed in [false, true] {
        let problem = FiniteProblem::new(vec![
            Variable::new(vec![0usize, 1, 2]).with_initial(1),
            Variable::new(vec![0, 1, 2]).with_initial(2),
        ])
        .unwrap();
        let calls = Cell::new(0);
        let mut builder = StreamGraphBuilder::<SimpleScore>::new();
        let source = builder.source::<usize, (usize, Option<usize>)>();
        let derived = builder
            .derive_dependencies_batched(
                source.port(),
                0..2,
                |r| r.1.filter(|&p| p < 2).into_iter().collect(),
                |_, r, inputs| {
                    calls.set(calls.get() + 1);
                    Some(r.0 as i64 + 1 + inputs.first().and_then(|(_, v)| v.value).unwrap_or(0))
                },
            )
            .unwrap();
        if observed {
            builder.dataset(&derived).unwrap();
        }
        builder
            .score("source", source.port(), |_| SimpleScore::new(1))
            .unwrap();
        let mut director = StreamScoreDirector::new(
            &problem,
            builder.build(),
            source,
            problem.initial_assignment(),
        )
        .unwrap();
        assert_eq!(director.score(), SimpleScore::new(2));
        assert_eq!(calls.get(), if observed { 2 } else { 3 });
        director.assert_consistent().unwrap();
    }
}
