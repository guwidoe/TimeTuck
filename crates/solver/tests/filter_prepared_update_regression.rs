//! Original ordinary-filter ownership and failure contracts; no upstream credit.
use std::{
    cell::{Cell, RefCell},
    cmp::Ordering,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::{FilterNode, TupleChange};

type Trace = Rc<RefCell<Vec<String>>>;
struct Key {
    ordinal: usize,
    tag: &'static str,
    trace: Trace,
}
impl Clone for Key {
    fn clone(&self) -> Self {
        self.trace
            .borrow_mut()
            .push(format!("clone key {}", self.tag));
        Self {
            ordinal: self.ordinal,
            tag: self.tag,
            trace: self.trace.clone(),
        }
    }
}
impl Drop for Key {
    fn drop(&mut self) {
        self.trace
            .borrow_mut()
            .push(format!("drop key {}", self.tag));
    }
}
impl PartialEq for Key {
    fn eq(&self, other: &Self) -> bool {
        self.ordinal == other.ordinal
    }
}
impl Eq for Key {}
impl PartialOrd for Key {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Key {
    fn cmp(&self, other: &Self) -> Ordering {
        self.ordinal.cmp(&other.ordinal)
    }
}
struct Value {
    tag: &'static str,
    accepted: bool,
    fail_clone: Rc<Cell<bool>>,
    trace: Trace,
}
impl Clone for Value {
    fn clone(&self) -> Self {
        self.trace
            .borrow_mut()
            .push(format!("clone value {}", self.tag));
        assert!(!self.fail_clone.replace(false), "payload clone failure");
        Self {
            tag: self.tag,
            accepted: self.accepted,
            fail_clone: self.fail_clone.clone(),
            trace: self.trace.clone(),
        }
    }
}
impl Drop for Value {
    fn drop(&mut self) {
        self.trace
            .borrow_mut()
            .push(format!("drop value {}", self.tag));
    }
}

#[test]
fn replacements_clone_outputs_before_dropping_input_keys_and_retained_payloads() {
    let trace = Trace::default();
    let fail_clone = Rc::new(Cell::new(false));
    let key = |tag| Key {
        ordinal: 7,
        tag,
        trace: trace.clone(),
    };
    let value = |tag, accepted| Value {
        tag,
        accepted,
        fail_clone: fail_clone.clone(),
        trace: trace.clone(),
    };
    let mut node = FilterNode::new(|value: &Value| {
        value
            .trace
            .borrow_mut()
            .push(format!("predicate {}", value.tag));
        value.accepted
    });
    drop(node.apply(TupleChange::put(key("canonical"), value("old", true))));
    trace.borrow_mut().clear();
    let output = node.apply(TupleChange::put(key("incoming"), value("new", true)));
    assert_eq!(
        &*trace.borrow(),
        &[
            "predicate new",
            "clone key incoming",
            "clone value new",
            "drop key incoming",
            "drop value old",
        ]
    );
    let (stored_key, stored_value) = node.rows().next().unwrap();
    assert_eq!((stored_key.tag, stored_value.tag), ("canonical", "new"));
    drop(output);
    trace.borrow_mut().clear();
    let output = node.apply(TupleChange::put(key("reject"), value("rejected", false)));
    assert_eq!(
        &*trace.borrow(),
        &[
            "predicate rejected",
            "clone key reject",
            "drop key reject",
            "drop value new",
        ]
    );
    assert!(output.as_ref().unwrap().value.is_none());
    assert_eq!(node.rows().count(), 0);
    drop(output);
    trace.borrow_mut().clear();
    drop(node);
    let observed = trace.borrow();
    assert!(observed.iter().any(|event| event == "drop value rejected"));
    assert!(observed.iter().any(|event| event == "drop key canonical"));
}

#[test]
fn failed_output_clone_keeps_original_row_usable_and_retraction_skips_predicate() {
    let trace = Trace::default();
    let fail_clone = Rc::new(Cell::new(false));
    let key = |tag| Key {
        ordinal: 7,
        tag,
        trace: trace.clone(),
    };
    let value = |tag| Value {
        tag,
        accepted: true,
        fail_clone: fail_clone.clone(),
        trace: trace.clone(),
    };
    let calls = Cell::new(0);
    let mut node = FilterNode::new(|_: &Value| {
        calls.set(calls.get() + 1);
        true
    });
    drop(node.apply(TupleChange::put(key("canonical"), value("old"))));
    fail_clone.set(true);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            node.apply(TupleChange::put(key("failing"), value("failed")))
        }))
        .is_err()
    );
    assert!(!node.is_poisoned());
    assert_eq!(node.rows().next().unwrap().1.tag, "old");
    let before = calls.get();
    let output = node.apply(TupleChange::retract(key("retract")));
    assert!(output.unwrap().value.is_none());
    assert_eq!(calls.get(), before);
    assert_eq!(node.rows().count(), 0);
}
