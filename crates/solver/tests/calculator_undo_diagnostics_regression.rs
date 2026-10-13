//! Original actual-trial state/score fault injection across all calculator models.
use std::{cell::Cell, marker::PhantomData, rc::Rc};
use timetuck::*;
trait Position {
    type Change;
    fn marker(&self) -> i64;
}
impl Position for Assignment<i64> {
    type Change = Vec<usize>;
    fn marker(&self) -> i64 {
        self.values().iter().flatten().sum()
    }
}
impl Position for ListAssignment<i64> {
    type Change = Vec<usize>;
    fn marker(&self) -> i64 {
        self.lists()
            .iter()
            .enumerate()
            .map(|(i, row)| i as i64 * row.iter().sum::<i64>())
            .sum()
    }
}
impl Position for MixedAssignment<i64, i64> {
    type Change = MixedCalculatorChange;
    fn marker(&self) -> i64 {
        self.basic().marker() + self.lists().marker()
    }
}
#[derive(Clone)]
struct Fault<A> {
    policy: CalculatorUndoDiagnostics,
    root: i64,
    changed: bool,
    bad: bool,
    score_fault: bool,
    field_fault: bool,
    captures: Rc<Cell<usize>>,
    marker: PhantomData<A>,
}
impl<A> Fault<A> {
    fn new(
        policy: CalculatorUndoDiagnostics,
        score_fault: bool,
        field_fault: bool,
        captures: Rc<Cell<usize>>,
    ) -> Self {
        Self {
            policy,
            root: 0,
            changed: false,
            bad: false,
            score_fault,
            field_fault,
            captures,
            marker: PhantomData,
        }
    }
}
fn field() -> CalculatorUndoField {
    CalculatorUndoField {
        entity_key: "0".into(),
        entity_type: "Entity".into(),
        entity_description: "entity0".into(),
        variable: "derived".into(),
    }
}
impl<A: Position> IncrementalCalculator<A> for Fault<A> {
    type Score = SimpleScore;
    fn undo_diagnostics(&self) -> CalculatorUndoDiagnostics {
        self.policy
    }
    type Change = A::Change;
    type Justification = ();
    fn reset(
        &mut self,
        a: &A,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.root = a.marker();
        self.changed = false;
        self.bad = false;
        Ok(())
    }
    fn after_change(
        &mut self,
        a: &A,
        _: &A::Change,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.bad = self.changed && a.marker() == self.root;
        self.changed = true;
        Ok(())
    }
    fn calculate_score(&mut self) -> SimpleScore {
        SimpleScore::new(i64::from(self.score_fault && self.bad))
    }
    fn capture_undo_state(&self) -> Result<Option<CalculatorStateSnapshot>, MatchRegistryError> {
        self.captures.set(self.captures.get() + 1);
        let mut s = CalculatorStateSnapshot::new();
        let value = self.field_fault && self.bad;
        s.capture(field(), value, value.to_string())?;
        Ok(Some(s))
    }
}
fn basic() -> FiniteProblem<i64> {
    FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap()
}
fn lists() -> ListProblem<i64> {
    ListProblem::new(
        vec![1],
        vec![ListEntity::new(vec![1]), ListEntity::new(vec![])],
    )
    .unwrap()
}
fn movement() -> ListMove<i64> {
    ListMove::Change {
        source: ListPosition {
            entity: 0,
            index: 0,
        },
        destination: ListPosition {
            entity: 1,
            index: 0,
        },
    }
}
#[test]
fn score_and_tracked_policies_detect_actual_undo_in_all_three_models() {
    for policy in [
        CalculatorUndoDiagnostics::Score,
        CalculatorUndoDiagnostics::Tracked,
    ] {
        for score_fault in [false, true] {
            if !score_fault && policy == CalculatorUndoDiagnostics::Score {
                continue;
            }
            let captures = Rc::new(Cell::new(0));
            let bp = basic();
            let lp = lists();
            let mp = MixedProblem::new(bp.clone(), lp.clone());
            let cfg = LocalSearchConfig {
                max_steps: 1,
                max_evaluations: 2,
                ..Default::default()
            };
            let b = calculator_local_search_with_provider(
                &bp,
                bp.initial_assignment(),
                || Fault::<Assignment<i64>>::new(policy, score_fault, true, captures.clone()),
                cfg.clone(),
                &mut |_: &FiniteProblem<i64>, _: &Assignment<i64>, _: u64| {
                    vec![Move::Change {
                        variable: 0,
                        value: Some(1),
                    }]
                },
            );
            let l = list_calculator_local_search_with_provider(
                &lp,
                lp.initial_assignment(),
                || Fault::<ListAssignment<i64>>::new(policy, score_fault, true, captures.clone()),
                cfg.clone(),
                &mut |_: &ListProblem<i64>, _: &ListAssignment<i64>, _: u64| vec![movement()],
            );
            let m = mixed_calculator_local_search_with_provider(
                &mp,
                mp.initial_assignment(),
                || {
                    Fault::<MixedAssignment<i64, i64>>::new(
                        policy,
                        score_fault,
                        true,
                        captures.clone(),
                    )
                },
                MixedLocalSearchConfig {
                    search: cfg,
                    ..Default::default()
                },
                &mut |_: &MixedProblem<i64, i64>, _: &MixedAssignment<i64, i64>, _: u64| {
                    vec![MixedMove {
                        basic: Move::Composite(vec![]),
                        lists: movement(),
                    }]
                },
            );
            let b = b.unwrap_err();
            let l = l.unwrap_err();
            let m = m.unwrap_err();
            let reports = match (b, l, m) {
                (
                    LocalSearchError::Scoring(ScoringError::UndoCorruption(b)),
                    ListLocalSearchError::Scoring(ListScoringError::UndoCorruption(l)),
                    MixedLocalSearchError::Scoring(MixedScoringError::UndoCorruption(m)),
                ) => [b, l, m],
                other => panic!("unexpected diagnostics {other:?}"),
            };
            for r in reports {
                assert_eq!(r.score.is_some(), score_fault);
                assert_eq!(
                    !r.fields.is_empty(),
                    policy == CalculatorUndoDiagnostics::Tracked
                );
                assert!(r.to_string().contains("corrupted undoMove"));
                if policy == CalculatorUndoDiagnostics::Tracked {
                    assert_eq!(r.fields[0].expected.as_deref(), Some("false"));
                    assert_eq!(r.fields[0].actual.as_deref(), Some("true"));
                }
            }
            assert_eq!(
                captures.get(),
                if policy == CalculatorUndoDiagnostics::Tracked {
                    6
                } else {
                    0
                }
            );
        }
    }
}
#[test]
fn default_and_disabled_modes_do_not_capture_and_healthy_tracked_trials_succeed() {
    let p = basic();
    for policy in [
        CalculatorUndoDiagnostics::Default,
        CalculatorUndoDiagnostics::Disabled,
        CalculatorUndoDiagnostics::Tracked,
    ] {
        let calls = Rc::new(Cell::new(0));
        let mut d = CalculatorScoreDirector::new(
            &p,
            p.initial_assignment(),
            || Fault::<Assignment<i64>>::new(policy, false, false, calls.clone()),
            false,
        )
        .unwrap()
        .with_undo_diagnostics(policy);
        let r = d.run_solver(SolverSequence {
            phases: vec![SolverPhase::LocalSearch(LocalSearchConfig {
                max_steps: 1,
                max_evaluations: 2,
                ..Default::default()
            })],
            ..Default::default()
        });
        assert!(r.is_ok());
        assert_eq!(
            calls.get(),
            if policy == CalculatorUndoDiagnostics::Tracked {
                2
            } else {
                0
            }
        );
    }
}
#[derive(Eq, PartialEq)]
struct Hidden(u32);
#[test]
fn typed_values_and_missing_fields_are_detected_even_with_equal_display_strings() {
    let mut a = CalculatorStateSnapshot::new();
    let mut b = CalculatorStateSnapshot::new();
    a.capture(field(), Hidden(1), "same".into()).unwrap();
    b.capture(field(), Hidden(2), "same".into()).unwrap();
    assert_eq!(a.differences(&b).len(), 1);
    assert_eq!(
        a.differences(&CalculatorStateSnapshot::new())[0].actual,
        None
    );
    assert_eq!(
        CalculatorStateSnapshot::new().differences(&b)[0].expected,
        None
    );
    assert!(a.capture(field(), Hidden(2), "replacement".into()).is_err());
    assert_eq!(a.differences(&b)[0].expected.as_deref(), Some("same"));
}
struct Unsupported;
impl IncrementalCalculator<Assignment<i64>> for Unsupported {
    type Score = SimpleScore;
    type Change = Vec<usize>;
    type Justification = ();
    fn undo_diagnostics(&self) -> CalculatorUndoDiagnostics {
        CalculatorUndoDiagnostics::Tracked
    }
    fn reset(
        &mut self,
        _: &Assignment<i64>,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        Ok(())
    }
    fn calculate_score(&mut self) -> SimpleScore {
        SimpleScore::ZERO
    }
}
#[test]
fn unsupported_tracking_is_reported_before_a_trial_without_requiring_clone() {
    let p = basic();
    let e = calculator_local_search_from(
        &p,
        p.initial_assignment(),
        || Unsupported,
        LocalSearchConfig {
            max_steps: 1,
            max_evaluations: 2,
            ..Default::default()
        },
    )
    .unwrap_err();
    assert!(
        matches!(e,LocalSearchError::Scoring(ScoringError::Calculator(MatchRegistryError::Calculator(ref m))) if m.contains("unsupported"))
    );
}
#[test]
fn rolled_back_custom_previews_use_actual_tracked_state_but_commits_do_not_check_undo() {
    let p = basic();
    let calls = Rc::new(Cell::new(0));
    let mut d = CalculatorScoreDirector::new(
        &p,
        p.initial_assignment(),
        || {
            Fault::<Assignment<i64>>::new(
                CalculatorUndoDiagnostics::Tracked,
                false,
                true,
                calls.clone(),
            )
        },
        false,
    )
    .unwrap();
    let mut command = |_,
                       c: &mut CustomPhaseContext<
        '_,
        Assignment<i64>,
        Move<i64>,
        SimpleScore,
        ScoringError,
    >| {
        c.evaluate(
            &Move::Change {
                variable: 0,
                value: Some(1),
            },
            |_, _| (),
        )?;
        Ok(())
    };
    let r = d.run_solver_with_extensions(
        SolverSequence {
            phases: vec![SolverPhase::Custom(CustomPhaseConfig::default())],
            ..Default::default()
        },
        SolverExtensions {
            command: Some(&mut command),
            ..Default::default()
        },
    );
    assert!(r.is_err());
    assert!(format!("{r:?}").contains("UndoCorruption"));
    assert_eq!(calls.get(), 2);
    d.reset(p.initial_assignment()).unwrap();
    calls.set(0);
    let mut command = |_,
                       c: &mut CustomPhaseContext<
        '_,
        Assignment<i64>,
        Move<i64>,
        SimpleScore,
        ScoringError,
    >| {
        c.execute(&Move::Change {
            variable: 0,
            value: Some(1),
        })?;
        Ok(())
    };
    d.run_solver_with_extensions(
        SolverSequence {
            phases: vec![SolverPhase::Custom(CustomPhaseConfig::default())],
            ..Default::default()
        },
        SolverExtensions {
            command: Some(&mut command),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(calls.get(), 1); // before capture; a committed trial has no undo comparison
}
#[derive(Clone)]
struct ReplayCounter<C> {
    policy: CalculatorUndoDiagnostics,
    direction: i64,
    score: i64,
    calls: Rc<Cell<u64>>,
    marker: PhantomData<C>,
}
impl<A: Position<Change = C>, C> IncrementalCalculator<A> for ReplayCounter<C> {
    type Score = SimpleScore;
    type Justification = ();
    type Change = C;
    fn undo_diagnostics(&self) -> CalculatorUndoDiagnostics {
        self.policy
    }
    fn reset(
        &mut self,
        a: &A,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.score = a.marker();
        Ok(())
    }
    fn after_change(
        &mut self,
        a: &A,
        _: &C,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.score = a.marker();
        Ok(())
    }
    fn calculate_score(&mut self) -> SimpleScore {
        self.calls.set(self.calls.get() + 1);
        SimpleScore::new(self.score * self.direction)
    }
}
#[test]
fn nonintrusive_replay_scores_respect_all_model_budgets_and_live_score_termination() {
    let bp = basic();
    let lp = lists();
    let mp = MixedProblem::new(bp.clone(), lp.clone());
    for limit in [1, 2, 3, 4] {
        let cfg = LocalSearchConfig {
            max_steps: 1,
            max_evaluations: limit,
            selection: MoveSelection::FirstAccepted,
            ..Default::default()
        };
        let expected = if limit >= 3 { 3 } else { 1 };
        let calls = Rc::new(Cell::new(0));
        let b = calculator_local_search_with_provider(
            &bp,
            bp.initial_assignment(),
            || ReplayCounter::<Vec<usize>> {
                score: 0,
                policy: CalculatorUndoDiagnostics::NonIntrusive,
                direction: 1,
                calls: calls.clone(),
                marker: PhantomData,
            },
            cfg.clone(),
            &mut |_: &FiniteProblem<i64>, _: &Assignment<i64>, _: u64| {
                vec![Move::Change {
                    variable: 0,
                    value: Some(1),
                }]
            },
        )
        .unwrap();
        assert_eq!(b.evaluations, expected);
        assert_eq!(calls.get(), expected);
        assert_eq!(b.completed_steps, u64::from(limit >= 3));
        calls.set(0);
        let l = list_calculator_local_search_with_provider(
            &lp,
            lp.initial_assignment(),
            || ReplayCounter::<Vec<usize>> {
                score: 0,
                policy: CalculatorUndoDiagnostics::NonIntrusive,
                direction: 1,
                calls: calls.clone(),
                marker: PhantomData,
            },
            cfg.clone(),
            &mut |_: &ListProblem<i64>, _: &ListAssignment<i64>, _: u64| vec![movement()],
        )
        .unwrap();
        assert_eq!(l.evaluations, expected);
        assert_eq!(calls.get(), expected);
        assert_eq!(l.completed_steps, u64::from(limit >= 3));
        calls.set(0);
        let m = mixed_calculator_local_search_with_provider(
            &mp,
            mp.initial_assignment(),
            || ReplayCounter::<MixedCalculatorChange> {
                score: 0,
                policy: CalculatorUndoDiagnostics::NonIntrusive,
                direction: 1,
                calls: calls.clone(),
                marker: PhantomData,
            },
            MixedLocalSearchConfig {
                search: cfg,
                ..Default::default()
            },
            &mut |_: &MixedProblem<i64, i64>, _: &MixedAssignment<i64, i64>, _: u64| {
                vec![MixedMove {
                    basic: Move::Composite(vec![]),
                    lists: movement(),
                }]
            },
        )
        .unwrap();
        assert_eq!(m.evaluations, expected);
        assert_eq!(calls.get(), expected);
        assert_eq!(m.completed_steps, u64::from(limit >= 3));
    }
    let calls = Rc::new(Cell::new(0));
    let mut d = CalculatorScoreDirector::new(
        &bp,
        bp.initial_assignment(),
        || ReplayCounter::<Vec<usize>> {
            score: 0,
            policy: CalculatorUndoDiagnostics::NonIntrusive,
            direction: 1,
            calls: calls.clone(),
            marker: PhantomData,
        },
        false,
    )
    .unwrap();
    let result = d
        .run_solver(SolverSequence {
            phases: vec![SolverPhase::LocalSearch(LocalSearchConfig {
                max_steps: u64::MAX,
                selection: MoveSelection::FirstAccepted,
                ..Default::default()
            })],
            limits: SolverLimits {
                termination: Some(&SearchTermination::Condition(
                    SearchLimit::ScoreEvaluations(3),
                )),
                ..Default::default()
            },
        })
        .unwrap();
    assert_eq!(result.work.evaluations, 3);
    assert_eq!(calls.get(), 3);
}
#[test]
fn completed_step_assertions_skip_empty_and_rejected_scans_but_check_real_steps() {
    let bp = basic();
    let lp = lists();
    let mp = MixedProblem::new(bp.clone(), lp.clone());
    for (empty, direction, expected) in [(true, 1, 3), (false, -1, 4), (false, 1, 6)] {
        let cfg = LocalSearchConfig {
            max_steps: 1,
            assert_step_scores: true,
            ..Default::default()
        };
        let calls = Rc::new(Cell::new(0));
        let expected_steps = u64::from(!empty && direction == 1);
        let b = calculator_local_search_with_provider(
            &bp,
            bp.initial_assignment(),
            || ReplayCounter::<Vec<usize>> {
                policy: CalculatorUndoDiagnostics::Default,
                direction,
                score: 0,
                calls: calls.clone(),
                marker: PhantomData,
            },
            cfg.clone(),
            &mut |_: &FiniteProblem<i64>, _: &Assignment<i64>, _: u64| {
                if empty {
                    vec![]
                } else {
                    vec![Move::Change {
                        variable: 0,
                        value: Some(1),
                    }]
                }
            },
        )
        .unwrap();
        assert_eq!(b.completed_steps, expected_steps);
        assert_eq!(calls.get(), expected);
        calls.set(0);
        let l = list_calculator_local_search_with_provider(
            &lp,
            lp.initial_assignment(),
            || ReplayCounter::<Vec<usize>> {
                policy: CalculatorUndoDiagnostics::Default,
                direction,
                score: 0,
                calls: calls.clone(),
                marker: PhantomData,
            },
            cfg.clone(),
            &mut |_: &ListProblem<i64>, _: &ListAssignment<i64>, _: u64| {
                if empty { vec![] } else { vec![movement()] }
            },
        )
        .unwrap();
        assert_eq!(l.completed_steps, expected_steps);
        assert_eq!(calls.get(), expected);
        calls.set(0);
        let m = mixed_calculator_local_search_with_provider(
            &mp,
            mp.initial_assignment(),
            || ReplayCounter::<MixedCalculatorChange> {
                policy: CalculatorUndoDiagnostics::Default,
                direction,
                score: 0,
                calls: calls.clone(),
                marker: PhantomData,
            },
            MixedLocalSearchConfig {
                search: cfg,
                ..Default::default()
            },
            &mut |_: &MixedProblem<i64, i64>, _: &MixedAssignment<i64, i64>, _: u64| {
                if empty {
                    vec![]
                } else {
                    vec![MixedMove {
                        basic: Move::Composite(vec![]),
                        lists: movement(),
                    }]
                }
            },
        )
        .unwrap();
        assert_eq!(m.completed_steps, expected_steps);
        assert_eq!(calls.get(), expected);
    }
}
