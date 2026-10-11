//! Original failure-context and cleanup contracts; no upstream method credit.
use std::{
    cell::{Cell, RefCell},
    marker::PhantomData,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;
#[derive(Clone)]
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
fn plan() -> SolverSequence<'static> {
    SolverSequence {
        phases: vec![
            SolverPhase::Construction(Default::default()),
            SolverPhase::Custom(Default::default()),
        ],
        ..Default::default()
    }
}
fn mixed_plan() -> MixedSolverSequence<'static> {
    MixedSolverSequence {
        phases: vec![
            MixedSolverPhase::Construction(Default::default()),
            MixedSolverPhase::Custom(Default::default()),
        ],
        ..Default::default()
    }
}
#[test]
fn custom_failure_retains_completed_handoff_on_all_eighteen_routes() {
    let b = FiniteProblem::new(vec![Variable::new(vec![0i64, 1])]).unwrap();
    let l = ListProblem::new(vec![1i64, 2], vec![ListEntity::new(vec![])]).unwrap();
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
        ($p:expr,$a:ty,$ext:ident,$invoke:expr) => {{
            let order=RefCell::new(Vec::new());
            let calls=Cell::new(0);
            let mut failure=|context:SolverFailure<'_,$a,SimpleScore>| {
                calls.set(calls.get()+1);
                assert!($p.is_complete(context.assignment));
                assert!(context.complete);
                assert_eq!(context.score,Some(&SimpleScore::ZERO));
                assert_eq!(context.phase,Some(SolverEventProducer {phase:1,kind:SolverPhaseKind::Custom}));
                assert_eq!(context.phases.len(),1);
                assert_eq!(context.phases[0].index,0);
                assert_eq!(context.work,context.phases[0].work);
                assert!(context.work.evaluations>0);
                assert!(format!("{:?}",context.error).contains("command failed"));
                order.borrow_mut().push("failure".into());
            };
            let mut lifecycle=|event| order.borrow_mut().push(format!("{event:?}"));
            let mut command=|_,_: &mut CustomPhaseContext<'_,_,_,_,_>| Err(CustomPhaseError::Command("command failed".into()));
            let $ext=SolverExtensions { failure:Some(&mut failure), lifecycle:Some(&mut lifecycle), command:Some(&mut command), ..Default::default() };
            let outcome=$invoke;
            assert!(matches!(outcome,Err(SolverError::Custom {phase:1,error:CustomPhaseError::Command(ref s)}) if s=="command failed"));
            assert_eq!(calls.get(),1);
            let order=order.borrow();
            assert_eq!(&order[order.len()-2..],["failure","SolveEnded"]);
        }};
    }
    check!(
        b,
        Assignment<i64>,
        ext,
        solve_with_extensions(&b, &bn, plan(), ext)
    );
    check!(
        b,
        Assignment<i64>,
        ext,
        solve_stream_with_extensions(&b, bg, plan(), ext)
    );
    check!(
        b,
        Assignment<i64>,
        ext,
        solve_calculator_with_extensions(&b, &zero::<Vec<usize>>, plan(), ext)
    );
    check!(
        l,
        ListAssignment<i64>,
        ext,
        solve_list_with_extensions(&l, &ln, plan(), ext)
    );
    check!(
        l,
        ListAssignment<i64>,
        ext,
        solve_list_stream_with_extensions(&l, lg, plan(), ext)
    );
    check!(
        l,
        ListAssignment<i64>,
        ext,
        solve_list_calculator_with_extensions(&l, &zero::<Vec<usize>>, plan(), ext)
    );
    check!(m,MixedAssignment<i64,i64>,ext,solve_mixed_with_extensions(&m,&mn,mixed_plan(),ext));
    check!(m,MixedAssignment<i64,i64>,ext,solve_mixed_stream_with_extensions(&m,mg,mixed_plan(),ext));
    check!(m,MixedAssignment<i64,i64>,ext,solve_mixed_calculator_with_extensions(&m,&zero::<MixedCalculatorChange>,mixed_plan(),ext));
    let mut d = ScoreDirector::new(&b, &bn, b.initial_assignment()).unwrap();
    check!(
        b,
        Assignment<i64>,
        ext,
        d.run_solver_with_extensions(plan(), ext)
    );
    let (g, s) = bg().unwrap();
    let mut d = StreamScoreDirector::new(&b, g, s, b.initial_assignment()).unwrap();
    check!(
        b,
        Assignment<i64>,
        ext,
        d.run_solver_with_extensions(plan(), ext)
    );
    let mut d = CalculatorScoreDirector::new(&b, b.initial_assignment(), zero::<Vec<usize>>, false)
        .unwrap();
    check!(
        b,
        Assignment<i64>,
        ext,
        d.run_solver_with_extensions(plan(), ext)
    );
    let mut d = ListScoreDirector::new(&l, &ln, l.initial_assignment()).unwrap();
    check!(
        l,
        ListAssignment<i64>,
        ext,
        d.run_solver_with_extensions(plan(), ext)
    );
    let (g, s) = lg().unwrap();
    let mut d = ListStreamScoreDirector::new(&l, g, s, l.initial_assignment()).unwrap();
    check!(
        l,
        ListAssignment<i64>,
        ext,
        d.run_solver_with_extensions(plan(), ext)
    );
    let mut d =
        ListCalculatorScoreDirector::new(&l, l.initial_assignment(), zero::<Vec<usize>>, false)
            .unwrap();
    check!(
        l,
        ListAssignment<i64>,
        ext,
        d.run_solver_with_extensions(plan(), ext)
    );
    let mut d = MixedScoreDirector::new(&m, &mn, m.initial_assignment()).unwrap();
    check!(m,MixedAssignment<i64,i64>,ext,d.run_solver_with_extensions(mixed_plan(),ext));
    let (g, s) = mg().unwrap();
    let mut d = MixedStreamScoreDirector::new(&m, g, s, m.initial_assignment()).unwrap();
    check!(m,MixedAssignment<i64,i64>,ext,d.run_solver_with_extensions(mixed_plan(),ext));
    let mut d = MixedCalculatorScoreDirector::new(
        &m,
        m.initial_assignment(),
        zero::<MixedCalculatorChange>,
        false,
    )
    .unwrap();
    check!(m,MixedAssignment<i64,i64>,ext,d.run_solver_with_extensions(mixed_plan(),ext));
}

