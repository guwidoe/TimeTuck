//! Native startup policy coverage; no upstream method credit.
use std::{cell::Cell, marker::PhantomData};
use timetuck::*;
struct Zero<C>(PhantomData<C>);
impl<A, C> IncrementalCalculator<A> for Zero<C> {
    type Score = SimpleScore;
    type Justification = ();
    type Change = C;
    fn reset(
        &mut self,
        _: &A,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        Ok(())
    }
    fn calculate_score(&mut self) -> SimpleScore {
        SimpleScore::ZERO
    }
}
fn bad() -> FiniteProblem<i32> {
    FiniteProblem::new(vec![
        Variable::new(vec![1])
            .allowing_unassigned()
            .with_pinned(true),
        Variable::new(vec![1]).with_pinned(true),
        Variable::new(vec![1]),
    ])
    .unwrap()
}
fn check<A, E: std::fmt::Debug>(r: Result<A, SolverError<E>>) {
    assert!(
        matches!(
            r,
            Err(SolverError::PinnedUnassignedVariable { variable: 1 })
        ),
        "wrong startup outcome"
    );
}
#[test]
fn every_basic_and_mixed_backend_rejects_before_factories_and_budgets() {
    let p = bad();
    let m = MixedProblem::new(p.clone(), ListProblem::<i32>::new(vec![], vec![]).unwrap());
    let calls = Cell::new(0);
    let mut n = ConstraintNetwork::new(3);
    n.add_constraint("never", vec![0], |_| {
        calls.set(calls.get() + 1);
        SimpleScore::ZERO
    })
    .unwrap();
    let mut mn = MixedConstraintNetwork::new(3, 0, 0);
    mn.add_constraint("never", vec![MixedScope::Basic(0)], |_| {
        calls.set(calls.get() + 1);
        SimpleScore::ZERO
    })
    .unwrap();
    let token = CancellationToken::new();
    token.cancel();
    for zero in [false, true] {
        for exact in [false, true] {
            let limits = SolverLimits {
                reject_unassigned_pins: true,
                max_evaluations: if zero { 0 } else { 100 },
                cancellation: zero.then_some(&token),
                ..Default::default()
            };
            let plan = || SolverSequence {
                limits,
                phases: vec![if exact {
                    SolverPhase::Exact(ExactPhaseConfig {
                        strategy: ExactSearchStrategy::Complete,
                        ..Default::default()
                    })
                } else {
                    SolverPhase::Construction(ConstructionConfig::default())
                }],
            };
            let mp = || MixedSolverSequence {
                limits,
                phases: vec![if exact {
                    MixedSolverPhase::Exact(ExactPhaseConfig::default())
                } else {
                    MixedSolverPhase::Construction(MixedConstructionConfig::default())
                }],
            };
            check(solve(&p, &n, plan()));
            check(solve_stream(
                &p,
                || {
                    calls.set(calls.get() + 1);
                    let mut b = StreamGraphBuilder::<SimpleScore>::new();
                    let s = b.source();
                    Ok((b.build(), s))
                },
                plan(),
            ));
            check(solve_calculator(
                &p,
                &|| {
                    calls.set(calls.get() + 1);
                    Zero::<Vec<usize>>(PhantomData)
                },
                plan(),
            ));
            check(solve_mixed(&m, &mn, mp()));
            check(solve_mixed_stream(
                &m,
                || {
                    calls.set(calls.get() + 1);
                    let mut b = StreamGraphBuilder::<SimpleScore>::new();
                    let s = MixedStreamSources::new(&mut b);
                    Ok((b.build(), s))
                },
                mp(),
            ));
            check(solve_mixed_calculator(
                &m,
                &|| {
                    calls.set(calls.get() + 1);
                    Zero::<MixedCalculatorChange>(PhantomData)
                },
                mp(),
            ));
        }
    }
    assert_eq!(calls.get(), 0);
}

