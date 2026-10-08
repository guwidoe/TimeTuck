// Adapted from Timefold Community SwapMoveProviderTest and its multivariable
// test-domain fixtures, Apache-2.0. See compat/provenance.json and notices.
use timetuck::*;
#[derive(Clone, Debug, Eq, PartialEq)]
enum Value {
    Main(usize),
    Other(usize),
}
fn generated(count: usize, main: usize, other: usize) -> FiniteProblem<Value> {
    FiniteProblem::new(
        (0..count)
            .flat_map(|i| {
                [
                    Variable::new((0..main).map(Value::Main).collect())
                        .with_initial(Value::Main(i % main)),
                    Variable::new((0..main).map(Value::Main).collect())
                        .with_initial(Value::Main((i + 1) % main)),
                    Variable::new((0..other).map(Value::Other).collect())
                        .with_initial(Value::Other(i % other))
                        .allowing_unassigned(),
                ]
            })
            .collect(),
    )
    .unwrap()
}
fn provider(entities: usize, columns: usize) -> EntitySwapProvider {
    EntitySwapProvider::new(
        (0..entities)
            .map(|r| (0..columns).map(|c| r * 3 + c).collect())
            .collect(),
        EntitySwapOrder::Cartesian {
            left: (0..entities).collect(),
            right: (0..entities).collect(),
        },
    )
    .unwrap()
}
fn expected<V>(left: usize, right: usize, columns: usize) -> Move<V> {
    Move::Composite(
        (0..columns)
            .map(|c| Move::Swap {
                left: left * 3 + c,
                right: right * 3 + c,
            })
            .collect(),
    )
}
#[test]
fn multivariate() {
    let p = generated(3, 2, 2);
    let a = p.initial_assignment();
    let provider = provider(3, 3);
    let moves: Vec<_> = provider
        .select(&p, &a, 0)
        .unwrap()
        .map(|c| c.movement)
        .collect();
    for (l, r) in [(0, 1), (1, 0), (1, 2), (2, 1)] {
        assert!(moves.contains(&expected(l, r, 3)));
    }
    for (l, r) in [(0, 2), (2, 0)] {
        assert!(!moves.contains(&expected(l, r, 3)));
    }
    assert_eq!(moves.len(), 4);
    // Native integration: each selected composite exchanges all three columns
    // atomically and dropping its guard restores the original assignment.
    for c in provider.select(&p, &a, 0).unwrap() {
        let mut working = a.clone();
        {
            let trial = p.apply(&mut working, &c.movement).unwrap();
            for column in 0..3 {
                assert_eq!(
                    trial.values()[c.left * 3 + column],
                    a.values()[c.right * 3 + column]
                );
                assert_eq!(
                    trial.values()[c.right * 3 + column],
                    a.values()[c.left * 3 + column]
                );
            }
        }
        assert_eq!(working, a);
    }
}
#[test]
fn multivariate_with_exclusions() {
    let p = generated(3, 1, 2);
    let a = p.initial_assignment();
    let moves: Vec<_> = provider(3, 2)
        .select(&p, &a, 0)
        .unwrap()
        .map(|c| c.movement)
        .collect();
    for (l, r) in [(0, 1), (1, 0), (0, 2), (2, 0), (1, 2), (2, 1)] {
        assert!(!moves.contains(&expected(l, r, 2)));
    }
    assert!(moves.is_empty());
    // The excluded column really differs; selecting it enables swaps.
    assert_eq!(provider(3, 3).select(&p, &a, 0).unwrap().count(), 4);
}
#[test]
fn mixed_legality_across_variables_rejects_whole_pair() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![1, 4])
            .with_initial(1)
            .allowing_unassigned(),
        Variable::new(vec![1, 4])
            .with_initial(1)
            .allowing_unassigned(),
        Variable::new(vec![1]).allowing_unassigned(),
        Variable::new(vec![1, 4])
            .with_initial(4)
            .allowing_unassigned(),
        Variable::new(vec![1, 3, 4])
            .with_initial(3)
            .allowing_unassigned(),
        Variable::new(vec![1]).allowing_unassigned(),
    ])
    .unwrap();
    let a = p.initial_assignment();
    let moves: Vec<_> = provider(2, 3)
        .select(&p, &a, 0)
        .unwrap()
        .map(|c| c.movement)
        .collect();
    assert!(!moves.contains(&expected(0, 1, 3)));
    assert!(!moves.contains(&expected(1, 0, 3)));
    assert!(moves.is_empty());
    // Native boundary check: applying the forbidden composite must also fail
    // after its legal first child, restoring that child and both missing values.
    let mut working = a.clone();
    assert!(p.apply(&mut working, &expected(0, 1, 3)).is_err());
    assert_eq!(working, a);
}