#[test]
fn diagnostic_rejection_does_not_promote_unverified_output_or_work() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]).with_initial(0)]).unwrap();
    let mut n = ConstraintNetwork::new(1);
    n.add_constraint("value", vec![0], |v| {
        SimpleScore::new(i64::from(*v[0].unwrap()))
    })
    .unwrap();
    for reject in 0..2 {
        let mut command = |phase,
                           ctx: &mut CustomPhaseContext<
            '_,
            Assignment<i32>,
            Move<i32>,
            SimpleScore,
            ScoringError,
        >| {
            ctx.execute(&Move::Change {
                variable: 0,
                value: Some(phase as i32 + 1),
            })?;
            Ok(())
        };
        let mut check = |phase, _, _: &Assignment<i32>| {
            if phase == reject {
                Err("audit failed".into())
            } else {
                Ok(None)
            }
        };
        let calls = Cell::new(0);
        let mut failure = |c: SolverFailure<'_, Assignment<i32>, SimpleScore>| {
            calls.set(calls.get() + 1);
            assert_eq!(c.assignment.values(), &[Some(reject as i32)]);
            assert_eq!(
                c.score.copied(),
                if reject == 0 {
                    None
                } else {
                    Some(SimpleScore::new(1))
                }
            );
            assert_eq!(c.phases.len(), reject);
            if reject == 0 {
                assert_eq!(c.work, SolverWork::default());
            } else {
                assert_eq!(c.work, c.phases[0].work);
            }
        };
        let result = solve_with_extensions(
            &p,
            &n,
            SolverSequence {
                phases: vec![SolverPhase::Custom(Default::default()); 2],
                ..Default::default()
            },
            SolverExtensions {
                command: Some(&mut command),
                score_check: Some(&mut check),
                failure: Some(&mut failure),
                ..Default::default()
            },
        );
        assert!(
            matches!(result,Err(SolverError::PhaseScoreCheckFailed {phase,ref reason,..}) if phase==reject && reason=="audit failed")
        );
        assert_eq!(calls.get(), 1);
    }
}

