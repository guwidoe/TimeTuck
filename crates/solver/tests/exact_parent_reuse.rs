//! Original assignment ownership and cleanup contracts; no upstream credit.
use std::{
    cell::Cell,
    fmt,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;
const ORDERS: [NodeExploration; 4] = [
    NodeExploration::DepthFirst,
    NodeExploration::BreadthFirst,
    NodeExploration::ScoreFirst,
    NodeExploration::OptimisticBoundFirst,
];
#[derive(Default)]
struct Counts {
    clones: Cell<u64>,
    live: Cell<i64>,
    cancel_clone: Cell<Option<i64>>,
    cancel_drop: Cell<Option<i64>>,
    panic_clone: Cell<bool>,
    token: CancellationToken,
}
struct V {
    id: i64,
    c: Rc<Counts>,
    cloned: bool,
}
impl V {
    fn new(id: i64, c: &Rc<Counts>) -> Self {
        c.live.set(c.live.get() + 1);
        Self {
            id,
            c: c.clone(),
            cloned: false,
        }
    }
}
impl Clone for V {
    fn clone(&self) -> Self {
        assert!(!self.c.panic_clone.get(), "payload clone failure");
        self.c.clones.set(self.c.clones.get() + 1);
        self.c.live.set(self.c.live.get() + 1);
        if self.c.cancel_clone.get() == Some(self.id) {
            self.c.token.cancel();
        }
        Self {
            id: self.id,
            c: self.c.clone(),
            cloned: true,
        }
    }
}
impl Drop for V {
    fn drop(&mut self) {
        self.c.live.set(self.c.live.get() - 1);
        if self.cloned && self.c.cancel_drop.get() == Some(self.id) {
            self.c.token.cancel();
        }
    }
}
impl PartialEq for V {
    fn eq(&self, b: &Self) -> bool {
        self.id == b.id
    }
}
impl Eq for V {}
impl fmt::Debug for V {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.id.fmt(f)
    }
}
fn conf(order: NodeExploration) -> BranchAndBoundConfig<'static> {
    BranchAndBoundConfig {
        exploration: order,
        ..Default::default()
    }
}
#[test]
fn binary_and_single_child_trees_clone_only_siblings_and_selected_domain_values() {
    for order in ORDERS {
        for width in [1, 2] {
            let c = Rc::new(Counts::default());
            let p = FiniteProblem::new(
                (0..3)
                    .map(|_| Variable::new((0..width).map(|id| V::new(id, &c)).collect()))
                    .collect(),
            )
            .unwrap();
            let live = c.live.get();
            c.clones.set(0);
            let r = branch_and_bound(
                &p,
                |a, _| {
                    BranchEvaluation::unbounded(SimpleScore::new(
                        a.values().iter().flatten().map(|v| v.id).sum(),
                    ))
                },
                conf(order),
            )
            .unwrap();
            assert_eq!(
                r.best.as_ref().unwrap().score,
                SimpleScore::new(3 * (width - 1))
            );
            // Root has no values. Binary: 14 domain clones + 2*1 + 4*2 sibling
            // clones; the old all-cloned tree needed 34. A unary chain needs only 3.
            assert_eq!(c.clones.get(), if width == 2 { 24 } else { 3 });
            assert_eq!(r.evaluations, if width == 2 { 15 } else { 4 });
            drop(r);
            assert_eq!(c.live.get(), live);
            assert_eq!(p.initial_assignment().values().iter().flatten().count(), 0);
        }
    }
}
#[test]
fn list_and_mixed_owned_children_match_independent_exhaustive_optima() {
    for order in ORDERS {
        let c = Rc::new(Counts::default());
        let l = ListProblem::new(
            (1..=3).map(|id| V::new(id, &c)).collect(),
            vec![ListEntity::new(vec![])],
        )
        .unwrap();
        let live = c.live.get();
        c.clones.set(0);
        let r = branch_and_bound_lists(
            &l,
            |a, _| {
                BranchEvaluation::unbounded(SimpleScore::new(
                    a.lists()[0]
                        .iter()
                        .enumerate()
                        .map(|(i, v)| (i + 1) as i64 * v.id)
                        .sum(),
                ))
            },
            conf(order),
        )
        .unwrap();
        assert_eq!(c.clones.get(), 18);
        assert_eq!(r.evaluations, 10);
        let plain = ListProblem::new(vec![1i64, 2, 3], vec![ListEntity::new(vec![])]).unwrap();
        let expected = exhaustive_list_search(
            &plain,
            |a| {
                SimpleScore::new(
                    a.lists()[0]
                        .iter()
                        .enumerate()
                        .map(|(i, v)| (i + 1) as i64 * v)
                        .sum(),
                )
            },
            100,
        );
        assert_eq!(r.best.as_ref().unwrap().score, expected.best.unwrap().score);
        drop(r);
        assert_eq!(c.live.get(), live);
        let b =
            FiniteProblem::new(vec![Variable::new(vec![V::new(0, &c), V::new(1, &c)])]).unwrap();
        let l = ListProblem::new(
            vec![V::new(1, &c), V::new(2, &c)],
            vec![ListEntity::new(vec![])],
        )
        .unwrap();
        let m = MixedProblem::new(b, l);
        let live = c.live.get();
        c.clones.set(0);
        let r = branch_and_bound_mixed(
            &m,
            |a, _, _| {
                BranchEvaluation::unbounded(SimpleScore::new(
                    a.basic().values()[0].as_ref().map_or(0, |v| v.id)
                        + a.lists().lists()[0]
                            .iter()
                            .enumerate()
                            .map(|(i, v)| (i + 1) as i64 * v.id)
                            .sum::<i64>(),
                ))
            },
            conf(order),
        )
        .unwrap();
        assert_eq!(c.clones.get(), 12);
        assert_eq!(r.best.as_ref().unwrap().score, SimpleScore::new(6));
        assert_eq!(r.evaluations, 8);
        drop(r);
        assert_eq!(c.live.get(), live);
    }
}
#[test]
fn cancellation_from_required_clone_and_discarded_leaf_drop_keeps_polling_boundary() {
    for order in ORDERS {
        let c = Rc::new(Counts::default());
        let p = FiniteProblem::new(
            (0..2)
                .map(|_| Variable::new(vec![V::new(0, &c), V::new(1, &c)]))
                .collect(),
        )
        .unwrap();
        let live = c.live.get();
        c.cancel_clone.set(Some(1));
        let r = branch_and_bound(
            &p,
            |_, _| BranchEvaluation::unbounded(SimpleScore::ZERO),
            BranchAndBoundConfig {
                cancellation: Some(&c.token),
                ..conf(order)
            },
        )
        .unwrap();
        assert_eq!(r.termination, BranchTermination::Cancelled);
        assert_eq!(r.evaluations, 3);
        assert!(r.best.is_none());
        drop(r);
        assert_eq!(c.live.get(), live);
        let c = Rc::new(Counts::default());
        let p =
            FiniteProblem::new(vec![Variable::new(vec![V::new(0, &c), V::new(1, &c)])]).unwrap();
        let live = c.live.get();
        c.cancel_drop.set(Some(0));
        let r = branch_and_bound(
            &p,
            |_, _| BranchEvaluation::unbounded(SimpleScore::ZERO.with_structural(-1)),
            BranchAndBoundConfig {
                cancellation: Some(&c.token),
                ..conf(order)
            },
        )
        .unwrap();
        assert_eq!(r.termination, BranchTermination::Cancelled);
        assert_eq!(r.evaluations, 2);
        assert!(r.best.is_none());
        drop(r);
        assert_eq!(c.live.get(), live);
    }
}
#[test]
fn callback_clone_and_observer_panics_drop_owned_search_values_without_changing_model() {
    for failure in 0..3 {
        let c = Rc::new(Counts::default());
        let p = FiniteProblem::new(vec![Variable::new(vec![V::new(1, &c)]); 3]).unwrap();
        let live = c.live.get();
        let calls = Cell::new(0);
        let r = catch_unwind(AssertUnwindSafe(|| {
            branch_and_bound_with_observer(
                &p,
                |_, remaining| {
                    calls.set(calls.get() + 1);
                    if failure == 0 && remaining.len() == 1 {
                        panic!("score panic");
                    }
                    if failure == 1 && remaining.len() == 1 {
                        c.panic_clone.set(true);
                    }
                    BranchEvaluation::unbounded(SimpleScore::ZERO)
                },
                BranchAndBoundConfig::default(),
                |_| {
                    if failure == 2 {
                        panic!("observer panic");
                    }
                },
            )
        }));
        assert!(r.is_err());
        assert_eq!(c.live.get(), live);
        c.panic_clone.set(false);
        let r = branch_and_bound(
            &p,
            |_, _| BranchEvaluation::unbounded(SimpleScore::ZERO),
            BranchAndBoundConfig::default(),
        )
        .unwrap();
        assert_eq!(r.evaluations, 4);
        drop(r);
        assert_eq!(c.live.get(), live);
    }
}
