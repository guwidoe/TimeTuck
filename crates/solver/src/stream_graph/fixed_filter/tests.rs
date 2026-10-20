//! Original fixed-filter regressions; no upstream-method credit.
use super::*;
use crate::{SimpleScore, StreamBatch, StreamScoreDirector};
use std::cell::{Cell, RefCell};
use std::panic::{AssertUnwindSafe, catch_unwind};

fn arithmetic() -> NodeArithmetic {
    NodeArithmetic {
        impact: crate::ScoringArithmetic::Checked,
        totals: crate::ScoringArithmetic::Checked,
    }
}
fn event<I: 'static, T: 'static>(id: I, value: Option<T>) -> Event {
    Rc::new(TupleChange { id, value })
}
fn apply<I: Ord + Clone + 'static, T: 'static>(
    node: &mut FixedFilter<'_, I, T>,
    value: &Event,
) -> Outputs {
    <FixedFilter<'_, I, T> as Node<SimpleScore>>::apply(node, false, value, arithmetic()).unwrap()
}

#[test]
fn fixed_filter_arbitrary_sparse_contiguous_and_empty_universes_validate_before_install() {
    for ids in [vec![], vec![usize::MAX], vec![0, usize::MAX], vec![0, 1, 2]] {
        let node = FixedFilter::new(ids.clone(), |_: &i64| true).unwrap();
        for query in [0, 1, 2, 3, usize::MAX] {
            assert_eq!(node.position(&query).is_ok(), ids.contains(&query));
        }
    }
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<String, i64>();
    let before = b.nodes.len();
    assert!(
        b.filter_fixed(source.port(), ["a".into(), "a".into()], |_| true)
            .is_err()
    );
    assert_eq!(b.nodes.len(), before);
    let mut foreign = StreamGraphBuilder::<SimpleScore>::new();
    assert!(matches!(
        foreign.filter_fixed(source.port(), ["a".into()], |_| true),
        Err(GraphError::ForeignPort)
    ));
    assert!(foreign.nodes.is_empty());
    let filtered = b
        .filter_fixed(source.port(), ["z".into(), "a".into()], |v| *v > 0)
        .unwrap();
    let data = b.dataset(&filtered).unwrap();
    let mut graph = b.build();
    let mut tx = graph.transaction();
    tx.apply(&source, TupleChange::put("z".into(), 3)).unwrap();
    tx.commit().unwrap();
    for value in [Some(-1), Some(4), None] {
        let mut tx = graph.transaction();
        assert!(
            tx.apply(
                &source,
                TupleChange {
                    id: "unknown".into(),
                    value
                }
            )
            .is_err()
        );
        assert!(matches!(tx.graph(), Err(GraphError::FailedTransaction)));
        assert!(matches!(tx.commit(), Err(GraphError::FailedTransaction)));
        assert_eq!(graph.source_rows(&source).unwrap().len(), 1);
        assert_eq!(graph.dataset_rows(&data).unwrap()[&"z".to_owned()], 3);
    }
}

#[test]
fn fixed_filter_nonclone_payloads_forward_actual_events_and_batch_keeps_intermediates() {
    struct Payload(i64);
    let calls = RefCell::new(Vec::new());
    let mut node = FixedFilter::new(vec![2usize, 9], |v: &Payload| {
        calls.borrow_mut().push(v.0);
        v.0 > 0
    })
    .unwrap();
    let input = [
        event(2usize, Some(Payload(-1))),
        event(2usize, Some(Payload(3))),
        event(2usize, Some(Payload(4))),
        event(2usize, Some(Payload(-2))),
        event::<usize, Payload>(2, None),
        event(9usize, Some(Payload(5))),
        event::<usize, Payload>(9, None),
        event::<usize, Payload>(9, None),
    ];
    let batch: Vec<_> = input.iter().map(|v| (false, v.clone())).collect();
    let output = <FixedFilter<'_, usize, Payload> as Node<SimpleScore>>::apply_batch(
        &mut node,
        &batch,
        arithmetic(),
    )
    .unwrap();
    let observed: Vec<_> = output
        .iter()
        .map(|e| {
            let c = typed::<usize, Payload>(e);
            (c.id, c.value.as_ref().map(|p| p.0))
        })
        .collect();
    assert_eq!(
        observed,
        [
            (2, Some(3)),
            (2, Some(4)),
            (2, None),
            (9, Some(5)),
            (9, None)
        ]
    );
    assert!(Rc::ptr_eq(&output[0], &input[1]));
    assert!(Rc::ptr_eq(&output[1], &input[2]));
    assert!(Rc::ptr_eq(&output[3], &input[5]));
    assert_eq!(*calls.borrow(), [-1, 3, 4, -2, 5]);
    let invalid = [
        (false, event(2usize, Some(Payload(7)))),
        (false, event::<usize, Payload>(10, None)),
    ];
    assert!(
        <FixedFilter<'_, usize, Payload> as Node<SimpleScore>>::apply_batch(
            &mut node,
            &invalid,
            arithmetic()
        )
        .is_err()
    );
    assert_eq!(*calls.borrow(), [-1, 3, 4, -2, 5]);
    assert!(node.rows.iter().all(|row| *row == Membership::Absent));
}

