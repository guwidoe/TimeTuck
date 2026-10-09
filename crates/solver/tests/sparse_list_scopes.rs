//! Original regressions for identity work in sparse list and mixed networks.
use std::{cell::Cell, rc::Rc};
use timetuck::*;

#[derive(Clone, Debug)]
struct CountedValue(usize, Rc<Cell<usize>>);
impl PartialEq for CountedValue {
    fn eq(&self, other: &Self) -> bool {
        self.1.set(self.1.get() + 1);
        self.0 == other.0
    }
}
impl Eq for CountedValue {}
const N: usize = 256;
fn problem(count: &Rc<Cell<usize>>) -> ListProblem<CountedValue> {
    let values: Vec<_> = (0..N).map(|v| CountedValue(v, count.clone())).collect();
    ListProblem::new(
        values.clone(),
        vec![
            ListEntity::new(values[..N / 2].to_vec()),
            ListEntity::new(values[N / 2..].to_vec()),
        ],
    )
    .unwrap()
}
fn swap() -> ListMove<CountedValue> {
    ListMove::Swap {
        left: ListPosition {
            entity: 0,
            index: 0,
        },
        right: ListPosition {
            entity: 1,
            index: 0,
        },
    }
}

macro_rules! check_sparse {
    ($problem:expr, $network:expr, $scope:path, $fact:path, $director:path, $movement:expr) => {{
        let count = Rc::new(Cell::new(0));
        let p = ($problem)(&count);
        let calls = Cell::new(0);
        let mut network = $network;
        // With no value constraints, move validation needs no value equality.
        let mut d = $director(&p, &network, p.initial_assignment()).unwrap();
        count.set(0);
        drop(d.apply(&$movement).unwrap());
        assert_eq!(count.get(), 0);
        drop(d);
        // Out-of-order registrations and repeated scopes must neither lose nor
        // duplicate updates; disabled constraints can be enabled in a new session.
        network
            .add_weighted_constraint(
                "sparse",
                vec![$scope(200), $scope(0), $scope(200)],
                SimpleScore::ZERO,
                |facts| {
                    calls.set(calls.get() + 1);
                    facts
                        .iter()
                        .map(|fact| {
                            let $fact {
                                element: Some(view),
                                ..
                            } = fact
                            else {
                                panic!("assigned value")
                            };
                            (1000 * view.position.entity + view.position.index) as i64
                        })
                        .sum()
                },
            )
            .unwrap();
        let mut d = $director(&p, &network, p.initial_assignment()).unwrap();
        assert_eq!(calls.get(), 0);
        drop(d.apply(&$movement).unwrap());
        assert_eq!(calls.get(), 0);
        drop(d);
        let overrides = [("sparse".to_owned(), SimpleScore::new(1))]
            .into_iter()
            .collect();
        network.apply_weight_overrides(&overrides).unwrap();
        let mut d = $director(&p, &network, p.initial_assignment()).unwrap();
        assert_eq!(d.score(), SimpleScore::new(2144));
        assert_eq!(calls.get(), 1);
        count.set(0);
        {
            let tx = d.apply(&$movement).unwrap();
            assert_eq!(tx.score(), SimpleScore::new(3144));
            // A linear bound excludes scanning all N identities over the lists.
            assert!(count.get() < 12 * N, "{} comparisons", count.get());
            assert_eq!(calls.get(), 2);
        }
        assert_eq!(d.score(), SimpleScore::new(2144));
        d.apply(&$movement).unwrap().commit();
        assert_eq!(d.score(), SimpleScore::new(3144));
        d.assert_consistent().unwrap();
    }};
}
#[test]
fn list_scopes_skip_unreferenced_identities_and_preserve_reactivation() {
    check_sparse!(
        problem,
        ListConstraintNetwork::<CountedValue, SimpleScore>::new(2, N),
        ListScope::Value,
        ListFact::Value,
        ListScoreDirector::new,
        swap()
    );
}
#[test]
fn mixed_scopes_skip_unreferenced_identities_and_preserve_reactivation() {
    check_sparse!(
        |count| MixedProblem::new(FiniteProblem::<i32>::new(vec![]).unwrap(), problem(count)),
        MixedConstraintNetwork::<i32, CountedValue, SimpleScore>::new(0, 2, N),
        MixedScope::ListValue,
        MixedFact::ListValue,
        MixedScoreDirector::new,
        MixedMove {
            basic: Move::Composite(vec![]),
            lists: swap()
        }
    );
}
