//! Original public panic-preflight regressions; no upstream-method credit.
use std::{
    cell::Cell,
    cmp::Ordering,
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::{ReversibleCollector, SnapshotSet, SortedSetCollector};

#[derive(Default)]
struct Control {
    // None means comparisons are unrestricted; Some(n) permits n then panics.
    remaining: Cell<Option<usize>>,
    clone_panics: Cell<bool>,
}
struct Value {
    key: i32,
    payload: &'static str,
    control: Rc<Control>,
}
impl Value {
    fn new(key: i32, payload: &'static str, control: &Rc<Control>) -> Self {
        Self {
            key,
            payload,
            control: control.clone(),
        }
    }
}
impl Clone for Value {
    fn clone(&self) -> Self {
        assert!(
            !self.control.clone_panics.get(),
            "injected payload clone panic"
        );
        Self::new(self.key, self.payload, &self.control)
    }
}
impl Ord for Value {
    fn cmp(&self, other: &Self) -> Ordering {
        if let Some(remaining) = self.control.remaining.get() {
            assert!(remaining != 0, "injected comparison panic");
            self.control.remaining.set(Some(remaining - 1));
        }
        self.key.cmp(&other.key)
    }
}
impl PartialOrd for Value {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl Eq for Value {}

type Model = BTreeMap<i32, (&'static str, usize)>;
fn fixture(control: &Rc<Control>, old_count: usize) -> (SortedSetCollector<Value>, Model) {
    let mut collector = SortedSetCollector::default();
    let mut model = Model::new();
    // Balanced and off-center search paths, with a two-child old key.
    for key in [16, 8, 24, 4, 12, 20, 28, 2, 6, 10, 14, 18, 22, 26, 30] {
        let count = if key == 16 {
            old_count
        } else if key == 22 {
            3
        } else {
            1
        };
        for _ in 0..count {
            collector.insert(&Value::new(key, "initial", control));
        }
        model.insert(key, ("initial", count));
    }
    (collector, model)
}
fn rows(set: &SnapshotSet<Value>) -> Vec<(i32, &'static str)> {
    set.iter().map(|v| (v.key, v.payload)).collect()
}
fn assert_membership(collector: &SortedSetCollector<Value>, model: &Model) {
    assert_eq!(
        rows(&collector.finish()),
        model.iter().map(|(&k, &(p, _))| (k, p)).collect::<Vec<_>>()
    );
}
fn drain(mut collector: SortedSetCollector<Value>, model: &Model, control: &Rc<Control>) {
    assert_membership(&collector, model);
    for (&key, &(payload, count)) in model {
        let lookup = Value::new(key, "captured lookup", control);
        for remaining in (0..count).rev() {
            assert!(collector.remove(&lookup));
            let set = collector.finish();
            assert_eq!(set.contains(&lookup), remaining != 0);
            if remaining != 0 {
                assert_eq!(set.iter().find(|v| v.key == key).unwrap().payload, payload);
            }
        }
        assert!(!collector.remove(&lookup));
    }
    assert!(collector.finish().is_empty());
}
fn changed(mut model: Model, new_key: i32) -> Model {
    let old = model.get_mut(&16).unwrap();
    old.1 -= 1;
    if old.1 == 0 {
        model.remove(&16);
    }
    model
        .entry(new_key)
        .and_modify(|entry| entry.1 += 1)
        .or_insert(("replacement", 1));
    model
}
fn comparison_budget_sweep(old_count: usize, new_key: i32, shared: bool) {
    let mut saw_panic = false;
    let mut saw_success = false;
    // A finite guard diagnoses accidental nontermination; this is not a timing assertion.
    for budget in 0..512 {
        let control = Rc::new(Control::default());
        let (mut collector, before) = fixture(&control, old_count);
        let saved = shared.then(|| (collector.clone(), collector.finish()));
        let old = Value::new(16, "captured lookup", &control);
        let new = Value::new(new_key, "replacement", &control);
        control.remaining.set(Some(budget));
        let result = catch_unwind(AssertUnwindSafe(|| collector.replace(&old, &new)));
        control.remaining.set(None);
        let expected = match result {
            Ok(found) => {
                assert!(found);
                saw_success = true;
                changed(before.clone(), new_key)
            }
            Err(_) => {
                saw_panic = true;
                before.clone()
            }
        };
        assert_membership(&collector, &expected);
        if let Some((saved_collector, snapshot)) = saved {
            assert_eq!(
                rows(&snapshot),
                before
                    .iter()
                    .map(|(&k, &(p, _))| (k, p))
                    .collect::<Vec<_>>()
            );
            drain(saved_collector, &before, &control);
            // Mutating the saved collector must not change its saved output.
            assert_eq!(
                rows(&snapshot),
                before
                    .iter()
                    .map(|(&k, &(p, _))| (k, p))
                    .collect::<Vec<_>>()
            );
        }
        drain(collector, &expected, &control);
        if saw_success {
            break;
        }
    }
    assert!(
        saw_panic && saw_success,
        "must cover every successive comparison through first success"
    );
}

#[test]
fn prepared_duplicate_old_existing_destination_comparison_panics_are_atomic() {
    for shared in [false, true] {
        comparison_budget_sweep(4, 22, shared);
    }
}
#[test]
fn prepared_duplicate_old_absent_destination_comparison_panics_are_atomic() {
    for shared in [false, true] {
        comparison_budget_sweep(4, 29, shared);
    }
}
#[test]
fn prepared_singleton_old_existing_destination_comparison_panics_are_atomic() {
    for shared in [false, true] {
        comparison_budget_sweep(1, 22, shared);
    }
}
#[test]
fn equal_class_singleton_fallback_comparison_panics_preserve_representative() {
    for shared in [false, true] {
        comparison_budget_sweep(1, 16, shared);
    }
}
#[test]
fn absent_destination_clone_panics_after_lookup_preparation_preserve_every_count() {
    for shared in [false, true] {
        for old_count in [1, 4] {
            let control = Rc::new(Control::default());
            let (mut collector, before) = fixture(&control, old_count);
            let saved = shared.then(|| (collector.clone(), collector.finish()));
            let old = Value::new(16, "captured lookup", &control);
            let new = Value::new(29, "replacement", &control);
            control.clone_panics.set(true);
            assert!(catch_unwind(AssertUnwindSafe(|| collector.replace(&old, &new))).is_err());
            control.clone_panics.set(false);
            assert_membership(&collector, &before);
            if let Some((saved_collector, snapshot)) = saved {
                assert_eq!(rows(&snapshot), rows(&collector.finish()));
                drain(saved_collector, &before, &control);
            }
            drain(collector, &before, &control);
        }
    }
}
#[test]
fn existing_destination_prepared_paths_never_clone_payloads() {
    for shared in [false, true] {
        for old_count in [1, 4] {
            let control = Rc::new(Control::default());
            let (mut collector, before) = fixture(&control, old_count);
            let saved = shared.then(|| (collector.clone(), collector.finish()));
            let old = Value::new(16, "captured lookup", &control);
            let new = Value::new(22, "replacement", &control);
            control.clone_panics.set(true);
            assert!(collector.replace(&old, &new));
            control.clone_panics.set(false);
            let expected = changed(before.clone(), 22);
            assert_membership(&collector, &expected);
            assert_eq!(
                collector
                    .finish()
                    .iter()
                    .find(|v| v.key == 22)
                    .unwrap()
                    .payload,
                "initial"
            );
            if let Some((saved_collector, snapshot)) = saved {
                assert_eq!(
                    rows(&snapshot),
                    before
                        .iter()
                        .map(|(&k, &(p, _))| (k, p))
                        .collect::<Vec<_>>()
                );
                drain(saved_collector, &before, &control);
            }
            drain(collector, &expected, &control);
        }
    }
}
