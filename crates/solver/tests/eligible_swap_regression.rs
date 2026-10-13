// Original native controls; no upstream-method credit.
use timetuck::*;
fn setup() -> (FiniteProblem<i32>, ReachableEntitySwapProvider) {
    let p = FiniteProblem::new(
        (0..3)
            .map(|v| Variable::new(vec![0, 1, 2]).with_initial(v))
            .collect(),
    )
    .unwrap();
    let s = ReachableEntitySwapProvider::new(
        vec![vec![0], vec![1], vec![2]],
        vec![0, 1, 2],
        vec![0, 1, 1, 2],
        EntitySwapSampling {
            seed: 37,
            max_candidates: 9,
            max_partner_attempts: 0,
        },
    )
    .unwrap()
    .with_eligible_partner_sampling();
    (p, s)
}
#[test]
fn duplicate_occurrences_replay_invalid_draw_and_refresh() {
    let (p, s) = setup();
    let a = p.initial_assignment();
    let mut draws = [0, 1].into_iter();
    let c = s
        .select_with(&p, &a, |bound| {
            let n = draws.next().unwrap();
            assert_eq!(bound, 3);
            n
        })
        .unwrap()
        .next()
        .unwrap()
        .unwrap();
    assert_eq!((c.left, c.right), (0, 1));
    let first: Vec<_> = s.select(&p, &a, 0).unwrap().collect();
    assert_eq!(first, s.select(&p, &a, 0).unwrap().collect::<Vec<_>>());
    assert_ne!(first, s.select(&p, &a, 1).unwrap().collect::<Vec<_>>());
    let mut iter = s.select_with(&p, &a, |_| usize::MAX).unwrap();
    assert!(iter.next().unwrap().is_err());
    assert!(iter.next().is_none());
    let p = FiniteProblem::new(vec![
        Variable::new(vec![0]).with_initial(0),
        Variable::new(vec![1]).with_initial(1),
        Variable::new(vec![2]).with_initial(2),
    ])
    .unwrap();
    let a = p.initial_assignment();
    let mut calls = 0;
    let mut iter = s
        .select_with(&p, &a, |_| {
            calls += 1;
            0
        })
        .unwrap();
    for c in iter.by_ref() {
        let c = c.unwrap();
        assert_eq!((c.left, c.right), (0, 0));
    }
    drop(iter);
    assert_eq!(calls, 9); // No right draw when dependent view is empty.
}
#[test]
fn eligible_policy_runs_in_real_solver_with_checked_domains() {
    let (p, mut s) = setup();
    let n = ConstraintNetwork::<_, SimpleScore>::new(3);
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
    assert_eq!(result.work.move_evaluations, 27);
    assert_eq!(result.work.completed_steps, 3);
    p.validate(&result.assignment).unwrap();
    let mut values = result.assignment.values().to_vec();
    values.sort();
    assert_eq!(values, vec![Some(0), Some(1), Some(2)]);
}
