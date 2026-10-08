//! Original independent phase auditing across every solver backend.
use std::{
    cell::{Cell, RefCell},
    marker::PhantomData,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
struct Zero<C>(PhantomData<C>);
fn zero<C>() -> Zero<C> {
    Zero(PhantomData)
}
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
fn basic_plan() -> SolverSequence<'static> {
    SolverSequence {
        phases: vec![
            SolverPhase::Construction(Default::default()),
            SolverPhase::LocalSearch(LocalSearchConfig {
                max_steps: 1,
                ..Default::default()
            }),
            SolverPhase::RuinRecreate(RuinRecreateConfig {
                max_attempts: 1,
                ..Default::default()
            }),
            SolverPhase::Custom(Default::default()),
            SolverPhase::Exact(Default::default()),
        ],
        ..Default::default()
    }
}
fn mixed_plan() -> MixedSolverSequence<'static> {
    MixedSolverSequence {
        phases: vec![
            MixedSolverPhase::Construction(Default::default()),
            MixedSolverPhase::LocalSearch(MixedLocalSearchConfig {
                search: LocalSearchConfig {
                    max_steps: 1,
                    ..Default::default()
                },
                ..Default::default()
            }),
            MixedSolverPhase::RuinRecreate(MixedRuinRecreateConfig {
                search: RuinRecreateConfig {
                    max_attempts: 1,
                    ..Default::default()
                },
                ..Default::default()
            }),
            MixedSolverPhase::Custom(Default::default()),
            MixedSolverPhase::Exact(Default::default()),
        ],
        ..Default::default()
    }
}
#[test]
fn independent_checks_run_for_all_phase_kinds_and_all_nine_backends_without_changing_work() {
    let b = FiniteProblem::new(vec![Variable::new(vec![0i64, 1]).with_initial(0)]).unwrap();
    let l = ListProblem::new(vec![1i64, 2], vec![ListEntity::new(vec![1, 2])]).unwrap();
    let m = MixedProblem::new(b.clone(), l.clone());
    let bn = ConstraintNetwork::<_, SimpleScore>::new(1);
    let ln = ListConstraintNetwork::<_, SimpleScore>::new(1, 2);
    let mn = MixedConstraintNetwork::<_, _, SimpleScore>::new(1, 1, 2);
    let bg = || {
        let mut g = StreamGraphBuilder::<SimpleScore>::new();
        let s = g.source::<usize, (usize, Option<i64>)>();
        Ok((g.build(), s))
    };
    let lg = || {
        let mut g = StreamGraphBuilder::<SimpleScore>::new();
        let s = ListStreamSources::<i64>::new(&mut g);
        Ok((g.build(), s))
    };
    let mg = || {
        let mut g = StreamGraphBuilder::<SimpleScore>::new();
        let s = MixedStreamSources::<i64, i64>::new(&mut g);
        Ok((g.build(), s))
    };
    macro_rules! check {
        ($solve:ident,$p:expr,$scorer:expr,$plan:expr,$a:ty) => {{
            let mut checked = vec![];
            let mut audit = |phase, kind, a: &$a| {
                assert!($p.is_complete(a));
                checked.push((phase, kind));
                Ok(Some(SimpleScore::ZERO))
            };
            let commands = Cell::new(0);
            let events = RefCell::new(vec![]);
            let mut notify = |event: SolverEvent<'_, $a, SimpleScore>| {
                events.borrow_mut().push(format!("{event:?}"))
            };
            let mut command = |_, _: &mut CustomPhaseContext<'_, _, _, _, _>| {
                commands.set(commands.get() + 1);
                Ok(())
            };
            let plain = $solve(
                &$p,
                $scorer,
                $plan,
                SolverExtensions {
                    command: Some(&mut command),
                    events: Some(&mut notify),
                    ..Default::default()
                },
            )
            .unwrap();
            let expected_events = events.take();
            assert_eq!(commands.get(), 1);
            let mut audited_plan = $plan;
            audited_plan.limits.assert_phase_scores = true;
            let audited = $solve(
                &$p,
                $scorer,
                audited_plan,
                SolverExtensions {
                    command: Some(&mut command),
                    events: Some(&mut notify),
                    score_check: Some(&mut audit),
                    ..Default::default()
                },
            )
            .unwrap();
            assert_eq!(commands.get(), 2);
            assert_eq!(*events.borrow(), expected_events);
            assert_eq!(audited, plain);
            assert_eq!(
                checked,
                vec![
                    (0, SolverPhaseKind::Construction),
                    (1, SolverPhaseKind::LocalSearch),
                    (2, SolverPhaseKind::RuinRecreate),
                    (3, SolverPhaseKind::Custom),
                    (4, SolverPhaseKind::Exact),
                ]
            );
        }};
    }
    check!(solve_with_extensions, b, &bn, basic_plan(), Assignment<i64>);
    check!(
        solve_stream_with_extensions,
        b,
        bg,
        basic_plan(),
        Assignment<i64>
    );
    check!(
        solve_calculator_with_extensions,
        b,
        &zero::<Vec<usize>>,
        basic_plan(),
        Assignment<i64>
    );
    check!(
        solve_list_with_extensions,
        l,
        &ln,
        basic_plan(),
        ListAssignment<i64>
    );
    check!(
        solve_list_stream_with_extensions,
        l,
        lg,
        basic_plan(),
        ListAssignment<i64>
    );
    check!(
        solve_list_calculator_with_extensions,
        l,
        &zero::<Vec<usize>>,
        basic_plan(),
        ListAssignment<i64>
    );
    check!(solve_mixed_with_extensions,m,&mn,mixed_plan(),MixedAssignment<i64,i64>);
    check!(solve_mixed_stream_with_extensions,m,mg,mixed_plan(),MixedAssignment<i64,i64>);
    check!(solve_mixed_calculator_with_extensions,m,&zero::<MixedCalculatorChange>,mixed_plan(),MixedAssignment<i64,i64>);
}
#[test]
fn mismatch_error_and_panic_stop_handoff_without_retracting_prior_best_events() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap();
    let n = ConstraintNetwork::<_, SimpleScore>::new(1);
    for mode in 0..3 {
        let order = RefCell::new(vec![]);
        let mut audit = |phase, kind, _: &Assignment<i32>| {
            assert_eq!((phase, kind), (0, SolverPhaseKind::Construction));
            order.borrow_mut().push("check");
            match mode {
                0 => Ok(Some(SimpleScore::new(1))),
                1 => Err("independent scorer failed".to_string()),
                _ => panic!("audit panic"),
            }
        };
        let mut events = |e: SolverEvent<'_, Assignment<i32>, SimpleScore>| match e {
            SolverEvent::Best(_) => order.borrow_mut().push("best"),
            SolverEvent::FirstInitialized { .. } => {
                panic!("failed validation must precede initialization")
            }
        };
        let result = catch_unwind(AssertUnwindSafe(|| {
            solve_with_extensions(
                &p,
                &n,
                SolverSequence::default(),
                SolverExtensions {
                    score_check: Some(&mut audit),
                    events: Some(&mut events),
                    ..Default::default()
                },
            )
        }));
        match mode {
            0 => assert!(
                matches!(result.unwrap(),Err(SolverError::PhaseScoreMismatch { phase:0,kind:SolverPhaseKind::Construction,actual,expected }) if actual.contains('0') && expected.contains('1'))
            ),
            1 => assert!(
                matches!(result.unwrap(),Err(SolverError::PhaseScoreCheckFailed { phase:0,reason,.. }) if reason=="independent scorer failed")
            ),
            _ => assert!(result.is_err()),
        }
        assert_eq!(*order.borrow(), ["best", "check"]);
    }
    let mut skip = |_, _, _: &Assignment<i32>| Ok(None);
    solve_with_extensions(
        &p,
        &n,
        SolverSequence::default(),
        SolverExtensions {
            score_check: Some(&mut skip),
            ..Default::default()
        },
    )
    .unwrap();
}
#[test]
fn partial_unscored_and_initial_only_outputs_have_explicit_audit_semantics() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]); 2]).unwrap();
    let n = ConstraintNetwork::<_, SimpleScore>::new(2);
    for limit in [0, 1] {
        let mut seen = vec![];
        let mut audit = |phase, kind, a: &Assignment<i32>| {
            let missing = a.values().iter().filter(|v| v.is_none()).count() as i64;
            seen.push((phase, kind, missing));
            Ok(Some(SimpleScore::ZERO.with_structural(-missing)))
        };
        let plan = SolverSequence {
            limits: SolverLimits {
                assert_phase_scores: true,
                ..Default::default()
            },
            phases: vec![SolverPhase::Construction(ConstructionConfig {
                max_evaluations: limit,
                ..Default::default()
            })],
        };
        let r = solve_with_extensions(
            &p,
            &n,
            plan,
            SolverExtensions {
                score_check: Some(&mut audit),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(seen.len(), usize::from(r.score.is_some()));
        if let Some(&(phase, kind, missing)) = seen.first() {
            assert_eq!((phase, kind), (0, SolverPhaseKind::Construction));
            assert_eq!(r.score.unwrap().structural(), -missing);
        }
    }
    let p = FiniteProblem::new(vec![
        Variable::new(vec![0, 1]).with_initial(0).with_pinned(true),
    ])
    .unwrap();
    let n = ConstraintNetwork::<_, SimpleScore>::new(1);
    let calls = Cell::new(0);
    let mut audit = |phase, kind, _: &Assignment<i32>| {
        assert_eq!((phase, kind), (0, SolverPhaseKind::Initial));
        calls.set(calls.get() + 1);
        Ok(Some(SimpleScore::ZERO))
    };
    let r = solve_with_extensions(
        &p,
        &n,
        SolverSequence {
            limits: SolverLimits {
                assert_phase_scores: true,
                ..Default::default()
            },
            ..Default::default()
        },
        SolverExtensions {
            score_check: Some(&mut audit),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(r.phases.is_empty());
    assert_eq!(calls.get(), 1);
}
#[derive(Default)]
struct Corrupt(i64);
impl IncrementalCalculator<Assignment<i32>> for Corrupt {
    type Score = SimpleScore;
    type Justification = ();
    type Change = Vec<usize>;
    fn reset(
        &mut self,
        _: &Assignment<i32>,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.0 = 0;
        Ok(())
    }
    fn after_change(
        &mut self,
        _: &Assignment<i32>,
        _: &Vec<usize>,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.0 += 10;
        Ok(())
    }
    fn calculate_score(&mut self) -> SimpleScore {
        SimpleScore::new(self.0)
    }
}
#[test]
fn independent_phase_score_detects_incremental_corruption_and_supports_phase_selection() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap();
    let plan = SolverSequence {
        phases: vec![SolverPhase::LocalSearch(LocalSearchConfig {
            max_steps: 1,
            ..Default::default()
        })],
        ..Default::default()
    };
    let plain = solve_calculator(&p, &Corrupt::default, plan.clone()).unwrap();
    assert!(plain.score.unwrap() > SimpleScore::ZERO);
    let mut audit = |_, _, _: &Assignment<i32>| Ok(Some(SimpleScore::ZERO));
    let result = solve_calculator_with_extensions(
        &p,
        &Corrupt::default,
        plan,
        SolverExtensions {
            score_check: Some(&mut audit),
            ..Default::default()
        },
    );
    assert!(matches!(
        result,
        Err(SolverError::PhaseScoreMismatch {
            phase: 0,
            kind: SolverPhaseKind::LocalSearch,
            ..
        })
    ));
    let n = ConstraintNetwork::<_, SimpleScore>::new(1);
    let calls = RefCell::new(vec![]);
    let mut select = |i, _, _: &Assignment<i32>| {
        calls.borrow_mut().push(i);
        Ok((i == 1).then_some(SimpleScore::ZERO))
    };
    solve_with_extensions(
        &p,
        &n,
        SolverSequence::default(),
        SolverExtensions {
            score_check: Some(&mut select),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(*calls.borrow(), [0, 1]);
}
