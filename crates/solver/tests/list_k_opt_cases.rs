// Pinned Timefold Community test fixtures, Apache-2.0; see compat/provenance.json.
// Output fixtures do not claim Java callback compatibility or method coverage.
use timetuck::*;
fn problem(n: usize, prefix: usize) -> ListProblem<usize> {
    ListProblem::new(
        (1..=n).collect(),
        vec![ListEntity::new((1..=n).collect()).with_pinned_prefix(prefix)],
    )
    .unwrap()
}
fn indices(edges: &[(usize, usize)]) -> Vec<(usize, usize)> {
    edges.iter().map(|&(a, b)| (a - 1, b - 1)).collect()
}
#[test]
fn single_entity_output_fixtures() {
    // (n, prefix, removed edges, added edges, expected order).
    let cases = [
        (
            6,
            0,
            vec![(6, 1), (2, 3), (4, 5)],
            vec![(1, 3), (2, 5), (4, 6)],
            vec![1, 2, 5, 6, 4, 3],
        ),
        (
            7,
            1,
            vec![(7, 2), (3, 4), (5, 6)],
            vec![(2, 4), (3, 6), (5, 7)],
            vec![1, 2, 3, 6, 7, 5, 4],
        ),
        (
            10,
            0,
            vec![(10, 1), (3, 4), (8, 9)],
            vec![(1, 4), (3, 9), (8, 10)],
            vec![1, 2, 3, 9, 10, 8, 7, 6, 5, 4],
        ),
        (
            8,
            0,
            vec![(1, 2), (4, 3), (7, 8), (6, 5)],
            vec![(2, 4), (3, 7), (8, 6), (5, 1)],
            vec![1, 5, 4, 2, 3, 7, 6, 8],
        ),
        (
            12,
            0,
            vec![(1, 12), (2, 3), (5, 6), (9, 10)],
            vec![(1, 3), (2, 6), (5, 10), (9, 12)],
            vec![1, 3, 4, 5, 10, 11, 12, 9, 8, 7, 6, 2],
        ),
        (
            12,
            0,
            vec![(2, 3), (6, 5), (10, 9), (12, 1)],
            vec![(2, 5), (6, 10), (9, 1), (12, 3)],
            vec![1, 2, 5, 4, 3, 12, 11, 10, 6, 7, 8, 9],
        ),
        (
            8,
            0,
            vec![(8, 1), (4, 5), (2, 3), (6, 7)],
            vec![(1, 4), (6, 3), (5, 8), (7, 2)],
            vec![1, 2, 7, 8, 5, 6, 3, 4],
        ),
        (
            12,
            0,
            vec![(12, 1), (5, 6), (2, 3), (8, 9)],
            vec![(1, 5), (8, 3), (6, 12), (9, 2)],
            vec![1, 2, 9, 10, 11, 12, 6, 7, 8, 3, 4, 5],
        ),
    ];
    for (n, prefix, removed, added, expected) in cases {
        let p = problem(n, prefix);
        let mut a = p.initial_assignment();
        let m = p
            .k_opt_move(
                &a,
                0,
                &indices(&removed),
                &indices(&added),
                expected[prefix + 1] - 1,
            )
            .unwrap();
        {
            let trial = p.apply(&mut a, &m).unwrap();
            assert_eq!(trial.lists()[0], expected);
        }
        assert_eq!(a, p.initial_assignment());
    }
}
#[test]
fn is_feasible() {
    let p = problem(8, 0);
    let a = p.initial_assignment();
    assert!(
        p.k_opt_move(
            &a,
            0,
            &indices(&[(1, 2), (4, 3), (7, 8), (6, 5)]),
            &indices(&[(2, 4), (3, 7), (8, 6), (5, 1)]),
            4
        )
        .is_ok()
    );
    assert_eq!(
        p.k_opt_move(
            &a,
            0,
            &indices(&[(1, 2), (3, 4), (7, 8), (6, 5)]),
            &indices(&[(2, 3), (4, 7), (8, 6), (5, 1)]),
            4
        ),
        Err(ListKOptError::DuplicateEdge)
    );
}