#[test]
fn construction_and_immovable_initial_scoring_failures_keep_unscored_initial_state() {
    for pinned in [false, true] {
        let p = FiniteProblem::new(vec![if pinned {
            Variable::new(vec![0i64]).with_initial(0).with_pinned(true)
        } else {
            Variable::new(vec![0i64])
        }])
        .unwrap();
        let mut n = ConstraintNetwork::new(1);
        n.add_constraint("a", vec![0], |_| SimpleScore::new(i64::MAX))
            .unwrap();
        n.add_constraint("b", vec![0], |_| SimpleScore::new(1))
            .unwrap();
        let calls = Cell::new(0);
        let mut failure = |c: SolverFailure<'_, Assignment<i64>, SimpleScore>| {
            calls.set(calls.get() + 1);
            assert_eq!(c.assignment, &p.initial_assignment());
            assert!(c.score.is_none());
            assert!(c.phases.is_empty());
            assert_eq!(c.work, SolverWork::default());
            assert_eq!(c.complete, pinned);
            assert_eq!(
                c.phase.unwrap().kind,
                if pinned {
                    SolverPhaseKind::Initial
                } else {
                    SolverPhaseKind::Construction
                }
            );
        };
        assert!(
            solve_with_extensions(
                &p,
                &n,
                SolverConfig::default(),
                SolverExtensions {
                    failure: Some(&mut failure),
                    ..Default::default()
                }
            )
            .is_err()
        );
        assert_eq!(calls.get(), 1);
    }
}

#[test]
fn local_provider_failure_runs_before_provider_solve_teardown() {
    struct Provider<'a>(&'a RefCell<Vec<&'static str>>);
    impl MoveProvider<FiniteProblem<i32>, Assignment<i32>, Move<i32>> for Provider<'_> {
        fn moves<'a>(
            &'a mut self,
            _: &'a FiniteProblem<i32>,
            _: &'a Assignment<i32>,
            _: u64,
        ) -> Box<dyn Iterator<Item = Move<i32>> + 'a>
        where
            Move<i32>: 'a,
        {
            Box::new(std::iter::once(Move::Change {
                variable: 999,
                value: Some(1),
            }))
        }
        fn event(&mut self, event: MoveProviderEvent) {
            if event == MoveProviderEvent::SolveEnded {
                self.0.borrow_mut().push("provider ended");
            }
        }
    }
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap();
    let n = ConstraintNetwork::<_, SimpleScore>::new(1);
    let order = RefCell::new(Vec::new());
    let mut provider = Provider(&order);
    let mut lifecycle = |event| {
        if event == SolverLifecycleEvent::SolveEnded {
            order.borrow_mut().push("solve ended")
        }
    };
    let mut failure = |c: SolverFailure<'_, Assignment<i32>, SimpleScore>| {
        assert_eq!(c.phase.unwrap().kind, SolverPhaseKind::LocalSearch);
        assert!(c.score.is_none());
        order.borrow_mut().push("failure");
    };
    let mut ext = SolverExtensions {
        failure: Some(&mut failure),
        lifecycle: Some(&mut lifecycle),
        ..Default::default()
    };
    ext.move_providers.bind(0, &mut provider);
    let result = solve_with_extensions(
        &p,
        &n,
        SolverSequence {
            phases: vec![SolverPhase::LocalSearch(Default::default())],
            ..Default::default()
        },
        ext,
    );
    assert!(result.is_err());
    assert_eq!(
        *order.borrow(),
        ["failure", "solve ended", "provider ended"]
    );
}