struct Never;
impl MoveProvider<FiniteProblem<i32>, Assignment<i32>, Move<i32>> for Never {
    fn event(&mut self, _: MoveProviderEvent) {
        panic!("provider event before rejection");
    }
    fn moves<'a>(
        &'a mut self,
        _: &'a FiniteProblem<i32>,
        _: &'a Assignment<i32>,
        _: u64,
    ) -> Box<dyn Iterator<Item = Move<i32>> + 'a>
    where
        Move<i32>: 'a,
    {
        panic!("provider scan before rejection");
    }
}
#[test]
fn checks_precede_provider_events_but_follow_plan_validation() {
    let p = bad();
    let n = ConstraintNetwork::<_, SimpleScore>::new(3);
    let mut provider = Never;
    let mut ext = SolverExtensions::default();
    ext.move_providers.bind(0, &mut provider);
    let limits = SolverLimits {
        reject_unassigned_pins: true,
        ..Default::default()
    };
    check(solve_with_extensions(
        &p,
        &n,
        SolverSequence {
            limits,
            phases: vec![SolverPhase::Construction(ConstructionConfig::default())],
        },
        ext,
    ));
    assert!(matches!(
        solve(
            &p,
            &n,
            SolverSequence {
                limits,
                phases: vec![]
            }
        ),
        Err(SolverError::EmptyPlan)
    ));
    assert!(matches!(
        solve(
            &p,
            &n,
            SolverSequence {
                limits,
                phases: vec![SolverPhase::Custom(CustomPhaseConfig::default())]
            }
        ),
        Err(SolverError::UnboundCustomPhase { .. })
    ));
}

#[test]
fn immovable_optional_and_partial_native_workflows_keep_their_meaning() {
    for optional in [false, true] {
        let v = Variable::<i32>::new(vec![1]).with_pinned(true);
        let p =
            FiniteProblem::new(vec![if optional { v.allowing_unassigned() } else { v }]).unwrap();
        let n = ConstraintNetwork::<_, SimpleScore>::new(1);
        let result = solve(
            &p,
            &n,
            SolverConfig {
                limits: SolverLimits {
                    reject_unassigned_pins: true,
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        if optional {
            let r = result.unwrap();
            assert!(r.complete);
            assert_eq!(r.assignment.values(), &[None]);
        } else {
            assert!(matches!(
                result,
                Err(SolverError::PinnedUnassignedVariable { variable: 0 })
            ));
        }
    }
    let p = bad();
    let n = ConstraintNetwork::<_, SimpleScore>::new(3);
    let r = solve(
        &p,
        &n,
        SolverConfig {
            local_search: vec![],
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(r.assignment.values(), &[None, None, Some(1)]);
    assert!(!r.complete);
    let p = FiniteProblem::new(vec![
        Variable::new(vec![1]).with_initial(1).with_pinned(true),
    ])
    .unwrap();
    let n = ConstraintNetwork::<_, SimpleScore>::new(1);
    let r = solve(
        &p,
        &n,
        SolverConfig {
            limits: SolverLimits {
                reject_unassigned_pins: true,
                ..Default::default()
            },
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(r.assignment.values(), &[Some(1)]);
}

#[test]
fn list_backends_accept_the_policy_without_rejecting_list_pins() {
    let p = ListProblem::new(
        vec![1],
        vec![
            ListEntity::new(vec![]).with_pinned(true),
            ListEntity::new(vec![]),
        ],
    )
    .unwrap();
    let n = ListConstraintNetwork::<_, SimpleScore>::new(2, 1);
    let plan = || SolverConfig {
        limits: SolverLimits {
            reject_unassigned_pins: true,
            ..Default::default()
        },
        local_search: vec![],
        ..Default::default()
    };
    assert!(solve_list(&p, &n, plan()).unwrap().complete);
    assert!(
        solve_list_stream(
            &p,
            || {
                let mut b = StreamGraphBuilder::<SimpleScore>::new();
                let s = ListStreamSources::new(&mut b);
                Ok((b.build(), s))
            },
            plan()
        )
        .unwrap()
        .complete
    );
    assert!(
        solve_list_calculator(&p, &|| Zero::<Vec<usize>>(PhantomData), plan())
            .unwrap()
            .complete
    );
}
