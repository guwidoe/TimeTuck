//! Original indexed grouping regressions; no upstream source-method credit.
use std::{
    hash::{Hash, Hasher},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};
use timetuck::*;
#[derive(Clone, Debug)]
struct Value {
    id: usize,
    payload: usize,
    collision: bool,
    hashes: Arc<AtomicUsize>,
    panic: Arc<AtomicBool>,
}
impl PartialEq for Value {
    fn eq(&self, rhs: &Self) -> bool {
        self.id == rhs.id
    }
}
impl Eq for Value {}
impl Hash for Value {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.hashes.fetch_add(1, Ordering::Relaxed);
        assert!(!self.panic.load(Ordering::Relaxed), "hash panic");
        if self.collision {
            0usize.hash(h)
        } else {
            self.id.hash(h)
        }
    }
}
fn payloads(m: &Move<Value>, out: &mut Vec<(usize, Option<(usize, usize)>)>) {
    match m {
        Move::Change { variable, value } => {
            out.push((*variable, value.as_ref().map(|v| (v.id, v.payload))))
        }
        Move::Composite(children) => {
            for c in children {
                payloads(c, out)
            }
        }
        Move::Swap { .. } => panic!("pillar changes expected"),
    }
}
fn state(a: &Assignment<Value>) -> Vec<Option<(usize, usize)>> {
    a.values()
        .iter()
        .map(|v| v.as_ref().map(|v| (v.id, v.payload)))
        .collect()
}
#[test]
fn groups_and_provider_trials_preserve_payloads_after_commits_and_reconstruction() {
    for collision in [false, true] {
        let hashes = Arc::new(AtomicUsize::new(0));
        let panic = Arc::new(AtomicBool::new(false));
        let value = |id, payload| Value {
            id,
            payload,
            collision,
            hashes: hashes.clone(),
            panic: panic.clone(),
        };
        let vars: Vec<_> = (0..300)
            .map(|i| {
                Variable::new(vec![value(i % 100, 1), value(999, 2)])
                    .with_initial(value(i % 100, 1000 + i))
                    .allowing_unassigned()
                    .with_pinned(i % 61 == 0)
            })
            .collect();
        let plain = FiniteProblem::new(vars.clone()).unwrap();
        let indexed = FiniteProblem::new_indexed(vars).unwrap();
        let a = plain.initial_assignment();
        hashes.store(0, Ordering::Relaxed);
        assert_eq!(
            pillar_groups(&plain, &a).unwrap(),
            pillar_groups(&indexed, &a).unwrap()
        );
        assert!(hashes.load(Ordering::Relaxed) > 0);
        let provider = PillarMoveProvider {
            swaps: false,
            ..Default::default()
        };
        let left: Vec<_> = provider.select(&plain, &a).unwrap().collect();
        let right: Vec<_> = provider.select(&indexed, &a).unwrap().collect();
        assert_eq!(left, right);
        for (l, r) in left.iter().zip(&right) {
            let mut lp = vec![];
            let mut rp = vec![];
            payloads(l, &mut lp);
            payloads(r, &mut rp);
            assert_eq!(lp, rp);
        }
        let mut net = ConstraintNetwork::new(300);
        for i in 0..300 {
            net.add_constraint(format!("v{i}"), vec![i], |v| {
                SimpleScore::new(v[0].map_or(0, |v: &Value| v.id as i64))
            })
            .unwrap();
        }
        let mut d = ScoreDirector::new(&indexed, &net, a.clone()).unwrap();
        let before = state(&a);
        for m in right.iter().take(12) {
            let trial = d.apply(m).unwrap();
            let score: i64 = trial
                .assignment()
                .values()
                .iter()
                .filter_map(|v| v.as_ref())
                .map(|v| v.id as i64)
                .sum();
            assert_eq!(trial.score(), SimpleScore::new(score));
        }
        assert_eq!(state(d.assignment()), before);
        d.assert_consistent().unwrap();
        d.apply(&right[0]).unwrap().commit();
        assert_eq!(
            pillar_groups(&plain, d.assignment()).unwrap(),
            pillar_groups(&indexed, d.assignment()).unwrap()
        );
        d.assert_consistent().unwrap();
        // A rebuilt model must retain the opt-in grouping capability and metadata.
        let selected: Vec<_> = (0..300).filter(|i| i % 61 != 0).collect();
        let plan = BasicRuinPlan::new(&indexed, d.assignment(), &selected).unwrap();
        hashes.store(0, Ordering::Relaxed);
        assert_eq!(
            pillar_groups(plan.repair_problem(), d.assignment()).unwrap(),
            pillar_groups(&indexed, d.assignment()).unwrap()
        );
        assert!(hashes.load(Ordering::Relaxed) > 0);
        let resumed = indexed.with_initial_assignment(d.assignment()).unwrap();
        assert_eq!(
            pillar_groups(&resumed, &resumed.initial_assignment()).unwrap(),
            pillar_groups(&indexed, d.assignment()).unwrap()
        );
    }
}
#[test]
fn hash_panics_leave_the_snapshot_and_future_openings_usable() {
    let hashes = Arc::new(AtomicUsize::new(0));
    let panic = Arc::new(AtomicBool::new(false));
    let vars = (0..320)
        .map(|id| {
            let v = Value {
                id,
                payload: id + 100,
                collision: false,
                hashes: hashes.clone(),
                panic: panic.clone(),
            };
            Variable::new(vec![v.clone()]).with_initial(v)
        })
        .collect();
    let p = FiniteProblem::new_indexed(vars).unwrap();
    let a = p.initial_assignment();
    let before = state(&a);
    panic.store(true, Ordering::Relaxed);
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| pillar_groups(&p, &a))).is_err()
    );
    assert_eq!(state(&a), before);
    panic.store(false, Ordering::Relaxed);
    assert_eq!(pillar_groups(&p, &a).unwrap().len(), 320);
    // A single large pillar never pays for hashing despite the number of rows.
    let a = Assignment::new(vec![None; 320]);
    hashes.store(0, Ordering::Relaxed);
    assert_eq!(pillar_groups(&p, &a).unwrap().len(), 1);
    assert_eq!(hashes.load(Ordering::Relaxed), 0);
}
