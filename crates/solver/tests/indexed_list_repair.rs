//! Original indexed dependency repair regressions; no upstream source credit.
use std::{
    cell::Cell,
    hash::{Hash, Hasher},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;
fn problem(indexed: bool, n: usize, bad: usize, pin: usize) -> ListProblem<usize> {
    let mut rows: Vec<_> = (0..bad)
        .step_by(2)
        .rev()
        .map(|i| {
            let row = ListEntity::new(vec![i + 1, i]).with_value_range(vec![i, i + 1]);
            match pin {
                1 => row.with_pinned(true),
                2 => row.with_pinned_prefix(1),
                _ => row,
            }
        })
        .collect();
    rows.push(ListEntity::new(vec![n - 1]));
    rows.push(ListEntity::new(vec![]));
    let mut deps = vec![vec![]; n];
    for i in (0..bad).step_by(2) {
        deps[i + 1] = vec![i];
    }
    (if indexed {
        ListProblem::new_indexed((0..n).collect(), rows)
    } else {
        ListProblem::new((0..n).collect(), rows)
    })
    .unwrap()
    .with_dependencies(deps)
    .unwrap()
}
#[test]
fn dense_sparse_and_threshold_repairs_match_direct_and_preserve_metadata() {
    for (n, bad) in [(8, 4), (128, 62), (128, 64), (128, 96), (4096, 64)] {
        for pin in 0..3 {
            let direct = problem(false, n, bad, pin);
            let indexed = problem(true, n, bad, pin);
            let original = indexed.initial_assignment();
            let expected = direct.prepare_assignment(&original);
            assert_eq!(indexed.prepare_assignment(&original), expected);
            if pin == 0 {
                let clean = expected.unwrap();
                assert!(clean.lists()[..bad / 2].iter().all(Vec::is_empty));
                assert_eq!(clean.lists()[bad / 2], vec![n - 1]);
                assert!(indexed.inconsistent_values(&clean).unwrap().is_empty());
                assert_eq!(indexed.prepare_assignment(&clean).unwrap(), clean);
            } else {
                // Rows are reversed: the earliest diagnostic must follow value
                // registry order, not the first pinned row encountered.
                assert_eq!(
                    expected,
                    Err(ListError::PinnedDependencyCycle {
                        value: if pin == 1 { 0 } else { 1 },
                        position: ListPosition {
                            entity: bad / 2 - 1,
                            index: usize::from(pin == 1)
                        }
                    })
                );
            }
            assert_eq!(indexed.initial_assignment(), original);
            assert_eq!(
                indexed.entities()[0].value_range(),
                Some([bad - 2, bad - 1].as_slice())
            );
        }
    }
}
#[derive(Clone, Debug)]
struct Value {
    id: usize,
    payload: usize,
    calls: Rc<Cell<usize>>,
    panic_at: Rc<Cell<usize>>,
}
impl PartialEq for Value {
    fn eq(&self, o: &Self) -> bool {
        self.id == o.id
    }
}
impl Eq for Value {}
impl Hash for Value {
    fn hash<H: Hasher>(&self, h: &mut H) {
        let calls = self.calls.get() + 1;
        self.calls.set(calls);
        assert_ne!(calls, self.panic_at.get(), "planned hash failure");
        self.id.hash(h);
    }
}
#[test]
fn repair_preserves_supplied_payloads_and_caller_on_hash_panic() {
    let calls = Rc::new(Cell::new(0));
    let panic_at = Rc::new(Cell::new(usize::MAX));
    let make = |id, payload| Value {
        id,
        payload,
        calls: calls.clone(),
        panic_at: panic_at.clone(),
    };
    let mut rows: Vec<_> = (0..96)
        .step_by(2)
        .map(|i| ListEntity::new(vec![make(i + 1, 1001 + i), make(i, 1000 + i)]))
        .collect();
    rows.push(ListEntity::new(
        (96..128).map(|i| make(i, 1000 + i)).collect(),
    ));
    let mut deps = vec![vec![]; 128];
    for i in (0..96).step_by(2) {
        deps[i + 1] = vec![i];
    }
    let p = ListProblem::new_indexed((0..128).map(|i| make(i, i)).collect(), rows)
        .unwrap()
        .with_dependencies(deps)
        .unwrap();
    let original = p.initial_assignment();
    let clean = p.prepare_assignment(&original).unwrap();
    assert_eq!(
        clean
            .lists()
            .last()
            .unwrap()
            .iter()
            .map(|v| v.payload)
            .collect::<Vec<_>>(),
        (1096..1128).collect::<Vec<_>>()
    );
    calls.set(0);
    panic_at.set(128 * 2 + 17);
    assert!(catch_unwind(AssertUnwindSafe(|| p.prepare_assignment(&original))).is_err());
    panic_at.set(usize::MAX);
    assert_eq!(original.lists()[0][0].payload, 1001);
    assert_eq!(original.lists()[0][1].payload, 1000);
    assert_eq!(original.lists().last().unwrap()[0].payload, 1096);
    assert_eq!(p.prepare_assignment(&original).unwrap(), clean);
}
#[test]
fn mixed_repair_removes_downstream_singletons_through_unassigned_values() {
    let mut p = problem(true, 132, 96, 0);
    let mut deps = vec![vec![]; 132];
    for i in (0..96).step_by(2) {
        deps[i + 1] = vec![i];
    }
    deps[130] = vec![1];
    deps[131] = vec![130];
    p = p.with_dependencies(deps).unwrap();
    let p = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![7, 8]).with_initial(7)]).unwrap(),
        p,
    );
    let original = p.initial_assignment();
    let clean = p.prepare_assignment(&original).unwrap();
    assert_eq!(clean.basic(), original.basic());
    assert!(clean.lists().lists().iter().all(Vec::is_empty));
    assert_eq!(p.initial_assignment(), original);
}
