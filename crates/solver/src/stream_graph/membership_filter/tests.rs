//! Original graph-only acceptance-cache controls; no upstream method credit.
use super::*;
use crate::{Score, SimpleScore};
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
};
fn arithmetic() -> NodeArithmetic {
    NodeArithmetic {
        impact: crate::ScoringArithmetic::Checked,
        totals: crate::ScoringArithmetic::Checked,
    }
}
fn event<I: 'static, T: 'static>(change: TupleChange<I, T>) -> Event {
    Event::new(change)
}
fn outputs(node: &mut dyn Node<SimpleScore>, event: &Event) -> Vec<(usize, Option<i64>)> {
    node.apply(false, event, arithmetic())
        .unwrap()
        .into_iter()
        .map(|event| {
            let change = typed::<usize, i64>(&event);
            (change.id, change.value)
        })
        .collect()
}

#[test]
fn membership_filter_matches_reference_emissions_batch_order_nested_captures_and_restore() {
    let trace = RefCell::new(Vec::new());
    let old_trace = RefCell::new(Vec::new());
    let mut fast: Box<dyn Node<SimpleScore> + '_> = Box::new(MembershipFilter::new(
        Membership::<usize>::new().unwrap(),
        |v: &i64| {
            trace.borrow_mut().push(*v);
            *v > 0
        },
    ));
    let mut original = FilterNode::<usize, i64>::new(|v: &i64| {
        old_trace.borrow_mut().push(*v);
        *v > 0
    });
    original.bind_primitive_inputs();
    let mut old: Box<dyn Node<SimpleScore> + '_> = Box::new(Filter(original));
    for change in [
        TupleChange::put(0usize, 1i64),
        TupleChange::put(15, -2),
        TupleChange::put(usize::MAX, 3),
        TupleChange::put(0, 1),
        TupleChange::retract(16),
    ] {
        let event = event(change);
        assert_eq!(
            outputs(fast.as_mut(), &event),
            outputs(old.as_mut(), &event)
        );
    }
    let outer = fast.begin_checkpoint();
    let old_outer = old.begin_checkpoint();
    let batch = vec![
        (false, event(TupleChange::put(0usize, -1i64))),
        (false, event(TupleChange::put(15usize, 2i64))),
        (false, event(TupleChange::<usize, i64>::retract(usize::MAX))),
    ];
    let read = |events: BatchOutputs| {
        events
            .into_iter()
            .map(|event| {
                let row = typed::<usize, i64>(&event);
                (row.id, row.value)
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(
        read(fast.apply_batch(&batch, arithmetic()).unwrap()),
        read(old.apply_batch(&batch, arithmetic()).unwrap())
    );
    let nested = fast.begin_checkpoint();
    let old_nested = old.begin_checkpoint();
    let e = event(TupleChange::put(16usize, 3i64));
    assert_eq!(outputs(fast.as_mut(), &e), outputs(old.as_mut(), &e));
    let saved = fast.capture_checkpoint(outer.as_ref());
    let old_saved = old.capture_checkpoint(old_outer.as_ref());
    fast.commit_checkpoint(nested);
    old.commit_checkpoint(old_nested);
    fast.restore(outer);
    old.restore(old_outer);
    let full = fast.checkpoint();
    let old_full = old.checkpoint();
    let outer = fast.begin_checkpoint();
    let old_outer = old.begin_checkpoint();
    let before = trace.borrow().len();
    fast.restore(saved);
    old.restore(old_saved);
    for id in [0usize, 15, 16, usize::MAX] {
        let e = event(TupleChange::<usize, i64>::retract(id));
        assert_eq!(outputs(fast.as_mut(), &e), outputs(old.as_mut(), &e));
    }
    fast.restore(outer);
    old.restore(old_outer);
    fast.clear();
    old.clear();
    fast.restore(full);
    old.restore(old_full);
    assert_eq!(trace.borrow().len(), before);
    assert_eq!(*trace.borrow(), *old_trace.borrow());
    for id in [0usize, 15, 16, usize::MAX] {
        let e = event(TupleChange::<usize, i64>::retract(id));
        assert_eq!(outputs(fast.as_mut(), &e), outputs(old.as_mut(), &e));
    }
}

#[test]
fn membership_filter_keeps_custom_nodrop_clone_order_and_both_clone_failure_boundaries() {
    #[derive(Default)]
    struct Probe {
        trace: RefCell<Vec<(&'static str, i64)>>,
        clones: Cell<usize>,
        fail: Cell<usize>,
    }
    struct Counted(i64, &'static Probe);
    impl Clone for Counted {
        fn clone(&self) -> Self {
            self.1.trace.borrow_mut().push(("clone", self.0));
            let n = self.1.clones.get() + 1;
            self.1.clones.set(n);
            assert!(n != self.1.fail.get(), "user Clone");
            Self(self.0, self.1)
        }
    }
    let probe: &'static Probe = Box::leak(Box::new(Probe::default()));
    let old_probe: &'static Probe = Box::leak(Box::new(Probe::default()));
    assert!(!std::mem::needs_drop::<Counted>());
    let mut fast: Box<dyn Node<SimpleScore>> = Box::new(MembershipFilter::new(
        Membership::<usize>::new().unwrap(),
        move |row: &Counted| {
            probe.trace.borrow_mut().push(("predicate", row.0));
            row.0 > 0
        },
    ));
    let mut original = FilterNode::<usize, Counted>::new(move |row: &Counted| {
        old_probe.trace.borrow_mut().push(("predicate", row.0));
        row.0 > 0
    });
    original.bind_primitive_inputs();
    let mut old: Box<dyn Node<SimpleScore>> = Box::new(Filter(original));
    for fail_stage in [0usize, 1, 2] {
        probe.fail.set(if fail_stage == 0 {
            0
        } else {
            probe.clones.get() + fail_stage
        });
        old_probe.fail.set(if fail_stage == 0 {
            0
        } else {
            old_probe.clones.get() + fail_stage
        });
        let fast_event = event(TupleChange::put(usize::MAX, Counted(1, probe)));
        let old_event = event(TupleChange::put(usize::MAX, Counted(1, old_probe)));
        let a = catch_unwind(AssertUnwindSafe(|| {
            fast.apply(false, &fast_event, arithmetic())
        }));
        let b = catch_unwind(AssertUnwindSafe(|| {
            old.apply(false, &old_event, arithmetic())
        }));
        assert_eq!(a.is_ok(), b.is_ok());
        assert_eq!(a.is_ok(), fail_stage == 0);
        assert_eq!(*probe.trace.borrow(), *old_probe.trace.borrow());
        // Neither input nor output Clone failure poisons or changes acceptance.
        let e = event(TupleChange::<usize, Counted>::retract(usize::MAX));
        assert_eq!(fast.apply(false, &e, arithmetic()).unwrap().len(), 1);
        assert_eq!(old.apply(false, &e, arithmetic()).unwrap().len(), 1);
        probe.fail.set(0);
        old_probe.fail.set(0);
        fast.apply(
            false,
            &event(TupleChange::put(usize::MAX, Counted(1, probe))),
            arithmetic(),
        )
        .unwrap();
        old.apply(
            false,
            &event(TupleChange::put(usize::MAX, Counted(1, old_probe))),
            arithmetic(),
        )
        .unwrap();
        assert_eq!(*probe.trace.borrow(), *old_probe.trace.borrow());
    }
}

#[test]
fn membership_filter_preserves_bound_panic_poison_and_sticky_rejected_universe_evidence() {
    let calls = Cell::new(0);
    let mut node = MembershipFilter::new(Membership::<usize>::new().unwrap(), |v: &i64| {
        calls.set(calls.get() + 1);
        *v > 0
    });
    for id in 0usize..2 {
        node.apply_row(&event(TupleChange::put(id, 1i64)));
    }
    assert!(node.bind_fixed(2));
    assert!(!node.bind_fixed(2));
    let saved =
        <MembershipFilter<'_, usize, i64> as Node<SimpleScore>>::begin_checkpoint(&mut node);
    assert!(
        catch_unwind(AssertUnwindSafe(
            || node.apply_row(&event(TupleChange::put(2usize, -1i64)))
        ))
        .is_err()
    );
    assert!(node.poisoned);
    assert_eq!(calls.get(), 3);
    <MembershipFilter<'_, usize, i64> as Node<SimpleScore>>::restore(&mut node, saved);
    assert!(!node.poisoned);
    assert!(
        node.apply_row(&event(TupleChange::<usize, i64>::retract(usize::MAX)))
            .is_none()
    );
    assert_eq!(calls.get(), 3);
    let mut pairs =
        MembershipFilter::new(Membership::<(usize, usize)>::new().unwrap(), |v: &i64| {
            *v > 0
        });
    pairs.apply_row(&event(TupleChange::put((0usize, 0usize), 1i64)));
    assert!(pairs.bind_pair(2, 65));
    assert!(
        catch_unwind(AssertUnwindSafe(
            || pairs.apply_row(&event(TupleChange::put((1usize, 65usize), 1i64)))
        ))
        .is_err()
    );
    assert!(pairs.poisoned);
    let mut sparse =
        MembershipFilter::new(Membership::<(usize, usize)>::new().unwrap(), |v: &i64| {
            *v > 0
        });
    let full =
        <MembershipFilter<'_, (usize, usize), i64> as Node<SimpleScore>>::checkpoint(&sparse);
    let outer = <MembershipFilter<'_, (usize, usize), i64> as Node<SimpleScore>>::begin_checkpoint(
        &mut sparse,
    );
    sparse.apply_row(&event(TupleChange::put((usize::MAX, usize::MAX), -1i64)));
    <MembershipFilter<'_, (usize, usize), i64> as Node<SimpleScore>>::restore(&mut sparse, outer);
    <MembershipFilter<'_, (usize, usize), i64> as Node<SimpleScore>>::clear(&mut sparse);
    <MembershipFilter<'_, (usize, usize), i64> as Node<SimpleScore>>::restore(&mut sparse, full);
    assert!(!sparse.bind_pair(2, 2));
}

#[test]
fn membership_filter_actual_graph_keeps_breadth_first_callbacks_matches_and_transaction_rollback() {
    type Observation = (Vec<(&'static str, i64)>, i64, Vec<(usize, i64)>);
    fn run(legacy: bool) -> Observation {
        let trace = RefCell::new(Vec::new());
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let source = b.source::<usize, i64>();
        let predicate = |v: &i64| {
            trace.borrow_mut().push(("filter", *v));
            *v > 0
        };
        let filtered = if legacy {
            let mut node = FilterNode::<usize, i64>::new(predicate);
            node.bind_primitive_inputs();
            let out = b.push(Filter(node));
            b.connect(source.0.node, out.node, false);
            out
        } else {
            b.filter(source.port(), predicate).unwrap()
        };
        let a = b
            .map(&filtered, |v| {
                trace.borrow_mut().push(("a", *v));
                *v
            })
            .unwrap();
        let c = b
            .map(&filtered, |v| {
                trace.borrow_mut().push(("b", *v));
                *v
            })
            .unwrap();
        let constraint = b
            .score("a", &a, |v| {
                trace.borrow_mut().push(("score a", *v));
                SimpleScore::new(*v)
            })
            .unwrap();
        b.score("b", &c, |v| {
            trace.borrow_mut().push(("score b", *v));
            SimpleScore::new(*v)
        })
        .unwrap();
        let mut graph = b.build();
        {
            let mut t = graph.transaction();
            t.apply(&source, TupleChange::put(0usize, 1i64)).unwrap();
            t.apply(&source, TupleChange::put(15usize, -1i64)).unwrap();
            t.commit().unwrap();
        }
        {
            let mut t = graph.transaction();
            t.apply(&source, TupleChange::put(0usize, -2i64)).unwrap();
            t.apply(&source, TupleChange::put(usize::MAX, 3i64))
                .unwrap();
        }
        let score = graph.score().ordinary_levels().next().unwrap();
        let matches = graph
            .constraint_matches(&constraint)
            .unwrap()
            .iter()
            .map(|(id, (value, _))| (*id, *value))
            .collect();
        drop(graph);
        (trace.into_inner(), score, matches)
    }
    let fast = run(false);
    let reference = run(true);
    assert_eq!(fast, reference);
    assert_eq!(fast.1, 2);
    assert_eq!(fast.2, vec![(0, 1)]);
    assert_eq!(
        &fast.0[..5],
        &[
            ("filter", 1),
            ("a", 1),
            ("b", 1),
            ("score a", 1),
            ("score b", 1)
        ]
    );
}

#[test]
fn membership_filter_predicate_panic_keeps_acceptance_and_does_not_poison() {
    let fail = Cell::new(false);
    let calls = Cell::new(0);
    let mut node = MembershipFilter::new(Membership::<usize>::new().unwrap(), |value: &i64| {
        calls.set(calls.get() + 1);
        assert!(!fail.replace(false), "predicate failure");
        *value > 0
    });
    node.apply_row(&event(TupleChange::put(usize::MAX, 1i64)));
    fail.set(true);
    assert!(
        catch_unwind(AssertUnwindSafe(
            || node.apply_row(&event(TupleChange::put(usize::MAX, -1i64)))
        ))
        .is_err()
    );
    assert!(!node.poisoned);
    assert_eq!(
        node.apply_row(&event(TupleChange::<usize, i64>::retract(usize::MAX))),
        Some(TupleChange::retract(usize::MAX))
    );
    assert_eq!(calls.get(), 2);
}

#[test]
fn membership_filter_builder_keeps_custom_drop_payload_lifetimes_on_the_legacy_route() {
    type Trace = Rc<RefCell<Vec<(&'static str, i64)>>>;
    struct Row(i64, Trace);
    impl Clone for Row {
        fn clone(&self) -> Self {
            self.1.borrow_mut().push(("clone", self.0));
            Self(self.0, self.1.clone())
        }
    }
    impl Drop for Row {
        fn drop(&mut self) {
            self.1.borrow_mut().push(("drop", self.0));
        }
    }
    fn run(legacy: bool) -> Vec<(&'static str, i64)> {
        let trace = Rc::new(RefCell::new(Vec::new()));
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let source = b.source::<usize, Row>();
        if legacy {
            let node = FilterNode::<usize, Row>::new(|row: &Row| row.0 > 0);
            let out: StreamPort<usize, Row> = b.push(Filter(node));
            b.connect(source.0.node, out.node, false);
        } else {
            b.filter(source.port(), |row: &Row| row.0 > 0).unwrap();
        }
        let mut graph = b.build();
        {
            let mut trial = graph.transaction();
            trial
                .apply(&source, TupleChange::put(0usize, Row(-1, trace.clone())))
                .unwrap();
            trial.commit().unwrap();
        }
        {
            let mut trial = graph.transaction();
            trial
                .apply(&source, TupleChange::put(0usize, Row(-2, trace.clone())))
                .unwrap();
        }
        {
            let mut trial = graph.transaction();
            trial.apply(&source, TupleChange::retract(0usize)).unwrap();
            trial.commit().unwrap();
        }
        drop(graph);
        trace.borrow().clone()
    }
    assert_eq!(run(false), run(true));
}
