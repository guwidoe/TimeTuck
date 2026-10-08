// Original native regressions; no upstream method credit.
use timetuck::*;

fn provider(
    rows: Vec<Vec<usize>>,
    left: Vec<usize>,
    right: Vec<usize>,
    candidates: u64,
    probes: u64,
) -> ReachableEntitySwapProvider {
    ReachableEntitySwapProvider::new(
        rows,
        left,
        right,
        EntitySwapSampling {
            seed: 71,
            max_candidates: candidates,
            max_partner_attempts: probes,
        },
    )
    .unwrap()
}

#[test]
fn overlap_does_not_override_legality_of_other_columns() {
    let rows = vec![vec![0, 1], vec![2, 3]];
    let s = provider(rows, vec![0], vec![1], 1, 1);
    let p = FiniteProblem::new(vec![
        Variable::new(vec![1]).allowing_unassigned(),
        Variable::new(vec![0]).allowing_unassigned(),
        Variable::new(vec![2]).allowing_unassigned(),
        Variable::new(vec![0]).allowing_unassigned(),
    ])
    .unwrap();
    for (values, expected) in [
        (vec![None, None, None, None], 1),
        (vec![Some(1), None, Some(2), None], 0),
        (vec![Some(1), Some(0), None, None], 0),
    ] {
        let a = Assignment::new(values);
        let c = s
            .select_with(&p, &a, |_| 0)
            .unwrap()
            .next()
            .unwrap()
            .unwrap();
        assert_eq!(c.right, expected);
        let mut working = a.clone();
        drop(p.apply(&mut working, &c.movement).unwrap());
        assert_eq!(working, a);
    }
    let p = FiniteProblem::new(vec![
        Variable::new(vec![1]),
        Variable::new(vec![1]).with_initial(1),
    ])
    .unwrap();
    let a = p.initial_assignment();
    let s = provider(vec![vec![0], vec![1]], vec![0], vec![1], 1, 1);
    assert_eq!(
        s.partners(&p, &a, 0).unwrap().iter().collect::<Vec<_>>(),
        vec![]
    );
    assert_eq!(s.select(&p, &a, 0).unwrap().next().unwrap().right, 0);
}

#[test]
fn finite_null_partners_and_random_overlap_are_distinct() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![7]).allowing_unassigned(),
        Variable::new(vec![8]).allowing_unassigned(),
        Variable::new(vec![7]).allowing_unassigned(),
    ])
    .unwrap();
    let a = p.initial_assignment();
    let s = provider(
        vec![vec![0], vec![1], vec![2]],
        vec![0],
        vec![0, 1, 2, 2],
        2,
        3,
    );
    let v = s.partners(&p, &a, 0).unwrap();
    let mut cursor = v.cursor();
    assert_eq!(cursor.previous_partner(), None);
    assert_eq!(cursor.next_partner(), Some(1));
    assert_eq!(cursor.next_partner(), Some(2));
    assert_eq!(cursor.next_partner(), Some(2));
    assert_eq!(cursor.next_partner(), None);
    assert_eq!(cursor.previous_partner(), Some(2));
    assert_eq!(cursor.previous_partner(), Some(2));
    cursor.rewind();
    assert_eq!(cursor.next_partner(), Some(1));
    let mut indices = [0, 1, 3].into_iter();
    assert_eq!(
        v.sample_with(3, |_| indices.next().unwrap(), |_| true)
            .unwrap(),
        Some(2)
    );
    assert_eq!(
        v.sample_with(0, |_| panic!("zero probes"), |_| true)
            .unwrap(),
        Some(0)
    );
    assert_eq!(v.sample_with(3, |_| 1, |_| true).unwrap(), Some(0));
    assert_eq!(
        v.sample_with(1, |_| 4, |_| true),
        Err(InvalidEntitySwapDraw { index: 4, size: 4 })
    );
}

#[test]
fn pinned_empty_and_invalid_orders_are_handled_before_sampling() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![0, 1]).with_initial(0).with_pinned(true),
        Variable::new(vec![0, 1]).with_initial(1),
    ])
    .unwrap();
    let a = p.initial_assignment();
    let rows = vec![vec![0], vec![1]];
    let s = provider(rows.clone(), vec![0, 1], vec![0], 3, 10);
    assert_eq!(s.pair_count_upper_bound(&p, &a).unwrap(), 0);
    let pinned = s.partners(&p, &a, 0).unwrap();
    assert_eq!(pinned.iter().count(), 0);
    assert_eq!(
        pinned
            .sample_with(10, |_| panic!("pinned"), |_| true)
            .unwrap(),
        None
    );
    let mut draws = 0;
    let out: Vec<_> = s
        .select_with(&p, &a, |size| {
            draws += 1;
            assert_eq!(size, 1);
            0
        })
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(draws, 3); // empty right skips all probes, but every left yields a fallback
    assert!(out.iter().all(|c| c.left == 1 && c.right == 1));
    let s = provider(rows.clone(), vec![0], vec![1], 10, 10);
    assert_eq!(
        s.select_with(&p, &a, |_| panic!("no active left"))
            .unwrap()
            .count(),
        0
    );
    let s = provider(rows.clone(), vec![1], vec![1], 0, 10);
    assert_eq!(
        s.select_with(&p, &a, |_| panic!("zero candidates"))
            .unwrap()
            .count(),
        0
    );
    let bad = provider(rows.clone(), vec![2], vec![1], 0, 0);
    assert!(matches!(
        bad.select(&p, &a, 0),
        Err(EntitySwapError::UnknownEntity { entity: 2, .. })
    ));
    let bad = provider(vec![vec![0], vec![2]], vec![0], vec![], 0, 0);
    assert!(matches!(
        bad.select(&p, &a, 0),
        Err(EntitySwapError::Schema(_))
    ));
    let s = provider(rows, vec![1], vec![1], 10, 10);
    assert!(matches!(
        s.partners(&p, &a, 2),
        Err(EntitySwapError::UnknownEntity { .. })
    ));
    let mut iter = s.select_with(&p, &a, |_| usize::MAX).unwrap();
    assert_eq!(iter.next().unwrap().unwrap_err().size, 1);
    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
    assert!(matches!(
        s.select(&p, &Assignment::new(vec![]), 0),
        Err(EntitySwapError::Model(_))
    ));
}

