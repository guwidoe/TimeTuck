use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
#[test]
fn generated_rejection_is_lazy_and_preserves_native_rank_values() {
    let calls = Cell::new(0);
    let compare = |a: &i64, b: &i64| {
        calls.set(calls.get() + 1);
        b.cmp(a)
    };
    let mut r = IntegerValueRange::new(-3, 8, 1).unwrap();
    let old = r.iter().collect::<Vec<_>>();
    assert_eq!(
        r.sort(Some(&compare)),
        Err(RangeOrderError::UnsupportedSorting)
    );
    assert_eq!(r.iter().collect::<Vec<_>>(), old);
    assert_eq!(calls.get(), 0);
    let mut large = BigIntegerValueRange::new(
        BigInt::from(0),
        "1000000000000000000000000000000".parse().unwrap(),
    )
    .unwrap();
    let count = large.len();
    let last = large.get(count - 1).unwrap();
    let compare =
        |_: &BigInt, _: &BigInt| panic!("immutable generator must not compare/materialize");
    assert_eq!(
        large.sort(Some(&compare)),
        Err(RangeOrderError::UnsupportedSorting)
    );
    assert_eq!(large.len(), count);
    assert_eq!(large.get(count - 1), Some(last));
}
#[test]
fn stored_sort_changes_real_selection_order_and_stages_panics() {
    let mut stored = EnumeratedValueRange::new(vec![3, 1, 2, 2]);
    stored.sort(None).unwrap();
    assert_eq!(stored.values(), [1, 2, 2, 3]);
    stored.sort(Some(&|a: &i32, b: &i32| b.cmp(a))).unwrap();
    assert_eq!(stored.values(), [3, 2, 2, 1]);
    let old = stored.values().to_vec();
    assert!(
        catch_unwind(AssertUnwindSafe(
            || stored.sort(Some(&|_, _| panic!("compare")))
        ))
        .is_err()
    );
    assert_eq!(stored.values(), old);
    let mut domain = stored.materialize(4).unwrap();
    domain.dedup(); // Native model domains require distinct values; range occurrences remain duplicated.
    let p = FiniteProblem::new(vec![Variable::new(domain)]).unwrap();
    let mut n = ConstraintNetwork::new(1);
    n.add_constraint("all equal", vec![0], |_| SimpleScore::ZERO)
        .unwrap();
    let result = solve_with_extensions(
        &p,
        &n,
        SolverSequence {
            phases: vec![SolverPhase::Construction(ConstructionConfig::default())],
            ..Default::default()
        },
        SolverExtensions::default(),
    )
    .unwrap();
    assert_eq!(result.assignment.values(), [Some(3)]);
    let mut generated = IntegerValueRange::new(0, 4, 1).unwrap();
    generated.sort(None).unwrap_err();
    let sorted = generated.sorted_by(4, |a, b| b.cmp(a)).unwrap();
    assert_eq!(sorted.values(), [3, 2, 1, 0]);
    assert_eq!(generated.get(0), Some(0));
}