#[test]
fn preflight_errors_and_panics_do_not_notify_and_failure_callback_panics_propagate() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap();
    let n = ConstraintNetwork::<_, SimpleScore>::new(1);
    for mode in 0..4 {
        let calls = Cell::new(0);
        let ended = Cell::new(false);
        let mut failure = |_: SolverFailure<'_, Assignment<i32>, SimpleScore>| {
            calls.set(calls.get() + 1);
            if mode == 3 {
                panic!("failure observer panic");
            }
        };
        let mut lifecycle = |event| {
            if event == SolverLifecycleEvent::SolveStarted && mode == 1 {
                panic!("start panic");
            }
            if event == SolverLifecycleEvent::SolveEnded {
                ended.set(true);
            }
        };
        let mut command = |_,
                           _: &mut CustomPhaseContext<
            '_,
            Assignment<i32>,
            Move<i32>,
            SimpleScore,
            ScoringError,
        >| {
            if mode == 2 {
                panic!("command panic");
            }
            Err(CustomPhaseError::Command("original error".into()))
        };
        let result = catch_unwind(AssertUnwindSafe(|| {
            solve_with_extensions(
                &p,
                &n,
                SolverSequence {
                    phases: if mode == 0 {
                        vec![]
                    } else {
                        vec![SolverPhase::Custom(Default::default())]
                    },
                    ..Default::default()
                },
                SolverExtensions {
                    failure: Some(&mut failure),
                    lifecycle: Some(&mut lifecycle),
                    command: Some(&mut command),
                    ..Default::default()
                },
            )
        }));
        if mode == 0 {
            assert!(matches!(result.unwrap(), Err(SolverError::EmptyPlan)));
        } else {
            assert!(result.is_err());
        }
        assert_eq!(calls.get(), usize::from(mode == 3));
        assert!(!ended.get());
    }
}

#[test]
fn successful_observation_adds_no_assignment_clones_or_score_calls() {
    struct Value {
        id: i64,
        clones: Rc<Cell<usize>>,
    }
    impl Clone for Value {
        fn clone(&self) -> Self {
            self.clones.set(self.clones.get() + 1);
            Self {
                id: self.id,
                clones: self.clones.clone(),
            }
        }
    }
    impl PartialEq for Value {
        fn eq(&self, other: &Self) -> bool {
            self.id == other.id
        }
    }
    impl Eq for Value {}
    let clones = Rc::new(Cell::new(0));
    let scores = Cell::new(0);
    let p = FiniteProblem::new(vec![Variable::new(
        (0..3)
            .map(|id| Value {
                id,
                clones: clones.clone(),
            })
            .collect(),
    )])
    .unwrap();
    let mut n = ConstraintNetwork::<Value, SimpleScore>::new(1);
    n.add_constraint("value", vec![0], |v| {
        scores.set(scores.get() + 1);
        SimpleScore::new(v[0].map_or(0, |v| v.id))
    })
    .unwrap();
    let mut results = Vec::new();
    for observed in [false, true] {
        clones.set(0);
        scores.set(0);
        let mut failure = |_: SolverFailure<'_, Assignment<Value>, SimpleScore>| {
            panic!("successful solve notified failure")
        };
        let result = solve_with_extensions(
            &p,
            &n,
            SolverConfig {
                local_search: vec![LocalSearchConfig {
                    max_steps: 2,
                    ..Default::default()
                }],
                ..Default::default()
            },
            SolverExtensions {
                failure: observed.then_some(&mut failure),
                ..Default::default()
            },
        )
        .unwrap();
        results.push((result.score, result.work, clones.get(), scores.get()));
    }
    assert_eq!(results[0], results[1]);
}

#[test]
fn failure_observation_respects_retained_and_fresh_resource_lifetimes() {
    struct Tracked(Rc<Cell<usize>>);
    impl Tracked {
        fn new(count: &Rc<Cell<usize>>) -> Self {
            count.set(count.get() + 1);
            Self(count.clone())
        }
    }
    impl Clone for Tracked {
        fn clone(&self) -> Self {
            Self::new(&self.0)
        }
    }
    impl Drop for Tracked {
        fn drop(&mut self) {
            self.0.set(self.0.get() - 1);
        }
    }
    impl IncrementalCalculator<Assignment<i32>> for Tracked {
        type Score = SimpleScore;
        type Justification = ();
        type Change = Vec<usize>;
        fn reset(
            &mut self,
            _: &Assignment<i32>,
            _: &mut MatchRegistry<SimpleScore, ()>,
        ) -> Result<(), MatchRegistryError> {
            Ok(())
        }
        fn calculate_score(&mut self) -> SimpleScore {
            SimpleScore::ZERO
        }
    }
    for retained in [false, true] {
        let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap();
        let live = Rc::new(Cell::new(0));
        let seen = RefCell::new(None);
        let mut failure = |context: SolverFailure<'_, Assignment<i32>, SimpleScore>| {
            assert_eq!(live.get() > 0, retained);
            *seen.borrow_mut() = Some(format!("{:?}", context.error));
        };
        let mut command = |_,
                           _: &mut CustomPhaseContext<
            '_,
            Assignment<i32>,
            Move<i32>,
            SimpleScore,
            ScoringError,
        >| Err(CustomPhaseError::Command("original error".into()));
        let ext = SolverExtensions {
            failure: Some(&mut failure),
            command: Some(&mut command),
            ..Default::default()
        };
        let factory = || Tracked::new(&live);
        let result = if retained {
            let mut d =
                CalculatorScoreDirector::new(&p, p.initial_assignment(), factory, false).unwrap();
            let result = d.run_solver_with_extensions(plan(), ext);
            assert!(live.get() > 0);
            drop(d);
            result
        } else {
            solve_calculator_with_extensions(&p, &factory, plan(), ext)
        };
        let error = result.unwrap_err();
        assert_eq!(seen.into_inner().unwrap(), format!("{error:?}"));
        assert_eq!(live.get(), 0);
    }
}

