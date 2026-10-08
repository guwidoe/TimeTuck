// Adapted from SwapMoveSelectorTest, Timefold Community (Apache-2.0).
// See compat/provenance.json. Native row IDs replace entity object identities.
use timetuck::*;

fn provider(right: Vec<usize>) -> ReachableEntitySwapProvider {
    ReachableEntitySwapProvider::new(
        vec![vec![0], vec![1], vec![2]],
        vec![0, 1, 2],
        right,
        EntitySwapSampling {
            seed: 0,
            max_candidates: 1,
            max_partner_attempts: 32,
        },
    )
    .unwrap()
}

#[test]
fn original_entities_pinned() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![1, 4]).allowing_unassigned(),
        Variable::new(vec![2, 3]).allowing_unassigned(),
        Variable::new(vec![1, 4]).allowing_unassigned(),
    ])
    .unwrap();
    let a = p.initial_assignment();
    // The source's code filter excludes B and C (it does not set planning pins).
    let selector = provider(vec![0]);
    // Recording a left identity binds a dependent view without lifecycle state.
    let left_a = selector.partners(&p, &a, 0).unwrap();
    let left_b = selector.partners(&p, &a, 1).unwrap();
    for _ in 0..2 {
        let mut empty = left_a.iter(); // regular and finite ending traversal
        assert_eq!(empty.next(), None);
        let mut iterator = left_b.iter().peekable();
        assert!(iterator.peek().is_some());
        assert_eq!(iterator.next(), Some(0));
        assert!(iterator.peek().is_none());
    }
    for _ in 0..2 {
        let mut empty = left_a.cursor();
        empty.rewind(); // opening at index zero
        assert!(!empty.has_next());
        let mut cursor = left_b.cursor();
        assert!(cursor.has_next());
        assert_eq!(cursor.next_partner(), Some(0));
        assert!(!cursor.has_next());
        assert!(cursor.has_previous());
        assert_eq!(cursor.previous_partner(), Some(0));
    }
}

#[test]
fn random_entities_pinned() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![1, 4])
            .allowing_unassigned()
            .with_initial(1),
        Variable::new(vec![2, 3])
            .allowing_unassigned()
            .with_initial(2),
        Variable::new(vec![1, 4]).allowing_unassigned(),
    ])
    .unwrap();
    let a = p.initial_assignment();
    let selector = provider(vec![0, 1, 2]);
    let view = selector.partners(&p, &a, 1).unwrap();
    let mut indices = std::iter::once(0).chain(std::iter::repeat_n(1, 31));
    let partner = view
        .sample_with(
            32,
            |size| {
                assert_eq!(size, 3);
                indices.next().unwrap()
            },
            |right| right == 0,
        )
        .unwrap();
    assert!(partner.is_some());
    assert_eq!(partner.unwrap(), 1); // same identity as recorded left B
}

#[test]
fn no_reachable_entities() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![1]).with_initial(1),
        Variable::new(vec![2]).with_initial(2),
        Variable::new(vec![3]).with_initial(3),
    ])
    .unwrap();
    let a = p.initial_assignment();
    let selector = provider(vec![0, 1, 2]);
    // Native seed zero; source asserts identity, not a particular Java draw trace.
    let mut iterator = selector.select(&p, &a, 0).unwrap().peekable();
    assert!(iterator.peek().is_some());
    let movement = iterator.next().unwrap();
    assert_eq!(movement.left, movement.right);
}
