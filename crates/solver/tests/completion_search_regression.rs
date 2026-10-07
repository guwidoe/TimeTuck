use std::collections::BTreeSet;
use timetuck::*;
fn config() -> SolverSequence<'static> {
    SolverSequence {
        phases: vec![SolverPhase::Exact(ExactPhaseConfig {
            strategy: ExactSearchStrategy::Complete,
            ..Default::default()
        })],
        ..Default::default()
    }
}
#[test]
fn basic_extensions_validate_stop_and_keep_optional_assigned_values() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![1, 2])
            .with_initial(1)
            .allowing_unassigned(),
        Variable::new(vec![1, 2]).allowing_unassigned(),
        Variable::new(vec![7])
            .with_pinned(true)
            .allowing_unassigned(),
    ])
    .unwrap();
    // The supplied entry, not model initial values, is the retained baseline.
    let a = Assignment::new(vec![Some(2), None, None]);
    let mut seen = vec![];
    assert!(
        visit_basic_completions(&p, &a, |v| {
            seen.push(v.clone());
            true
        })
        .unwrap()
    );
    assert_eq!(
        seen,
        [
            Assignment::new(vec![Some(2), Some(1), None]),
            Assignment::new(vec![Some(2), Some(2), None]),
            a.clone()
        ]
    );
    let mut calls = 0;
    assert!(
        !visit_basic_completions(&p, &a, |_| {
            calls += 1;
            false
        })
        .unwrap()
    );
    assert_eq!(calls, 1);
    assert!(
        visit_basic_completions(&p, &Assignment::new(vec![Some(9), None, None]), |_| panic!(
            "invalid scored"
        ))
        .is_err()
    );
    let blocked = FiniteProblem::new(vec![Variable::new(vec![1]).with_pinned(true)]).unwrap();
    assert!(
        visit_basic_completions(&blocked, &blocked.initial_assignment(), |_| panic!(
            "impossible scored"
        ))
        .unwrap()
    );
    assert_eq!(a.values(), &[Some(2), None, None]);
}
#[test]
fn list_completions_equal_full_enumeration_filtered_by_retained_subsequences() {
    // Exhaust all partial assignments of three values across two lists. Compare
    // the new iterator with full enumeration, independently filtering anchors.
    for optional in [false, true] {
        let all = ListProblem::new(vec![1, 2, 3], vec![ListEntity::new(vec![]); 2])
            .unwrap()
            .allowing_unassigned();
        let mut partials = vec![];
        exhaustive_list_search(
            &all,
            |a| {
                partials.push(a.clone());
                SimpleScore::ZERO
            },
            u64::MAX,
        );
        let p = ListProblem::new(vec![1, 2, 3], vec![ListEntity::new(vec![]); 2]).unwrap();
        let p = if optional { p.allowing_unassigned() } else { p };
        let mut full = vec![];
        exhaustive_list_search(
            &p,
            |a| {
                full.push(a.clone());
                SimpleScore::ZERO
            },
            u64::MAX,
        );
        for a in partials {
            let expected: BTreeSet<_> = full
                .iter()
                .filter(|candidate| {
                    a.lists()
                        .iter()
                        .zip(candidate.lists())
                        .all(|(anchor, list)| {
                            list.iter().filter(|v| anchor.contains(v)).eq(anchor.iter())
                        })
                })
                .map(|a| a.lists().to_vec())
                .collect();
            let mut seen = vec![];
            assert!(
                visit_list_completions(&p, &a, |a| {
                    seen.push(a.lists().to_vec());
                    true
                })
                .unwrap()
            );
            let actual: BTreeSet<_> = seen.iter().cloned().collect();
            assert_eq!(actual.len(), seen.len(), "duplicate {a:?}");
            assert_eq!(actual, expected, "{a:?}");
        }
    }
}
#[test]
fn list_completions_respect_pins_ranges_dependencies_and_early_stop() {
    let p = ListProblem::new(
        vec![1, 2, 3, 4],
        vec![
            ListEntity::new(vec![4]).with_pinned(true),
            ListEntity::new(vec![1, 3])
                .with_pinned_prefix(1)
                .with_value_range(vec![1, 2, 3]),
        ],
    )
    .unwrap()
    .with_dependencies(vec![vec![], vec![0], vec![1], vec![]])
    .unwrap();
    let a = p.initial_assignment();
    let mut seen = vec![];
    assert!(
        visit_list_completions(&p, &a, |a| {
            seen.push(a.lists().to_vec());
            true
        })
        .unwrap()
    );
    assert_eq!(seen, [vec![vec![4], vec![1, 2, 3]]]);
    let mut calls = 0;
    assert!(
        !visit_list_completions(&p, &a, |_| {
            calls += 1;
            false
        })
        .unwrap()
    );
    assert_eq!(calls, 1);
    assert_eq!(a, p.initial_assignment());
    assert!(
        visit_list_completions(
            &p,
            &ListAssignment::new(vec![vec![], vec![1, 3, 4]]),
            |_| panic!()
        )
        .is_err()
    );
}
#[test]
fn mixed_completion_product_and_stop_propagation() {
    let b = FiniteProblem::new(vec![
        Variable::new(vec![1, 2]),
        Variable::new(vec![1, 2]).with_initial(1),
    ])
    .unwrap();
    let l = ListProblem::new(vec![1, 2, 3], vec![ListEntity::new(vec![1, 3])]).unwrap();
    let p = MixedProblem::new(b, l);
    let a = p.initial_assignment();
    let mut seen = vec![];
    assert!(
        visit_mixed_completions(&p, &a, |a| {
            seen.push(a.clone());
            true
        })
        .unwrap()
    );
    assert_eq!(seen.len(), 6);
    for pair in seen.chunks(2) {
        assert_eq!(pair[0].lists(), pair[1].lists());
        assert_eq!(pair[0].basic().values(), &[Some(1), Some(1)]);
        assert_eq!(pair[1].basic().values(), &[Some(2), Some(1)]);
    }
    let mut calls = 0;
    assert!(
        !visit_mixed_completions(&p, &a, |_| {
            calls += 1;
            calls < 3
        })
        .unwrap()
    );
    assert_eq!(calls, 3);
    assert_eq!(a, p.initial_assignment());
}
#[test]
fn completion_controls_phase_entry_state_and_initialization_events() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![1, 2, 3]).with_initial(1),
        Variable::new(vec![1, 2, 3]),
    ])
    .unwrap();
    let mut n = ConstraintNetwork::new(2);
    n.add_constraint("sum", vec![0, 1], |v| {
        SimpleScore::new(v.iter().flatten().map(|v| **v).sum())
    })
    .unwrap();
    for budget in [0, 1, 3] {
        let mut c = config();
        c.limits.max_evaluations = budget;
        let mut initialized = vec![];
        let r = solve_with_events(&p, &n, c, |e| {
            if let SolverEvent::FirstInitialized { initialization, .. } = e {
                initialized.push(initialization)
            }
        })
        .unwrap();
        assert_eq!(r.work.evaluations, budget);
        assert!(!r.is_optimal());
        if budget == 3 {
            assert_eq!(r.termination, SolverTermination::PlanCompleted);
            assert!(initialized.is_empty());
            assert_eq!(r.assignment.values(), &[Some(1), Some(3)]);
        } else {
            assert_eq!(r.termination, SolverTermination::EvaluationLimit);
            if budget == 0 {
                assert_eq!(r.assignment, p.initial_assignment());
                assert!(initialized.is_empty());
            }
        }
    }
    let mut c = config();
    c.phases.push(SolverPhase::LocalSearch(LocalSearchConfig {
        max_steps: 1,
        selection: MoveSelection::BestAccepted,
        ..Default::default()
    }));
    let mut initialized = vec![];
    let r = solve_with_events(&p, &n, c, |e| {
        if let SolverEvent::FirstInitialized { initialization, .. } = e {
            initialized.push(initialization)
        }
    })
    .unwrap();
    assert_eq!(initialized.len(), 1);
    assert!(!initialized[0].terminated_early);
    assert_eq!(initialized[0].kind, SolverPhaseKind::Exact);
    assert_eq!(r.assignment.values(), &[Some(3), Some(3)]);
    assert_eq!(r.phases.len(), 2);
    let token = CancellationToken::new();
    token.cancel();
    let c = SolverSequence {
        phases: vec![SolverPhase::Exact(ExactPhaseConfig {
            strategy: ExactSearchStrategy::Complete,
            cancellation: Some(&token),
            ..Default::default()
        })],
        ..Default::default()
    };
    let r = solve(&p, &n, c).unwrap();
    assert_eq!(r.termination, SolverTermination::Cancelled);
    assert_eq!(r.work.evaluations, 0);
    // A heuristic improves the first variable before completion starts. Completion
    // must retain that phase-entry state and cannot revert to the model initial.
    let mut c = config();
    c.phases
        .insert(0, SolverPhase::Construction(ConstructionConfig::default()));
    c.phases.insert(
        1,
        SolverPhase::LocalSearch(LocalSearchConfig {
            max_steps: 1,
            selection: MoveSelection::BestAccepted,
            ..Default::default()
        }),
    );
    let r = solve(&p, &n, c).unwrap();
    assert_eq!(r.assignment.values(), &[Some(3), Some(3)]);
    assert_eq!(r.phases.len(), 3);
    assert_eq!(r.phases[2].work.evaluations, 1);
    assert!(!r.is_optimal());
}
