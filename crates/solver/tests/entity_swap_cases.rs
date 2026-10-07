// Adapted from SwapMoveSelectorTest (Timefold Community, Apache-2.0).
// Lifecycle-bearing methods below are partial fixtures, not method credits.
use timetuck::*;
#[test]
fn original_left_unequals_right_with_entity_range() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![1, 4]).allowing_unassigned(),
        Variable::new(vec![2, 3]).allowing_unassigned(),
        Variable::new(vec![1, 4]).allowing_unassigned(),
    ])
    .unwrap();
    let selector = EntitySwapProvider::new(
        vec![vec![0], vec![1], vec![2]],
        EntitySwapOrder::Cartesian {
            left: vec![0, 1, 2],
            right: vec![0, 1, 2],
        },
    )
    .unwrap()
    .with_noops(true);
    let a = p.initial_assignment();
    assert_eq!(selector.pair_count_upper_bound(&p, &a).unwrap(), 9);
    let names = ["A", "B", "C"];
    let actual: Vec<_> = selector
        .select(&p, &a, 0)
        .unwrap()
        .map(|c| format!("{}<->{}", names[c.left], names[c.right]))
        .collect();
    assert_eq!(
        actual,
        ["A<->B", "A<->C", "B<->A", "B<->C", "C<->A", "C<->B"]
    );
}
#[test]
fn deterministic_reopened_orders_partial_lifecycle_fixtures() {
    for (n, order, expected) in [
        (
            4,
            EntitySwapOrder::Unordered(vec![0, 1, 2, 3]),
            vec![(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)],
        ),
        (0, EntitySwapOrder::Unordered(vec![]), vec![]),
        (
            7,
            EntitySwapOrder::Cartesian {
                left: vec![0, 1, 2, 3],
                right: vec![4, 5, 6],
            },
            vec![
                (0, 4),
                (0, 5),
                (0, 6),
                (1, 4),
                (1, 5),
                (1, 6),
                (2, 4),
                (2, 5),
                (2, 6),
                (3, 4),
                (3, 5),
                (3, 6),
            ],
        ),
        (
            4,
            EntitySwapOrder::Cartesian {
                left: vec![0, 1, 2, 3],
                right: vec![],
            },
            vec![],
        ),
    ] {
        let p = FiniteProblem::new((0..n).map(|_| Variable::new(vec![1])).collect()).unwrap();
        let a = p.initial_assignment();
        let selector = EntitySwapProvider::new((0..n).map(|i| vec![i]).collect(), order)
            .unwrap()
            .with_noops(true);
        // Two steps, then three steps: preserve every source output sequence.
        // Native reopening does not test Java lifecycle callback forwarding.
        for step in [0, 1, 0, 1, 2] {
            assert_eq!(
                selector
                    .select(&p, &a, step)
                    .unwrap()
                    .map(|c| (c.left, c.right))
                    .collect::<Vec<_>>(),
                expected
            );
        }
    }
}
