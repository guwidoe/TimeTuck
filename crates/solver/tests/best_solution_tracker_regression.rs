//! Original snapshot, tie, failure and real-solver observer regressions.
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;
#[test]
fn lazy_non_clone_snapshots_and_failures_preserve_retained_quality() {
    struct Snapshot(usize);
    let mut tracker = BestSolutionTracker::new(Snapshot(0), SimpleScore::new(-100), 1);
    for (count, score) in [(2, 1000), (1, -101), (1, -100)] {
        assert!(!tracker.consider(SimpleScore::new(score), count, || panic!(
            "rejected snapshot"
        )));
    }
    assert!(
        catch_unwind(AssertUnwindSafe(|| tracker.consider(
            SimpleScore::new(-200),
            0,
            || panic!("snapshot failure")
        )))
        .is_err()
    );
    assert_eq!(tracker.best().assignment.0, 0);
    assert_eq!(tracker.best().required_unassigned, 1);
    assert_eq!(tracker.best().score, SimpleScore::new(-100));
    assert!(tracker.consider(SimpleScore::new(-200), 0, || Snapshot(1)));
    assert!(!tracker.consider(SimpleScore::new(999), 1, || panic!(
        "complete best displaced"
    )));
    assert_eq!(tracker.into_best().assignment.0, 1);
}
#[derive(Debug)]
struct Counted {
    value: i32,
    clones: Rc<Cell<usize>>,
    fail: bool,
}
impl Clone for Counted {
    fn clone(&self) -> Self {
        self.clones.set(self.clones.get() + 1);
        assert!(!self.fail, "clone failure");
        Self {
            value: self.value,
            clones: self.clones.clone(),
            fail: self.fail,
        }
    }
}
#[test]
fn borrowed_candidates_clone_only_improvements_and_remain_independent() {
    let clones = Rc::new(Cell::new(0));
    let mut tracker = BestSolutionTracker::new(
        Counted {
            value: 0,
            clones: clones.clone(),
            fail: false,
        },
        10,
        0,
    );
    let mut candidate = Counted {
        value: 1,
        clones: clones.clone(),
        fail: false,
    };
    assert!(!tracker.consider_ref(&candidate, &10, 0));
    assert!(!tracker.consider_ref(&candidate, &100, 1));
    assert_eq!(clones.get(), 0);
    assert!(tracker.consider_ref(&candidate, &11, 0));
    assert_eq!(clones.get(), 1);
    candidate.value = 99;
    assert_eq!(tracker.best().assignment.value, 1);
    candidate.fail = true;
    assert!(catch_unwind(AssertUnwindSafe(|| tracker.consider_ref(&candidate, &12, 0))).is_err());
    assert_eq!(tracker.best().assignment.value, 1);
    assert_eq!(tracker.best().score, 11);
}
#[test]
fn real_solver_observers_retain_partial_and_complete_snapshots_in_both_backends() {
    for stream in [false, true] {
        for steps in [1, 3] {
            let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]); 3]).unwrap();
            let initial = p.initial_assignment();
            let mut tracker = BestSolutionTracker::new(initial, SimpleScore::ZERO, 3);
            let config = SolverSequence {
                phases: vec![SolverPhase::Construction(ConstructionConfig {
                    max_steps: Some(steps),
                    ..Default::default()
                })],
                ..Default::default()
            };
            let mut observer = |u: SolverUpdate<'_, Assignment<i32>, SimpleScore>| {
                let missing = p
                    .variables()
                    .iter()
                    .zip(u.assignment.values())
                    .filter(|(v, a)| !v.allows_unassigned() && a.is_none())
                    .count();
                tracker.consider_ref(u.assignment, &u.score, missing);
            };
            let result = if stream {
                solve_stream_with_observer(
                    &p,
                    || {
                        let mut b = StreamGraphBuilder::new();
                        let source = b.source::<usize, (usize, Option<i32>)>();
                        b.score("value", source.port(), |(_, v)| {
                            SimpleScore::new(i64::from(v.unwrap_or(0)))
                        })?;
                        Ok((b.build(), source))
                    },
                    config,
                    &mut observer,
                )
                .unwrap()
            } else {
                let mut n = ConstraintNetwork::new(3);
                for i in 0..3 {
                    n.add_constraint(format!("value-{i}"), vec![i], |v| {
                        SimpleScore::new(i64::from(v[0].copied().unwrap_or(0)))
                    })
                    .unwrap();
                }
                solve_with_observer(&p, &n, config, &mut observer).unwrap()
            };
            let best = tracker.into_best();
            assert_eq!(best.assignment, result.assignment);
            assert_eq!(Some(best.score), result.score);
            assert_eq!(best.required_unassigned, 3 - steps as usize);
            assert_eq!(result.complete, steps == 3);
        }
    }
}
