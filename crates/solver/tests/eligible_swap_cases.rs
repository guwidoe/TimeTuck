// Adapted from Timefold Community SwapMoveSelectorTest (Apache-2.0).
// See compat/provenance.json. Semantic eligible indices replace Java mock RNG traces.
use timetuck::*;
fn fixture(multi: bool) -> (FiniteProblem<i32>, ReachableEntitySwapProvider) {
    let ranges = if multi {
        vec![
            vec![1, 4],
            vec![1, 4],
            vec![1],
            vec![2, 3],
            vec![2, 3],
            vec![1],
            vec![1, 4],
            vec![1, 3, 4],
            vec![1],
        ]
    } else {
        vec![vec![1, 4], vec![2, 3], vec![1, 4]]
    };
    let width = if multi { 3 } else { 1 };
    let p = FiniteProblem::new(
        ranges
            .into_iter()
            .map(|r| Variable::new(r).allowing_unassigned())
            .collect(),
    )
    .unwrap();
    let s = ReachableEntitySwapProvider::new(
        (0..3)
            .map(|r| (r * width..(r + 1) * width).collect())
            .collect(),
        vec![0, 1, 2],
        vec![0, 1, 2],
        EntitySwapSampling {
            seed: 0,
            max_candidates: 100,
            max_partner_attempts: 32,
        },
    )
    .unwrap()
    .with_eligible_partner_sampling();
    (p, s)
}
fn prefix(
    p: &FiniteProblem<i32>,
    a: &Assignment<i32>,
    s: &ReachableEntitySwapProvider,
    draws: &[usize],
    expected: &[(usize, usize)],
    size: bool,
) {
    if size {
        assert_eq!(s.pair_count_upper_bound(p, a).unwrap(), 9);
    }
    let mut indices = draws.iter().copied().cycle();
    let mut iter = s
        .select_repeating_with(p, a, move |_| indices.next().unwrap())
        .unwrap()
        .peekable();
    for &(l, r) in expected {
        let c = iter.next().unwrap().unwrap();
        assert_eq!((c.left, c.right), (l, r));
        let mut b = a.clone();
        let applied = p.apply(&mut b, &c.movement).unwrap();
        drop(applied);
        assert_eq!(&b, a);
    }
    // Source helper's nonempty-prefix never-ending/lookahead assertion uses
    // the real unbounded stream; actual solver consumption remains bounded.
    if !expected.is_empty() {
        assert!(iter.peek().is_some());
    }
}
#[test]
fn single_var_random_selection_with_entity_value_range() {
    let (p, s) = fixture(false);
    // Source reset(0,2,0,1,0,2): semantic A->C, with next available.
    prefix(&p, &p.initial_assignment(), &s, &[0, 0], &[(0, 2)], true);
    // Source reset(0,1,0,0,0,0); dependent eligible view contains only C.
    let a = Assignment::new(vec![Some(1), Some(3), Some(4)]);
    prefix(&p, &a, &s, &[0, 0], &[(0, 2)], true);
}
#[test]
fn multi_var_random_selection_with_entity_value_range() {
    let (p, s) = fixture(true);
    // Shared solution-owned third range [1] makes missing B reachable.
    prefix(
        &p,
        &p.initial_assignment(),
        &s,
        &[0, 0, 0, 1],
        &[(0, 1), (0, 2)],
        true,
    );
    let a = Assignment::new(vec![
        Some(1),
        Some(1),
        None,
        Some(3),
        Some(3),
        None,
        Some(4),
        Some(4),
        None,
    ]);
    prefix(&p, &a, &s, &[0, 0], &[(0, 2)], true);
    let a = Assignment::new(vec![
        Some(1),
        Some(1),
        None,
        Some(3),
        Some(3),
        None,
        Some(4),
        Some(3),
        None,
    ]);
    // Source helper's empty prefix performs no reads/size/continuation check.
    prefix(&p, &a, &s, &[0], &[], false);
    // Additional actual filter/legality control: third-column overlap cannot
    // bypass an incompatible second incoming value.
    let c = s
        .select_with(&p, &a, |_| 0)
        .unwrap()
        .next()
        .unwrap()
        .unwrap();
    assert_eq!((c.left, c.right), (0, 0));
    assert_eq!(s.partners(&p, &a, 0).unwrap().iter().count(), 0);
}