#[test]
fn fixed_filter_full_installs_nested_commits_clear_and_outer_undo_copy_only_membership() {
    let calls = Cell::new(0);
    let mut node = FixedFilter::new(vec!["a", "b"], |v: &i64| {
        calls.set(calls.get() + 1);
        *v > 0
    })
    .unwrap();
    apply(&mut node, &event("a", Some(1i64)));
    apply(&mut node, &event("b", Some(-1i64)));
    let outer = <FixedFilter<'_, _, _> as Node<SimpleScore>>::begin_checkpoint(&mut node);
    apply(&mut node, &event("a", Some(-2i64)));
    let full = <FixedFilter<'_, _, _> as Node<SimpleScore>>::checkpoint(&node);
    let inner = <FixedFilter<'_, _, _> as Node<SimpleScore>>::begin_checkpoint(&mut node);
    apply(&mut node, &event("b", Some(2i64)));
    <FixedFilter<'_, _, _> as Node<SimpleScore>>::commit_checkpoint(&mut node, inner);
    <FixedFilter<'_, _, _> as Node<SimpleScore>>::clear(&mut node);
    <FixedFilter<'_, _, _> as Node<SimpleScore>>::restore(&mut node, full);
    assert_eq!(node.rows, [Membership::Rejected, Membership::Rejected]);
    <FixedFilter<'_, _, _> as Node<SimpleScore>>::restore(&mut node, outer);
    assert_eq!(node.rows, [Membership::Accepted, Membership::Rejected]);
    assert_eq!(calls.get(), 4);
    assert!(node.undo.is_empty());
    assert!(node.frames.is_empty());
}

#[test]
fn fixed_filter_independent_source_model_covers_batches_saved_install_and_discard() {
    let ids = [7usize, 90, usize::MAX];
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, i64>();
    let filtered = b
        .filter_fixed(source.port(), ids, |v| v.rem_euclid(3) == 1)
        .unwrap();
    let data = b.dataset(&filtered).unwrap();
    b.score("sum", &filtered, |v| SimpleScore::new(*v)).unwrap();
    let mut graph = b.build();
    let mut model = BTreeMap::new();
    let mut random = 274u64;
    for round in 0..120 {
        let mut candidate = model.clone();
        let mut batch = StreamBatch::new();
        for _ in 0..5 {
            random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
            let id = ids[(random >> 32) as usize % ids.len()];
            let value = (!random.is_multiple_of(4)).then_some(((random >> 17) % 31) as i64 - 15);
            match value {
                Some(v) => {
                    candidate.insert(id, v);
                }
                None => {
                    candidate.remove(&id);
                }
            }
            batch.push(&source, TupleChange { id, value }).unwrap();
        }
        let expected: BTreeMap<_, _> = candidate
            .iter()
            .filter_map(|(&k, &v)| (v.rem_euclid(3) == 1).then_some((k, v)))
            .collect();
        let mut tx = graph.transaction();
        tx.apply_batch(batch).unwrap();
        assert_eq!(tx.graph().unwrap().dataset_rows(&data).unwrap(), &expected);
        assert_eq!(
            tx.score().unwrap(),
            SimpleScore::new(expected.values().sum())
        );
        if round % 7 == 0 {
            let saved = tx.capture().unwrap();
            drop(tx);
            graph.install(saved);
            model = candidate;
        } else if round % 4 == 0 {
            drop(tx);
        } else {
            tx.commit().unwrap();
            model = candidate;
        }
        assert_eq!(graph.source_rows(&source).unwrap(), &model);
        let expected: BTreeMap<_, _> = model
            .iter()
            .filter_map(|(&k, &v)| (v.rem_euclid(3) == 1).then_some((k, v)))
            .collect();
        assert_eq!(graph.dataset_rows(&data).unwrap(), &expected);
        graph.assert_consistent().unwrap();
    }
}