#[test]
fn partial_consumption_reopens_deterministically_and_moves_apply_atomically() {
    let p = FiniteProblem::new(
        (0..6)
            .map(|v| Variable::new((0..6).collect()).with_initial(v))
            .collect(),
    )
    .unwrap();
    let a = p.initial_assignment();
    let rows = vec![vec![0, 1], vec![2, 3], vec![4, 5]];
    let s = provider(rows, vec![0, 1, 2], vec![0, 1, 2], 50, 4);
    let expected: Vec<_> = s.select(&p, &a, 9).unwrap().collect();
    assert_eq!(expected.len(), 50);
    assert_eq!(
        s.select(&p, &a, 9).unwrap().take(3).collect::<Vec<_>>(),
        expected[..3]
    );
    assert_eq!(s.select(&p, &a, 9).unwrap().collect::<Vec<_>>(), expected);
    assert_ne!(s.select(&p, &a, 10).unwrap().collect::<Vec<_>>(), expected);
    for candidate in expected {
        let mut b = a.clone();
        let applied = p.apply(&mut b, &candidate.movement).unwrap();
        drop(applied);
        assert_eq!(b, a);
    }
}

#[test]
fn bounded_provider_runs_in_solver_and_fallbacks_preserve_assignment() {
    let p = FiniteProblem::new(
        (0..3)
            .map(|v| Variable::new(vec![v]).with_initial(v))
            .collect(),
    )
    .unwrap();
    let n = ConstraintNetwork::<_, SimpleScore>::new(3);
    let mut s = provider(
        vec![vec![0], vec![1], vec![2]],
        vec![0, 1, 2],
        vec![0, 1, 2],
        7,
        2,
    );
    let mut ext = SolverExtensions::default();
    ext.move_providers.bind_shared([0], &mut s).unwrap();
    let result = solve_with_extensions(
        &p,
        &n,
        SolverSequence {
            phases: vec![SolverPhase::LocalSearch(LocalSearchConfig {
                max_steps: 3,
                selection: MoveSelection::BestAccepted,
                acceptance: Acceptance::HillClimbing,
                ..Default::default()
            })],
            ..Default::default()
        },
        ext,
    )
    .unwrap();
    assert_eq!(result.work.move_evaluations, 21);
    assert_eq!(result.work.completed_steps, 3);
    assert_eq!(result.assignment, p.initial_assignment());
}

#[test]
fn mixed_solver_uses_reachable_partners_and_preserves_list_state() {
    let p = MixedProblem::new(
        FiniteProblem::new(vec![
            Variable::new(vec![2, 4]).with_initial(2),
            Variable::new(vec![2, 4]).with_initial(4),
        ])
        .unwrap(),
        ListProblem::new(vec![9], vec![ListEntity::new(vec![9])]).unwrap(),
    );
    let mut n = MixedConstraintNetwork::new(2, 1, 1);
    n.add_constraint("first", vec![MixedScope::Basic(0)], |facts| {
        let MixedFact::Basic(v) = facts[0] else {
            unreachable!()
        };
        SimpleScore::new(i64::from(v.copied().unwrap()))
    })
    .unwrap();
    let mut s = provider(vec![vec![0], vec![1]], vec![0], vec![1], 1, 1);
    let mut ext = SolverExtensions::default();
    ext.move_providers.bind(0, &mut s);
    let result = solve_mixed_with_extensions(
        &p,
        &n,
        MixedSolverSequence {
            phases: vec![MixedSolverPhase::LocalSearch(MixedLocalSearchConfig {
                search: LocalSearchConfig {
                    max_steps: 2,
                    assert_scores: true,
                    ..Default::default()
                },
                ..Default::default()
            })],
            ..Default::default()
        },
        ext,
    )
    .unwrap();
    assert_eq!(result.score, Some(SimpleScore::new(4)));
    assert_eq!(result.assignment.basic().values(), &[Some(4), Some(2)]);
    assert_eq!(result.assignment.lists().lists(), &[vec![9]]);
}

#[test]
fn basic_sampling_needs_no_clone_payloads() {
    #[derive(PartialEq, Eq)]
    struct Value(u8);
    let p = FiniteProblem::new(vec![
        Variable::new(vec![Value(0)]),
        Variable::new(vec![Value(0)]),
    ])
    .unwrap();
    let a = Assignment::new(vec![None, None]);
    let s = provider(vec![vec![0], vec![1]], vec![0], vec![1], 1, 1);
    let c = s.select(&p, &a, 0).unwrap().next().unwrap();
    assert_eq!((c.left, c.right), (0, 1));
}