#[test]
fn failed_custom_candidate_never_replaces_the_prior_retained_handoff() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]).with_initial(0)]).unwrap();
    let mut n = ConstraintNetwork::new(1);
    n.add_constraint("value", vec![0], |v| {
        SimpleScore::new(i64::from(*v[0].unwrap()))
    })
    .unwrap();
    let mut command = |phase,
                       ctx: &mut CustomPhaseContext<
        '_,
        Assignment<i32>,
        Move<i32>,
        SimpleScore,
        ScoringError,
    >| {
        ctx.execute(&Move::Change {
            variable: 0,
            value: Some(phase as i32 + 1),
        })?;
        if phase == 1 {
            Err(CustomPhaseError::Command("after candidate".into()))
        } else {
            Ok(())
        }
    };
    let calls = Cell::new(0);
    let mut failure = |c: SolverFailure<'_, Assignment<i32>, SimpleScore>| {
        calls.set(calls.get() + 1);
        assert_eq!(c.assignment.values(), &[Some(1)]);
        assert_eq!(c.score, Some(&SimpleScore::new(1)));
        assert_eq!(c.phases.len(), 1);
        assert_eq!(c.work, c.phases[0].work);
    };
    let result = solve_with_extensions(
        &p,
        &n,
        SolverSequence {
            phases: vec![SolverPhase::Custom(Default::default()); 2],
            ..Default::default()
        },
        SolverExtensions {
            command: Some(&mut command),
            failure: Some(&mut failure),
            ..Default::default()
        },
    );
    assert!(
        matches!(result,Err(SolverError::Custom{phase:1,error:CustomPhaseError::Command(ref s)}) if s=="after candidate")
    );
    assert_eq!(calls.get(), 1);
}

#[test]
fn incomplete_handoff_failure_exposes_retained_partial_and_completed_phase_work() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]); 2]).unwrap();
    let n = ConstraintNetwork::<_, SimpleScore>::new(2);
    let calls = Cell::new(0);
    let mut failure = |c: SolverFailure<'_, Assignment<i32>, SimpleScore>| {
        calls.set(calls.get() + 1);
        assert!(!c.complete);
        assert!(c.score.is_some());
        assert_eq!(
            c.assignment.values().iter().filter(|v| v.is_some()).count(),
            1
        );
        assert_eq!(c.phases.len(), 1);
        assert_eq!(c.work, c.phases[0].work);
        assert_eq!(
            c.phase,
            Some(SolverEventProducer {
                phase: 0,
                kind: SolverPhaseKind::Construction
            })
        );
    };
    let result = solve_with_extensions(
        &p,
        &n,
        SolverConfig {
            construction: ConstructionConfig {
                max_steps: Some(1),
                ..Default::default()
            },
            limits: SolverLimits {
                incomplete_handoff: IncompleteHandoffPolicy::Error,
                ..Default::default()
            },
            ..Default::default()
        },
        SolverExtensions {
            failure: Some(&mut failure),
            ..Default::default()
        },
    );
    assert!(matches!(
        result,
        Err(SolverError::IncompleteLocalSearch { phase: 1 })
    ));
    assert_eq!(calls.get(), 1);
}