#[test]
fn fixed_filter_predicate_and_downstream_panics_deny_retained_transaction_then_restore() {
    let failure = Cell::new(0);
    let calls = Cell::new(0);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, i64>();
    let filtered = b
        .filter_fixed(source.port(), [0, 1], |v| {
            assert!(!(failure.get() == 1 && *v == 9));
            *v > 0
        })
        .unwrap();
    let data = b.dataset(&filtered).unwrap();
    b.score("sum", &filtered, |v| {
        calls.set(calls.get() + 1);
        assert_ne!(failure.get(), 2);
        SimpleScore::new(*v)
    })
    .unwrap();
    let mut graph = b.build();
    let mut tx = graph.transaction();
    tx.apply(&source, TupleChange::put(0, 3)).unwrap();
    tx.apply(&source, TupleChange::put(1, -3)).unwrap();
    tx.commit().unwrap();
    for fail in [1, 2] {
        failure.set(fail);
        let before = calls.get();
        let mut tx = graph.transaction();
        assert!(
            catch_unwind(AssertUnwindSafe(
                || tx.apply(&source, TupleChange::put(0, 9))
            ))
            .is_err()
        );
        assert!(matches!(tx.graph(), Err(GraphError::FailedTransaction)));
        assert!(matches!(tx.score(), Err(GraphError::FailedTransaction)));
        assert_eq!(tx.score_evaluations(), u128::from(fail == 2));
        assert!(matches!(tx.capture(), Err(GraphError::FailedTransaction)));
        assert!(matches!(tx.commit(), Err(GraphError::FailedTransaction)));
        failure.set(0);
        assert_eq!(calls.get() - before, usize::from(fail == 2));
        assert_eq!(graph.score(), SimpleScore::new(3));
        assert_eq!(graph.dataset_rows(&data).unwrap()[&0], 3);
        assert_eq!(graph.source_rows(&source).unwrap()[&1], -3);
        graph.assert_consistent().unwrap();
    }
}

#[test]
fn fixed_filter_generated_retract_clone_panics_before_install_and_uses_declared_representative() {
    struct Id {
        key: usize,
        label: &'static str,
        fail: Rc<Cell<bool>>,
    }
    impl Clone for Id {
        fn clone(&self) -> Self {
            assert!(self.label != "declared" || !self.fail.get());
            Self {
                key: self.key,
                label: self.label,
                fail: self.fail.clone(),
            }
        }
    }
    impl PartialEq for Id {
        fn eq(&self, other: &Self) -> bool {
            self.key == other.key
        }
    }
    impl Eq for Id {}
    impl PartialOrd for Id {
        fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
            Some(self.cmp(other))
        }
    }
    impl Ord for Id {
        fn cmp(&self, other: &Self) -> std::cmp::Ordering {
            self.key.cmp(&other.key)
        }
    }
    let fail = Rc::new(Cell::new(false));
    let mut node = FixedFilter::new(
        vec![Id {
            key: 1,
            label: "declared",
            fail: fail.clone(),
        }],
        |v: &i64| *v > 0,
    )
    .unwrap();
    let put = event(
        Id {
            key: 1,
            label: "incoming",
            fail: fail.clone(),
        },
        Some(3i64),
    );
    let output = apply(&mut node, &put);
    assert!(Rc::ptr_eq(&output[0], &put));
    assert_eq!(typed::<Id, i64>(&output[0]).id.label, "incoming");
    fail.set(true);
    let rejected = event(
        Id {
            key: 1,
            label: "reject",
            fail: fail.clone(),
        },
        Some(-1i64),
    );
    assert!(catch_unwind(AssertUnwindSafe(|| apply(&mut node, &rejected))).is_err());
    assert_eq!(node.rows, [Membership::Accepted]);
    fail.set(false);
    let output = apply(&mut node, &rejected);
    assert_eq!(typed::<Id, i64>(&output[0]).id.label, "declared");
    assert!(typed::<Id, i64>(&output[0]).value.is_none());
    assert_eq!(node.rows, [Membership::Rejected]);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<Id, i64>();
    let id = |label| Id {
        key: 1,
        label,
        fail: fail.clone(),
    };
    let filtered = b
        .filter_fixed(source.port(), [id("declared")], |v| *v > 0)
        .unwrap();
    let data = b.dataset(&filtered).unwrap();
    let mut graph = b.build();
    let mut tx = graph.transaction();
    tx.apply(&source, TupleChange::put(id("incoming"), 3))
        .unwrap();
    tx.commit().unwrap();
    fail.set(true);
    let mut tx = graph.transaction();
    assert!(
        catch_unwind(AssertUnwindSafe(
            || tx.apply(&source, TupleChange::put(id("incoming"), -1))
        ))
        .is_err()
    );
    assert!(matches!(tx.graph(), Err(GraphError::FailedTransaction)));
    assert!(matches!(tx.commit(), Err(GraphError::FailedTransaction)));
    fail.set(false);
    assert_eq!(graph.source_rows(&source).unwrap()[&id("query")], 3);
    assert_eq!(graph.dataset_rows(&data).unwrap()[&id("query")], 3);
    let mut tx = graph.transaction();
    tx.apply(&source, TupleChange::put(id("incoming"), -1))
        .unwrap();
    tx.commit().unwrap();
    assert!(graph.dataset_rows(&data).unwrap().is_empty());
    assert_eq!(graph.source_rows(&source).unwrap()[&id("query")], -1);
}

