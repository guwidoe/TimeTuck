// Adapted from pinned Timefold Community BruteForceTest (Apache-2.0).
// Four complete methods and fixture provenance are supplied separately.
// Native unbounded tree search uses actual partial-node expansion steps. The
// source asserts upper ceilings, not equal Java/native traversal counts.
use timetuck::*;
fn plan() -> SolverSequence<'static> {
    SolverSequence {
        phases: vec![SolverPhase::Exact(ExactPhaseConfig {
            strategy: ExactSearchStrategy::BranchAndBound(BoundedPhaseConfig {
                exploration: NodeExploration::InputOrderDepthFirst,
                bound: PartialScoreBound::Unknown,
                ..Default::default()
            }),
            ..Default::default()
        })],
        ..Default::default()
    }
}
fn observe(event: SolverLifecycleEvent, ceiling: u64, steps: &mut u64) {
    match event {
        SolverLifecycleEvent::PhaseStarted { kind, .. }
        | SolverLifecycleEvent::PhaseEnded { kind, .. } => assert_eq!(kind, SolverPhaseKind::Exact),
        SolverLifecycleEvent::StepStarted { kind, step, .. } => {
            assert_eq!(kind, SolverPhaseKind::Exact, "wrong phase started");
            assert!(
                step < ceiling,
                "exhaustive phase exceeded source step ceiling"
            );
            assert_eq!(step, *steps);
            *steps += 1;
        }
        SolverLifecycleEvent::StepEnded { completed, .. } => assert!(completed),
        _ => {}
    }
}
#[test]
fn solve_basic_variable_not_allowed_unassigned() {
    // TestdataSolution.generateSolution(2,2), followed by clearing both values.
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]); 2]).unwrap();
    let mut n = ConstraintNetwork::new(2);
    // TestdataEasyScoreCalculator: every ordered equal-value entity pair costs1.
    n.add_constraint("ordered collisions", vec![0, 1], |v| {
        SimpleScore::new(
            -v.iter()
                .enumerate()
                .map(|(i, left)| {
                    v.iter()
                        .enumerate()
                        .filter(|(j, right)| i != *j && left.is_some() && left == *right)
                        .count() as i64
                })
                .sum::<i64>(),
        )
    })
    .unwrap();
    let mut steps = 0;
    let r = solve_with_extensions(
        &p,
        &n,
        plan(),
        SolverExtensions {
            lifecycle: Some(&mut |e| observe(e, 3, &mut steps)),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(r.score, Some(SimpleScore::ZERO));
    assert_eq!(r.assignment.values()[0], Some(0));
    assert_eq!(r.assignment.values()[1], Some(1));
    // Additional native proof that the source listener assertion was nonvacuous.
    assert_eq!(steps, 3);
    assert_eq!(r.work.completed_steps, steps);
    assert!(r.is_optimal());
}
fn list_case(optional: bool) -> (SolverResult<ListAssignment<i32>, SimpleScore>, u64) {
    // Both source generators create3 distinct values and2 initially empty lists.
    let p = ListProblem::new(vec![0, 1, 2], vec![ListEntity::new(vec![]); 2]).unwrap();
    let p = if optional { p.allowing_unassigned() } else { p };
    let mut n = ListConstraintNetwork::new(2, 3);
    n.add_constraint(
        "source objective",
        vec![ListScope::Entity(0), ListScope::Entity(1)],
        move |facts| {
            SimpleScore::new(
                facts
                    .iter()
                    .map(|fact| {
                        let ListFact::Entity(list) = fact else {
                            unreachable!()
                        };
                        if optional {
                            -(list.len() as i64)
                        } else if list.len() == 1 {
                            2
                        } else {
                            1
                        }
                    })
                    .sum(),
            )
        },
    )
    .unwrap();
    let mut steps = 0;
    let r = solve_list_with_extensions(
        &p,
        &n,
        plan(),
        SolverExtensions {
            lifecycle: Some(&mut |e| observe(e, 25, &mut steps)),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(steps > 0);
    assert_eq!(r.work.completed_steps, steps);
    assert!(r.is_optimal());
    (r, steps)
}
#[test]
fn solve_list_variable_not_allowed_unassigned() {
    let (r, steps) = list_case(false);
    // Source checks inverse owner == null; membership is the native equivalent.
    let unassigned: Vec<_> = (0..3)
        .filter(|v| !r.assignment.lists().iter().any(|row| row.contains(v)))
        .collect();
    assert!(unassigned.is_empty());
    assert_eq!(steps, 9); // Additional native count, not Java count equivalence.
    assert_eq!(r.score, Some(SimpleScore::new(3)));
}
#[test]
fn solve_list_variable_allowed_unassigned() {
    let (r, steps) = list_case(true);
    let unassigned: Vec<_> = (0..3)
        .filter(|v| !r.assignment.lists().iter().any(|row| row.contains(v)))
        .collect();
    assert_eq!(unassigned.len(), 3);
    assert_eq!(steps, 15);
    assert_eq!(r.score, Some(SimpleScore::ZERO));
}

#[test]
fn solve_basic_variable_allowed_unassigned() {
    // TestdataAllowsUnassignedSolution.generateSolution(1,2); both values cleared.
    let p = FiniteProblem::new(vec![Variable::new(vec![0]).allowing_unassigned(); 2]).unwrap();
    let mut n = ConstraintNetwork::new(2);
    n.add_constraint("optional source objective", vec![0, 1], |values| {
        SimpleScore::new(
            values
                .iter()
                .enumerate()
                .map(|(i, left)| {
                    if left.is_none() {
                        -1
                    } else {
                        -1000
                            * values
                                .iter()
                                .enumerate()
                                .filter(|(j, right)| i != *j && left == *right)
                                .count() as i64
                    }
                })
                .sum(),
        )
    })
    .unwrap();
    let mut c = plan();
    let SolverPhase::Exact(ref mut phase) = c.phases[0] else {
        unreachable!()
    };
    let ExactSearchStrategy::BranchAndBound(ref mut bounded) = phase.strategy else {
        unreachable!()
    };
    // Native None is visited after domain values; reverse entity traversal to
    // preserve the source's equal-optimum choice: first unassigned, second set.
    bounded.order = Some(
        BranchOrder {
            basic_variables: Some(vec![1, 0]),
            ..Default::default()
        }
        .into(),
    );
    let mut steps = 0;
    let r = solve_with_extensions(
        &p,
        &n,
        c,
        SolverExtensions {
            lifecycle: Some(&mut |e| observe(e, 3, &mut steps)),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(r.score, Some(SimpleScore::new(-1)));
    assert!(r.assignment.values()[0].is_none());
    assert!(r.assignment.values()[1].is_some());
    assert_eq!(steps, 3);
    assert_eq!(r.work.completed_steps, steps);
    assert!(r.is_optimal());
}
