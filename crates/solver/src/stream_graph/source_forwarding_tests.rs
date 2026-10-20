//! Original old-loop differential tests; no upstream source-method credit.
use super::*;
use crate::SimpleScore;
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
};

#[derive(Debug)]
struct Row {
    value: i64,
    trace: Rc<RefCell<Vec<(String, i64)>>>,
    clones_left: Rc<Cell<Option<usize>>>,
}
impl Clone for Row {
    fn clone(&self) -> Self {
        self.trace.borrow_mut().push(("clone".into(), self.value));
        if let Some(left) = self.clones_left.get() {
            assert!(left > 0, "row clone panic");
            self.clones_left.set(Some(left - 1));
        }
        Self {
            value: self.value,
            trace: self.trace.clone(),
            clones_left: self.clones_left.clone(),
        }
    }
}
impl Drop for Row {
    fn drop(&mut self) {
        self.trace.borrow_mut().push(("drop".into(), self.value));
    }
}

fn reference(
    g: &mut StreamGraph<'_, SimpleScore>,
    source: &StreamSource<usize, Row>,
    change: TupleChange<usize, Row>,
    commit: bool,
) -> Result<u128, GraphError> {
    let mut tx = g.transaction();
    tx.valid()?;
    tx.failed = true;
    tx.graph.validate(source)?;
    tx.drain(VecDeque::from([(
        Edge {
            node: source.0.node,
            right: false,
        },
        Event::new(change),
    )]))?;
    tx.failed = false;
    let count = tx.score_evaluations();
    if commit {
        tx.commit()?;
    }
    Ok(count)
}

#[derive(Debug, PartialEq)]
struct Outcome {
    result: String,
    trace: Vec<(String, i64)>,
    rows: Vec<(usize, i64)>,
    score: SimpleScore,
}
fn run(
    fast: bool,
    observed: bool,
    value: Option<i64>,
    id: usize,
    commit: bool,
    fail: Option<usize>,
) -> Outcome {
    let trace = Rc::new(RefCell::new(Vec::new()));
    let clones_left = Rc::new(Cell::new(None));
    let row = |value| Row {
        value,
        trace: trace.clone(),
        clones_left: clones_left.clone(),
    };
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, Row>();
    if observed {
        let filter_trace = trace.clone();
        let filtered = b
            .filter(source.port(), move |v| {
                filter_trace.borrow_mut().push(("filter".into(), v.value));
                true
            })
            .unwrap();
        let direct_trace = trace.clone();
        b.score("direct", source.port(), move |v| {
            direct_trace.borrow_mut().push(("direct".into(), v.value));
            SimpleScore::new(v.value)
        })
        .unwrap();
        let last_trace = trace.clone();
        b.score("filtered", &filtered, move |v| {
            last_trace.borrow_mut().push(("filtered".into(), v.value));
            assert_ne!(v.value, 99, "callback panic");
            SimpleScore::new(v.value).with_structural(i64::from(v.value == 98))
        })
        .unwrap();
    }
    let mut g = b.build();
    reference(&mut g, &source, TupleChange::put(0, row(1)), true).unwrap();
    trace.borrow_mut().clear();
    clones_left.set(fail);
    let change = if let Some(value) = value {
        TupleChange::put(id, row(value))
    } else {
        TupleChange::retract(id)
    };
    let result = catch_unwind(AssertUnwindSafe(|| {
        if fast {
            let mut tx = g.transaction();
            tx.apply(&source, change)?;
            let work = tx.score_evaluations();
            if commit {
                tx.commit()?;
            }
            Ok(work)
        } else {
            reference(&mut g, &source, change, commit)
        }
    }));
    clones_left.set(None);
    let result = match result {
        Ok(r) => format!("{r:?}"),
        Err(_) => "panic".into(),
    };
    let rows = g
        .source_rows(&source)
        .unwrap()
        .iter()
        .map(|(i, r)| (*i, r.value))
        .collect();
    let score = g.score();
    let captured = trace.borrow().clone();
    g.assert_consistent().unwrap();
    Outcome {
        result,
        trace: captured,
        rows,
        score,
    }
}

#[test]
fn direct_source_forwarding_matches_old_root_loop_clone_drop_and_callback_order() {
    for observed in [false, true] {
        for commit in [false, true] {
            for (id, value) in [
                (0, Some(2)),
                (1, Some(2)),
                (0, None),
                (9, None),
                (0, Some(98)),
                (0, Some(99)),
            ] {
                assert_eq!(
                    run(true, observed, value, id, commit, None),
                    run(false, observed, value, id, commit, None),
                    "observed={observed} commit={commit} id={id} value={value:?}"
                );
            }
            for fail in 0..6 {
                assert_eq!(
                    run(true, observed, Some(2), 0, commit, Some(fail)),
                    run(false, observed, Some(2), 0, commit, Some(fail)),
                    "observed={observed} commit={commit} clone failure={fail}"
                );
            }
        }
    }
}