#[test]
fn fixed_filter_activity_replay_retains_rejected_and_disabled_source_facts() {
    for pruning in [false, true] {
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        if pruning {
            b = b.with_activity_pruning();
        }
        let source = b.source::<usize, i64>();
        let calls = Cell::new(0);
        let filtered = b
            .filter_fixed(source.port(), [0, 1, 2], |v| {
                calls.set(calls.get() + 1);
                *v > 0
            })
            .unwrap();
        let terminal = b
            .score_weighted("sum", &filtered, SimpleScore::ZERO, |v| *v)
            .unwrap();
        let mut graph = b.build();
        for (id, value) in [(0, 4), (1, -2), (2, 6)] {
            let mut tx = graph.transaction();
            tx.apply(&source, TupleChange::put(id, value)).unwrap();
            tx.commit().unwrap();
        }
        assert_eq!(graph.source_rows(&source).unwrap().len(), 3);
        assert_eq!(graph.score(), SimpleScore::ZERO);
        let weights =
            crate::ConstraintWeightOverrides::from_iter([("sum".to_owned(), SimpleScore::new(1))]);
        graph.apply_weight_overrides(&weights).unwrap();
        assert_eq!(graph.score(), SimpleScore::new(10));
        assert_eq!(graph.constraint_matches(&terminal).unwrap().len(), 2);
        assert_eq!(graph.source_rows(&source).unwrap()[&1], -2);
        assert!(calls.get() >= 3);
        graph
            .apply_weight_overrides(&crate::ConstraintWeightOverrides::from_iter([(
                "sum".to_owned(),
                SimpleScore::ZERO,
            )]))
            .unwrap();
        let mut tx = graph.transaction();
        tx.apply(&source, TupleChange::put(1, 8)).unwrap();
        tx.apply(&source, TupleChange::put(2, -6)).unwrap();
        tx.commit().unwrap();
        graph.apply_weight_overrides(&weights).unwrap();
        assert_eq!(graph.score(), SimpleScore::new(12));
        assert_eq!(graph.source_rows(&source).unwrap()[&2], -6);
        graph.assert_consistent().unwrap();
    }
}

#[test]
fn fixed_filter_actual_planning_trials_nested_rollback_and_saved_winner_preserve_callbacks() {
    use crate::search_scoring::SearchDirector;
    let problem = crate::FiniteProblem::new(
        (0..3)
            .map(|_| crate::Variable::new(vec![1i64, 2, 4]).allowing_unassigned())
            .collect(),
    )
    .unwrap();
    let calls = Cell::new(0);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, (usize, Option<i64>)>();
    let filtered = b
        .filter_fixed(source.port(), 0..3, |row| row.1.is_some_and(|v| v % 2 == 0))
        .unwrap();
    let data = b.dataset(&filtered).unwrap();
    b.score("sum", &filtered, |row| {
        calls.set(calls.get() + 1);
        SimpleScore::new(row.1.unwrap())
    })
    .unwrap();
    let initial = problem.initial_assignment();
    let mut director = StreamScoreDirector::new(&problem, b.build(), source, initial).unwrap();
    let selected = director
        .select_best(
            [Some(4), Some(1), None].map(|value| crate::Move::Change { variable: 2, value }),
            3,
            crate::ForagerConfig::<SimpleScore>::default(),
            |_, _| true,
            |_| false,
            false,
        )
        .unwrap();
    assert_eq!(selected.evaluations, 3);
    assert_eq!(selected.constraint_evaluations, 1);
    assert_eq!(calls.get(), 1);
    assert_eq!(director.score(), SimpleScore::new(4));
    let held = director.graph().dataset_rows(&data).unwrap().clone();
    let assignment = director.assignment().clone();
    {
        let mut outer = director.edit_session().unwrap();
        outer
            .apply(&crate::Move::Change {
                variable: 2,
                value: Some(1),
            })
            .unwrap();
        {
            let mut inner = outer.nested().unwrap();
            inner
                .apply(&crate::Move::Change {
                    variable: 0,
                    value: Some(2),
                })
                .unwrap();
            inner.commit();
        }
        assert_eq!(outer.director().score(), SimpleScore::new(2));
    }
    assert_eq!(director.assignment(), &assignment);
    assert_eq!(director.graph().dataset_rows(&data).unwrap(), &held);
    director.assert_consistent().unwrap();
}
