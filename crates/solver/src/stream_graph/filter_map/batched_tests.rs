//! Original identity propagation and old-loop ownership regressions.
use super::*;
use crate::{SimpleScore, StreamBatch, StreamScoreDirector};
use std::{
    cell::{Cell, RefCell},
    cmp::Ordering,
    panic::{AssertUnwindSafe, catch_unwind},
};

type Trace = Rc<RefCell<Vec<(&'static str, i64)>>>;

#[derive(Default)]
struct Control {
    trace: Trace,
    clones: Cell<usize>,
    comparisons: Cell<usize>,
    fail_clone: Cell<Option<usize>>,
    fail_comparison: Cell<Option<usize>>,
    fail_mapping: Cell<bool>,
}

struct Id(i64, Rc<Control>);
impl Clone for Id {
    fn clone(&self) -> Self {
        self.1.trace.borrow_mut().push(("ID clone", self.0));
        let call = self.1.clones.get() + 1;
        self.1.clones.set(call);
        assert_ne!(self.1.fail_clone.get(), Some(call), "ID clone panic");
        Self(self.0, self.1.clone())
    }
}
impl Drop for Id {
    fn drop(&mut self) {
        self.1.trace.borrow_mut().push(("ID drop", self.0));
    }
}
impl PartialEq for Id {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl Eq for Id {}
impl PartialOrd for Id {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Id {
    fn cmp(&self, other: &Self) -> Ordering {
        self.1.trace.borrow_mut().push(("ID compare", self.0));
        let call = self.1.comparisons.get() + 1;
        self.1.comparisons.set(call);
        assert_ne!(
            self.1.fail_comparison.get(),
            Some(call),
            "ID comparison panic"
        );
        self.0.cmp(&other.0)
    }
}

struct Input(i64, Trace); // Deliberately not Clone.
impl Drop for Input {
    fn drop(&mut self) {
        self.1.borrow_mut().push(("input drop", self.0));
    }
}
struct Output(i64, Trace); // Deliberately not Clone.
impl Drop for Output {
    fn drop(&mut self) {
        self.1.borrow_mut().push(("output drop", self.0));
    }
}

#[derive(Debug, PartialEq)]
struct Observation {
    panicked: bool,
    outputs: Vec<(i64, Option<i64>)>,
    trace: Vec<(&'static str, i64)>,
    clones: usize,
    comparisons: usize,
}

fn observe_batch(
    fast: bool,
    mapping_panic: bool,
    clone_panic: Option<usize>,
    comparison_panic: Option<usize>,
) -> Observation {
    let control = Rc::new(Control::default());
    let callback = control.clone();
    let mut node = FilterMap(crate::FilterMapNode::<Id, Input, Output>::new(move |row| {
        callback.trace.borrow_mut().push(("mapping", row.0));
        assert!(
            !(callback.fail_mapping.get() && row.0 == 2),
            "mapping panic"
        );
        (row.0 > 0).then(|| Output(row.0 * 10, row.1.clone()))
    }));
    let seed = TupleChange::put(Id(0, control.clone()), Input(7, control.trace.clone()));
    let _ = node.0.apply_ref(&seed);
    drop(seed);
    let saved =
        <FilterMap<'_, Id, Input, Output> as Node<SimpleScore>>::begin_checkpoint(&mut node);
    let inputs: Vec<_> = [
        (0, Some(1)),
        (1, Some(2)),
        (0, None),
        (1, Some(-1)),
        (2, Some(3)),
        (9, None),
    ]
    .into_iter()
    .map(|(id, value)| {
        (
            false,
            Rc::new(TupleChange {
                id: Id(id, control.clone()),
                value: value.map(|v| Input(v, control.trace.clone())),
            }) as Event,
        )
    })
    .collect();
    control.trace.borrow_mut().clear();
    control.clones.set(0);
    control.comparisons.set(0);
    control.fail_mapping.set(mapping_panic);
    control.fail_clone.set(clone_panic);
    control.fail_comparison.set(comparison_panic);
    let arithmetic = NodeArithmetic {
        impact: crate::ScoringArithmetic::Checked,
        totals: crate::ScoringArithmetic::Checked,
    };
    let result = catch_unwind(AssertUnwindSafe(|| {
        if fast {
            <FilterMap<'_, Id, Input, Output> as Node<SimpleScore>>::apply_batch(
                &mut node, &inputs, arithmetic,
            )
            .unwrap()
        } else {
            // Frozen trait-default algorithm: do not call the new batch method.
            let mut output = Vec::new();
            for (right, event) in &inputs {
                output.extend(
                    <FilterMap<'_, Id, Input, Output> as Node<SimpleScore>>::apply(
                        &mut node, *right, event, arithmetic,
                    )
                    .unwrap(),
                );
            }
            output
        }
    }));
    control.fail_mapping.set(false);
    control.fail_clone.set(None);
    control.fail_comparison.set(None);
    let panicked = result.is_err();
    let outputs = match result {
        Ok(output) => {
            let observed = output
                .iter()
                .map(|event| {
                    let change = typed::<Id, Output>(event);
                    (change.id.0, change.value.as_ref().map(|row| row.0))
                })
                .collect();
            drop(output);
            observed
        }
        Err(_) => Vec::new(),
    };
    let clones = control.clones.get();
    let comparisons = control.comparisons.get();
    <FilterMap<'_, Id, Input, Output> as Node<SimpleScore>>::restore(&mut node, saved);
    assert_eq!(node.0.accepted_count(), 1);
    drop(inputs);
    drop(node);
    let trace = control.trace.borrow().clone();
    Observation {
        panicked,
        outputs,
        trace,
        clones,
        comparisons,
    }
}

#[test]
fn batches_match_the_old_loop_for_repeated_ids_and_nonclone_payloads() {
    let actual = observe_batch(true, false, None, None);
    assert_eq!(actual, observe_batch(false, false, None, None));
    assert!(!actual.panicked);
    assert_eq!(
        actual.outputs,
        [
            (0, Some(10)),
            (1, Some(20)),
            (0, None),
            (1, None),
            (2, Some(30))
        ]
    );
    assert_eq!(
        actual
            .trace
            .iter()
            .filter(|(name, _)| *name == "mapping")
            .map(|(_, value)| *value)
            .collect::<Vec<_>>(),
        [1, 2, -1, 3]
    );
}

#[test]
fn every_id_clone_comparison_and_later_mapping_panic_keeps_old_cleanup_order() {
    let baseline = observe_batch(false, false, None, None);
    for call in 1..=baseline.clones {
        let actual = observe_batch(true, false, Some(call), None);
        assert!(actual.panicked);
        assert_eq!(actual, observe_batch(false, false, Some(call), None));
    }
    for call in 1..=baseline.comparisons {
        let actual = observe_batch(true, false, None, Some(call));
        assert!(actual.panicked);
        assert_eq!(actual, observe_batch(false, false, None, Some(call)));
    }
    let actual = observe_batch(true, true, None, None);
    assert!(actual.panicked);
    assert_eq!(actual, observe_batch(false, true, None, None));
}

struct BindingProbe(Rc<Cell<Option<usize>>>);
impl Node<SimpleScore> for BindingProbe {
    fn bind_fixed_input(&mut self, right: bool, count: usize) {
        assert!(!right);
        self.0.set(Some(count));
    }
    fn preserves_input_ids(&self) -> bool {
        true
    }
    fn checkpoint(&self) -> Box<dyn Any> {
        Box::new(())
    }
    fn clear(&mut self) {}
    fn restore(&mut self, _: Box<dyn Any>) {}
    fn apply(&mut self, _: bool, event: &Event, _: NodeArithmetic) -> Result<Outputs, GraphError> {
        Ok(Outputs::one(event.clone()))
    }
}

#[test]
fn explicit_boundary_forwards_the_validated_universe_and_stops_at_group_keys() {
    for count in [0usize, 2, 256, 257] {
        for pruning in [false, true] {
            let problem = crate::FiniteProblem::new(
                (0..count)
                    .map(|_| crate::Variable::new(vec![1i64, 2]).allowing_unassigned())
                    .collect(),
            )
            .unwrap();
            let mut b = StreamGraphBuilder::<SimpleScore>::new();
            if pruning {
                b = b.with_activity_pruning();
            }
            let source = b.source::<usize, (usize, Option<i64>)>();
            let fused = b
                .filter_map_batched(source.port(), |row| {
                    row.1.filter(|v| v % 2 == 0).map(|v| v * 10)
                })
                .unwrap();
            let binding = Rc::new(Cell::new(None));
            let probe: StreamPort<usize, i64> = b.push(BindingProbe(binding.clone()));
            b.connect(fused.node, probe.node, false);
            let groups = b
                .group_sum_batched(&probe, |value| (9usize, *value))
                .unwrap();
            let after_group = Rc::new(Cell::new(None));
            let after: StreamPort<usize, (usize, i64)> = b.push(BindingProbe(after_group.clone()));
            b.connect(groups.node, after.node, false);
            let data = b.dataset(&groups).unwrap();
            b.score("sum", &after, |row| SimpleScore::new(row.1))
                .unwrap();
            let mut director =
                StreamScoreDirector::new(&problem, b.build(), source, problem.initial_assignment())
                    .unwrap();
            // The validated identity universe propagates independently of the
            // source's 256-row storage limit. Each consumer chooses its cache.
            assert_eq!(binding.get(), Some(count));
            assert_eq!(
                after_group.get(),
                None,
                "group keys start a new ID namespace"
            );
            assert!(director.graph().dataset_rows(&data).unwrap().is_empty());
            if count != 0 {
                let movement = crate::Move::Change {
                    variable: count - 1,
                    value: Some(2),
                };
                {
                    let trial = director.apply(&movement).unwrap();
                    assert_eq!(trial.score(), SimpleScore::new(20));
                    assert_eq!(trial.constraint_evaluations(), 1);
                }
                assert_eq!(director.score(), SimpleScore::ZERO);
                director.apply(&movement).unwrap().commit();
                assert_eq!(director.graph().dataset_rows(&data).unwrap()[&9], (9, 20));
                director.assert_consistent().unwrap();
            }
        }
    }
}

#[test]
fn ordinary_sparse_and_generic_fact_sources_do_not_acquire_a_planning_universe() {
    for generic in [false, true] {
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let source = b.source::<usize, i64>();
        let other = b.source::<String, i64>();
        let filtered = if generic {
            // Map changes only payloads here; IDs remain the arbitrary source IDs.
            let generic_rows = b
                .filter_map_batched(other.port(), |value| (*value > 0).then_some(*value))
                .unwrap();
            let binding = Rc::new(Cell::new(None));
            let probe: StreamPort<String, i64> = b.push(BindingProbe(binding.clone()));
            b.connect(generic_rows.node, probe.node, false);
            b.score("generic", &probe, |value| SimpleScore::new(*value))
                .unwrap();
            Some(binding)
        } else {
            None
        };
        let rows = b
            .filter_map_batched(source.port(), |value| (*value > 0).then_some(*value))
            .unwrap();
        let binding = Rc::new(Cell::new(None));
        let probe: StreamPort<usize, i64> = b.push(BindingProbe(binding.clone()));
        b.connect(rows.node, probe.node, false);
        b.score("facts", &probe, |value| SimpleScore::new(*value))
            .unwrap();
        let mut graph = b.build();
        let mut tx = graph.transaction();
        tx.apply(&source, TupleChange::put(900, 7)).unwrap();
        tx.apply(
            &other,
            TupleChange::put("outside any planning IDs".to_owned(), 3),
        )
        .unwrap();
        tx.commit().unwrap();
        assert_eq!(binding.get(), None);
        if let Some(binding) = filtered {
            assert_eq!(binding.get(), None);
        }
        assert_eq!(
            graph.score(),
            SimpleScore::new(if generic { 10 } else { 7 })
        );
        graph.assert_consistent().unwrap();
    }
}

#[test]
fn planning_nested_edits_saved_winners_and_full_replacement_keep_sparse_membership() {
    use crate::search_scoring::SearchDirector;
    for count in [2usize, 256, 257] {
        let calls = Cell::new(0usize);
        let problem = crate::FiniteProblem::new(
            (0..count)
                .map(|_| crate::Variable::new(vec![1i64, 2, 3, 4]).allowing_unassigned())
                .collect(),
        )
        .unwrap();
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let source = b.source::<usize, (usize, Option<i64>)>();
        let fused = b
            .filter_map_batched(source.port(), |row| row.1.filter(|v| v % 2 == 0))
            .unwrap();
        let data = b.dataset(&fused).unwrap();
        let groups = b.group_sum_batched(&fused, |value| ((), *value)).unwrap();
        b.score("sum", &groups, |row| {
            calls.set(calls.get() + 1);
            SimpleScore::new(row.1)
        })
        .unwrap();
        let initial = problem.initial_assignment();
        let mut director =
            StreamScoreDirector::new(&problem, b.build(), source, initial.clone()).unwrap();
        calls.set(0);
        let selection = director
            .select_best(
                [Some(4), Some(1), None].map(|value| crate::Move::Change {
                    variable: count - 1,
                    value,
                }),
                3,
                crate::ForagerConfig::<SimpleScore>::default(),
                |_, _| true,
                |_| false,
                false,
            )
            .unwrap();
        assert_eq!(selection.evaluations, 3);
        assert_eq!(selection.constraint_evaluations, 1);
        assert_eq!(calls.get(), 1, "saved winner must install without replay");
        assert_eq!(director.score(), SimpleScore::new(4));
        let assignment = director.assignment().clone();
        let held = director.graph().dataset_rows(&data).unwrap().clone();
        {
            let mut outer = director.edit_session().unwrap();
            outer
                .apply(&crate::Move::Change {
                    variable: count - 1,
                    value: None,
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
        {
            let replacement = director.replace_assignment(initial.clone()).unwrap();
            assert_eq!(replacement.score(), SimpleScore::ZERO);
        }
        assert_eq!(director.assignment(), &assignment);
        director.replace_assignment(initial).unwrap().commit();
        assert!(director.graph().dataset_rows(&data).unwrap().is_empty());
        assert_eq!(held[&(count - 1)], 4);
        director.assert_consistent().unwrap();
    }
}

#[test]
fn complete_source_model_matches_sparse_batches_rollback_saved_states_and_rebuild() {
    for pruning in [false, true] {
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        if pruning {
            b = b.with_activity_pruning();
        }
        let source = b.source::<usize, i64>();
        let fused = b
            .filter_map_batched(source.port(), |value| {
                (value % 3 != 0).then_some((value.rem_euclid(5) as usize, value * 2))
            })
            .unwrap();
        let data = b.dataset(&fused).unwrap();
        let groups = b.group_sum_batched(&fused, |row| *row).unwrap();
        let grouped = b.dataset(&groups).unwrap();
        b.score("sum", &groups, |row| SimpleScore::new(row.1))
            .unwrap();
        let mut graph = b.build();
        let mut rows = BTreeMap::<usize, i64>::new();
        let mut random = 268u64;
        for round in 0..180 {
            let mut expected = rows.clone();
            let mut batch = StreamBatch::new();
            for _ in 0..7 {
                random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
                let id = 1000 + ((random >> 17) % 24) as usize;
                let value =
                    (!random.is_multiple_of(4)).then_some(((random >> 33) % 51) as i64 - 25);
                if let Some(value) = value {
                    expected.insert(id, value);
                } else {
                    expected.remove(&id);
                }
                batch.push(&source, TupleChange { id, value }).unwrap();
            }
            let outputs: BTreeMap<_, _> = expected
                .iter()
                .filter_map(|(&id, value)| {
                    (value % 3 != 0).then_some((id, (value.rem_euclid(5) as usize, value * 2)))
                })
                .collect();
            let mut sums = BTreeMap::<usize, i64>::new();
            for &(key, value) in outputs.values() {
                *sums.entry(key).or_default() += value;
            }
            let before = graph.source_rows(&source).unwrap().clone();
            let held = graph.dataset_rows(&data).unwrap().clone();
            let mut tx = graph.transaction();
            tx.apply_batch(batch).unwrap();
            let working = tx.graph().unwrap();
            assert_eq!(working.dataset_rows(&data).unwrap(), &outputs);
            let actual: BTreeMap<_, _> = working
                .dataset_rows(&grouped)
                .unwrap()
                .iter()
                .map(|(key, row)| (*key, row.1))
                .collect();
            assert_eq!(actual, sums);
            assert_eq!(tx.score().unwrap(), SimpleScore::new(sums.values().sum()));
            if round % 7 == 0 {
                let saved = tx.capture().unwrap();
                drop(tx);
                assert_eq!(graph.source_rows(&source).unwrap(), &before);
                graph.install(saved);
                rows = expected;
            } else if round % 4 == 0 {
                drop(tx);
                assert_eq!(graph.source_rows(&source).unwrap(), &before);
                assert_eq!(graph.dataset_rows(&data).unwrap(), &held);
            } else {
                tx.commit().unwrap();
                rows = expected;
            }
            assert_eq!(graph.source_rows(&source).unwrap(), &rows);
            graph.assert_consistent().unwrap();
        }
    }
}

#[test]
fn later_mapping_projected_clone_and_score_panics_restore_bound_and_generic_graphs() {
    struct Projected(i64, Rc<Cell<usize>>);
    impl PartialEq for Projected {
        fn eq(&self, other: &Self) -> bool {
            self.0 == other.0
        }
    }
    impl Clone for Projected {
        fn clone(&self) -> Self {
            assert_ne!(self.1.get(), 2, "projected Clone failure");
            Self(self.0, self.1.clone())
        }
    }
    for count in [2usize, 257] {
        let failure = Rc::new(Cell::new(0usize));
        let problem = crate::FiniteProblem::new(
            (0..count)
                .map(|index| {
                    let variable = crate::Variable::new(vec![2i64, 4]).allowing_unassigned();
                    if index == 0 {
                        variable.with_initial(2)
                    } else {
                        variable
                    }
                })
                .collect(),
        )
        .unwrap();
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let source = b.source::<usize, (usize, Option<i64>)>();
        let mapping_failure = failure.clone();
        let fused = b
            .filter_map_batched(source.port(), move |row| {
                assert!(
                    !(mapping_failure.get() == 1 && row.1 == Some(4)),
                    "projection failure"
                );
                row.1.map(|value| Projected(value, mapping_failure.clone()))
            })
            .unwrap();
        let data = b.dataset(&fused).unwrap();
        let mapped = b.map(&fused, |row| row.0).unwrap();
        let groups = b.group_sum_batched(&mapped, |value| ((), *value)).unwrap();
        let score_failure = failure.clone();
        b.score("sum", &groups, move |row| {
            assert_ne!(score_failure.get(), 3, "score failure");
            SimpleScore::new(row.1)
        })
        .unwrap();
        let inspect = source.clone();
        let initial = problem.initial_assignment();
        let mut director =
            StreamScoreDirector::new(&problem, b.build(), source, initial.clone()).unwrap();
        let held = director.graph().dataset_rows(&data).unwrap().clone();
        let movement = crate::Move::Composite(vec![
            crate::Move::Change {
                variable: 0,
                value: Some(4),
            },
            crate::Move::Change {
                variable: count - 1,
                value: Some(4),
            },
        ]);
        for failed in 1..=3 {
            failure.set(failed);
            assert!(
                catch_unwind(AssertUnwindSafe(|| {
                    let _trial = director.apply(&movement).unwrap();
                }))
                .is_err()
            );
            failure.set(0);
            assert_eq!(director.assignment(), &initial);
            assert_eq!(director.score(), SimpleScore::new(2));
            let rows = director.graph().dataset_rows(&data).unwrap();
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[&0].0, 2);
            assert_eq!(held[&0].0, 2);
            assert_eq!(director.graph().source_rows(&inspect).unwrap().len(), count);
            director.assert_consistent().unwrap();
        }
        director.apply(&movement).unwrap().commit();
        assert_eq!(director.score(), SimpleScore::new(8));
        assert_eq!(director.graph().dataset_rows(&data).unwrap().len(), 2);
        assert_eq!(held[&0].0, 2);
        director.assert_consistent().unwrap();
    }
}

#[test]
fn ordinary_filter_map_keeps_its_legacy_binding_and_publication_capabilities() {
    let old = FilterMap(crate::FilterMapNode::<usize, i64, i64>::new(|value| {
        Some(*value)
    }));
    assert!(!<FilterMap<'_, usize, i64, i64> as Node<SimpleScore>>::preserves_input_ids(&old));
    assert!(!<FilterMap<'_, usize, i64, i64> as Node<SimpleScore>>::prefers_batch(&old));
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, (usize, Option<i64>)>();
    let fused = b.filter_map(source.port(), |row| row.1).unwrap();
    let binding = Rc::new(Cell::new(None));
    let probe: StreamPort<usize, i64> = b.push(BindingProbe(binding.clone()));
    b.connect(fused.node, probe.node, false);
    b.score("sum", &probe, |value| SimpleScore::new(*value))
        .unwrap();
    let problem =
        crate::FiniteProblem::new(vec![crate::Variable::new(vec![1i64, 2]).with_initial(1)])
            .unwrap();
    let director =
        StreamScoreDirector::new(&problem, b.build(), source, problem.initial_assignment())
            .unwrap();
    assert_eq!(binding.get(), None);
    assert_eq!(director.score(), SimpleScore::new(1));
}

#[derive(Clone)]
struct Representative(usize, usize);
impl PartialEq for Representative {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl Eq for Representative {}
impl PartialOrd for Representative {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Representative {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
}

#[test]
fn explicit_batches_keep_first_id_and_final_payload_without_cloning_row_payloads() {
    let trace = Trace::default();
    let callback = trace.clone();
    let mut node = BatchedFilterMap::<Representative, Input, Output>::new(move |row| {
        callback.borrow_mut().push(("mapping", row.0));
        (row.0 > 0).then(|| Output(row.0 * 10, row.1.clone()))
    });
    let _ = node.apply_row(&Representative(2, 200), Some(&Input(4, trace.clone())));
    trace.borrow_mut().clear();
    let inputs: Vec<_> = [
        (3, 30, Some(99)),
        (1, 10, Some(2)),
        (2, 20, Some(7)),
        (3, 31, Some(5)),
        (2, 21, None),
        (0, 0, Some(-1)),
    ]
    .into_iter()
    .map(|(id, tag, value)| {
        (
            false,
            Rc::new(TupleChange {
                id: Representative(id, tag),
                value: value.map(|v| Input(v, trace.clone())),
            }) as Event,
        )
    })
    .collect();
    let arithmetic = NodeArithmetic {
        impact: crate::ScoringArithmetic::Checked,
        totals: crate::ScoringArithmetic::Checked,
    };
    let outputs =
        <BatchedFilterMap<'_, Representative, Input, Output> as Node<SimpleScore>>::apply_batch(
            &mut node, &inputs, arithmetic,
        )
        .unwrap();
    let observed: Vec<_> = outputs
        .iter()
        .map(|event| {
            let change = typed::<Representative, Output>(event);
            (change.id.0, change.id.1, change.value.as_ref().map(|v| v.0))
        })
        .collect();
    assert_eq!(
        observed,
        [(1, 10, Some(20)), (2, 20, None), (3, 30, Some(50))]
    );
    assert_eq!(node.membership.rows().count(), 2);
    assert_eq!(
        trace
            .borrow()
            .iter()
            .filter(|(name, _)| *name == "mapping")
            .map(|(_, value)| *value)
            .collect::<Vec<_>>(),
        [-1, 2, 5]
    );
    // A direct event remains immediate, including equal-valued accepted puts.
    let event: Event = Rc::new(TupleChange::put(
        Representative(1, 11),
        Input(2, trace.clone()),
    ));
    let output = <BatchedFilterMap<'_, Representative, Input, Output> as Node<SimpleScore>>::apply(
        &mut node, false, &event, arithmetic,
    )
    .unwrap();
    assert_eq!(output.len(), 1);
    assert_eq!(typed::<Representative, Output>(&output[0]).id.1, 11);
}

#[test]
fn new_boundary_validates_foreign_ports_before_mapping_or_topology_registration() {
    let calls = Cell::new(0usize);
    let mut original = StreamGraphBuilder::<SimpleScore>::new();
    let source = original.source::<usize, i64>();
    let mut other = StreamGraphBuilder::<SimpleScore>::new();
    let before = other.nodes.len();
    assert!(matches!(
        other.filter_map_batched(source.port(), |value| {
            calls.set(calls.get() + 1);
            Some(*value)
        }),
        Err(GraphError::ForeignPort)
    ));
    assert_eq!(other.nodes.len(), before);
    assert_eq!(calls.get(), 0);
}

type KeyTrace = Rc<RefCell<Vec<(&'static str, usize)>>>;
struct GroupKey(usize, KeyTrace, bool);
impl Clone for GroupKey {
    fn clone(&self) -> Self {
        self.1.borrow_mut().push(("clone", self.0));
        Self(self.0, self.1.clone(), false)
    }
}
impl Drop for GroupKey {
    fn drop(&mut self) {
        self.1
            .borrow_mut()
            .push((if self.2 { "capture drop" } else { "clone drop" }, self.0));
    }
}
impl PartialEq for GroupKey {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl Eq for GroupKey {}
impl PartialOrd for GroupKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for GroupKey {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
}

fn held_capture_release_trace(batched: bool) -> Vec<(&'static str, usize)> {
    let trace = KeyTrace::default();
    let problem = crate::FiniteProblem::new(
        (0..4)
            .map(|_| crate::Variable::new(vec![1i64, 2]).with_initial(1))
            .collect(),
    )
    .unwrap();
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, (usize, Option<i64>)>();
    let mapping = |row: &(usize, Option<i64>)| row.1.map(|value| (row.0, value));
    let fused = if batched {
        b.filter_map_batched(source.port(), mapping)
    } else {
        b.filter_map(source.port(), mapping)
    }
    .unwrap();
    let capture_trace = trace.clone();
    let groups = b
        .group_sum_batched(&fused, move |&(id, value)| {
            (GroupKey(id, capture_trace.clone(), true), value)
        })
        .unwrap();
    b.score("observe", &groups, |_| SimpleScore::ZERO).unwrap();
    let mut director =
        StreamScoreDirector::new(&problem, b.build(), source, problem.initial_assignment())
            .unwrap();
    let full = director.graph().capture_full();
    director
        .replace_assignment(crate::Assignment::new(vec![Some(2); 4]))
        .unwrap()
        .commit();
    assert_eq!(director.assignment().values(), &[Some(2); 4]);
    trace.borrow_mut().clear();
    drop(full);
    let released = trace.borrow().clone();
    drop(director);
    released
}

#[test]
fn explicit_binding_retains_snapshot_owners_without_promising_old_key_release_order() {
    let old = held_capture_release_trace(false);
    let new = held_capture_release_trace(true);
    // Captured key owners must all be released once after all rows were replaced.
    // Tree and Dense snapshots may release those owners in different orders.
    let owners = |trace: &[(&'static str, usize)], role| {
        assert!(
            trace
                .iter()
                .all(|(action, _)| matches!(*action, "capture drop" | "clone drop"))
        );
        let mut values: Vec<_> = trace
            .iter()
            .filter_map(|(action, value)| (*action == role).then_some(*value))
            .collect();
        values.sort();
        values
    };
    assert_eq!(owners(&old, "capture drop"), [0, 1, 2, 3]);
    assert_eq!(owners(&new, "capture drop"), [0, 1, 2, 3]);
    assert_eq!(owners(&old, "clone drop"), owners(&new, "clone drop"));
}

#[test]
fn zero_outputs_and_nonclone_intermediate_payloads_keep_real_callbacks_and_membership() {
    struct NonClone(i64);
    let first_calls = Cell::new(0usize);
    let second_calls = Cell::new(0usize);
    let score_calls = Cell::new(0usize);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, i64>();
    let first = b
        .filter_map_batched(source.port(), |value| {
            first_calls.set(first_calls.get() + 1);
            Some(NonClone(*value))
        })
        .unwrap();
    let second = b
        .filter_map_batched(&first, |row| {
            second_calls.set(second_calls.get() + 1);
            Some(row.0)
        })
        .unwrap();
    let data = b.dataset(&second).unwrap();
    b.score("zero", &second, |value| {
        score_calls.set(score_calls.get() + 1);
        SimpleScore::new(*value)
    })
    .unwrap();
    let mut graph = b.build();
    let saved;
    {
        let mut batch = StreamBatch::new();
        batch.push(&source, TupleChange::put(0, 5)).unwrap();
        batch.push(&source, TupleChange::put(0, 0)).unwrap();
        let mut tx = graph.transaction();
        tx.apply_batch(batch).unwrap();
        assert_eq!(tx.score_evaluations(), 1);
        assert_eq!(tx.score().unwrap(), SimpleScore::ZERO);
        saved = tx.capture().unwrap();
    }
    assert_eq!(first_calls.get(), 1);
    assert_eq!(second_calls.get(), 1);
    assert_eq!(score_calls.get(), 1);
    assert!(graph.source_rows(&source).unwrap().is_empty());
    graph.install(saved);
    assert_eq!(graph.source_rows(&source).unwrap()[&0], 0);
    assert_eq!(graph.dataset_rows(&data).unwrap()[&0], 0);
    {
        let mut tx = graph.transaction();
        tx.apply(&source, TupleChange::retract(0)).unwrap();
        assert_eq!(tx.score_evaluations(), 0);
        tx.commit().unwrap();
    }
    assert_eq!(first_calls.get(), 1);
    assert_eq!(second_calls.get(), 1);
    assert_eq!(score_calls.get(), 1);
    assert!(graph.dataset_rows(&data).unwrap().is_empty());
    graph.assert_consistent().unwrap();
}
