//! Original operation-count and unwind regressions for diversified history updates.
use std::{
    cell::Cell,
    cmp::Ordering,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;
#[derive(Debug, Default)]
struct Control {
    operations: Cell<usize>,
    fail_at: Cell<Option<usize>>,
}
impl Control {
    fn operation(&self) {
        let next = self.operations.get() + 1;
        self.operations.set(next);
        assert_ne!(self.fail_at.get(), Some(next), "score operation failed");
    }
}
#[derive(Debug)]
struct Probe {
    value: i64,
    control: Rc<Control>,
}
impl Clone for Probe {
    fn clone(&self) -> Self {
        self.control.operation();
        Self {
            value: self.value,
            control: self.control.clone(),
        }
    }
}
impl PartialEq for Probe {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}
impl Eq for Probe {}
impl PartialOrd for Probe {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Probe {
    fn cmp(&self, other: &Self) -> Ordering {
        self.control.operation();
        self.value.cmp(&other.value)
    }
}
impl Score for Probe {
    type Level = i64;
    const HARD_LEVEL_COUNT: usize = 0;
    fn ordinary_levels(&self) -> impl Iterator<Item = i64> {
        [self.value].into_iter()
    }
    fn structural(&self) -> i64 {
        0
    }
    fn hard_cmp(&self, _: &Self) -> Ordering {
        Ordering::Equal
    }
    fn is_feasible(&self) -> bool {
        true
    }
}
fn probe(value: i64, control: &Rc<Control>) -> Probe {
    Probe {
        value,
        control: control.clone(),
    }
}
fn snapshot(a: &DiversifiedLateAcceptance<Probe>) -> (Vec<i64>, usize, i64, usize) {
    (
        a.history().iter().map(|s| s.value).collect(),
        a.cursor(),
        a.worst_score().value,
        a.worst_occurrences(),
    )
}
#[test]
fn replacements_preserving_a_minimum_do_constant_comparison_work() {
    for size in [2, 400, 40_000] {
        let c = Rc::new(Control::default());
        let mut a = DiversifiedLateAcceptance::new(size, probe(0, &c)).unwrap();
        c.operations.set(0);
        assert!(a.accepts(probe(0, &c), probe(5, &c)));
        assert!(
            c.operations.get() <= 10,
            "{} operations for {size} slots",
            c.operations.get()
        );
        assert_eq!(a.worst_score().value, 0);
        assert_eq!(a.worst_occurrences(), size - 1);
        c.operations.set(0);
        assert!(a.accepts(probe(3, &c), probe(3, &c)));
        assert!(c.operations.get() <= 10);
        assert_eq!(a.worst_occurrences(), size - 1);
    }
}
fn scenario(kind: usize, c: &Rc<Control>) -> (DiversifiedLateAcceptance<Probe>, i64, i64) {
    let mut a = DiversifiedLateAcceptance::new(4, probe(0, c)).unwrap();
    match kind {
        0 => (a, 0, 5),
        1 => (a, -5, -5),
        2 | 4 => {
            a.accepts(probe(0, c), probe(5, c));
            if kind == 2 { (a, 0, 0) } else { (a, 3, 3) }
        }
        3 => {
            for _ in 0..3 {
                a.accepts(probe(0, c), probe(5, c));
                a.step_ended();
            }
            (a, 0, 5)
        }
        _ => unreachable!(),
    }
}
#[test]
fn comparison_and_clone_panics_leave_every_history_branch_retryable() {
    for kind in 0..5 {
        let c = Rc::new(Control::default());
        let (mut successful, current, candidate) = scenario(kind, &c);
        c.operations.set(0);
        assert!(successful.accepts(probe(current, &c), probe(candidate, &c)));
        let operations = c.operations.get();
        let expected = snapshot(&successful);
        for fail_at in 1..=operations {
            let c = Rc::new(Control::default());
            let (mut a, current, candidate) = scenario(kind, &c);
            let before = snapshot(&a);
            c.operations.set(0);
            c.fail_at.set(Some(fail_at));
            assert!(
                catch_unwind(AssertUnwindSafe(
                    || a.accepts(probe(current, &c), probe(candidate, &c))
                ))
                .is_err()
            );
            assert_eq!(snapshot(&a), before, "kind {kind}, operation {fail_at}");
            c.fail_at.set(None);
            assert!(a.accepts(probe(current, &c), probe(candidate, &c)));
            assert_eq!(snapshot(&a), expected);
        }
    }
}
#[test]
fn long_histories_match_a_full_scan_reference_through_repeated_minimum_eviction() {
    for size in [1, 2, 17, 400] {
        let mut a = DiversifiedLateAcceptance::new(size, SimpleScore::new(0)).unwrap();
        let mut rows = vec![0i64; size];
        let mut cursor = 0;
        let mut seed = 81731u64;
        for step in 0..(size * 5 + 1000) {
            for trial in 0..5 {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                let current = step as i64 - 600;
                let candidate = if trial == 0 {
                    current
                } else {
                    current + ((seed >> 32) % 401) as i64 - 200
                };
                let accepted = candidate == current || candidate > *rows.iter().min().unwrap();
                if accepted
                    && (candidate < rows[cursor]
                        || (candidate > rows[cursor] && candidate > current))
                {
                    rows[cursor] = candidate;
                }
                assert_eq!(
                    a.accepts(SimpleScore::new(current), SimpleScore::new(candidate)),
                    accepted
                );
                let minimum = *rows.iter().min().unwrap();
                assert_eq!(a.worst_score(), &SimpleScore::new(minimum));
                assert_eq!(
                    a.worst_occurrences(),
                    rows.iter().filter(|&&v| v == minimum).count()
                );
            }
            assert_eq!(
                a.history()
                    .iter()
                    .map(|s| s.ordinary_levels().next().unwrap())
                    .collect::<Vec<_>>(),
                rows
            );
            a.step_ended();
            cursor = (cursor + 1) % size;
            assert_eq!(a.cursor(), cursor);
        }
    }
}
