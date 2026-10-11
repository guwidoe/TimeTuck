//! Original invocation-local score-goal contracts; no new upstream credit.
use std::time::Duration;
use timetuck::*;
fn state<S: Score>(
    session: &mut StatefulTerminationSession<'_, S>,
    best: Option<&S>,
) -> TerminationState {
    session.status(SearchProgress::default(), best).unwrap()
}
#[test]
fn goal_validation_and_explicit_initialized_capture_preserve_static_queries() {
    assert_eq!(
        ScoreGoalProgressConfig::target(SimpleScore::new(1).with_structural(-1), vec![]),
        Err(ScoreProgressError::FlawedTarget)
    );
    assert!(matches!(
        ScoreGoalProgressConfig::target(HardSoftScore::ZERO, vec![]),
        Err(ScoreProgressError::WeightCount {
            expected: 1,
            actual: 0
        })
    ));
    for weight in [f64::NAN, f64::INFINITY, -0.1, 1.1] {
        assert!(matches!(
            ScoreGoalProgressConfig::target(HardSoftScore::ZERO, vec![weight]),
            Err(ScoreProgressError::InvalidWeight { index: 0 })
        ));
    }
    assert!(matches!(
        ScoreGoalProgressConfig::<BendableScore<2, 1>>::feasibility(vec![]),
        Err(ScoreProgressError::WeightCount {
            expected: 1,
            actual: 0
        })
    ));
    let expression = StatefulSearchTermination::Condition(StatefulSearchLimit::ScoreGoal(
        ScoreGoalProgressConfig::target(SimpleScore::new(100), vec![]).unwrap(),
    ));
    assert_eq!(
        expression.try_status(SearchProgress::default(), Some(&SimpleScore::new(100))),
        Err(StatefulTerminationError::HistoryRequired)
    );
    let mut session = StatefulTerminationSession::new(&expression);
    let unknown = TerminationState {
        reached: false,
        progress: None,
    };
    assert_eq!(state(&mut session, Some(&SimpleScore::new(100))), unknown);
    session.start_search(Duration::ZERO);
    session.observe_best(Duration::ZERO, &SimpleScore::new(20)); // no implicit goal baseline
    assert_eq!(state(&mut session, Some(&SimpleScore::new(100))), unknown);
    session.observe_initialized_score(&SimpleScore::new(20).with_structural(-1));
    assert_eq!(state(&mut session, Some(&SimpleScore::new(100))), unknown);
    session.observe_initialized_score(&SimpleScore::new(20));
    session.observe_initialized_score(&SimpleScore::new(60)); // baseline is immutable
    assert_eq!(
        state(&mut session, Some(&SimpleScore::new(60))),
        TerminationState {
            reached: false,
            progress: Some(0.5)
        }
    );
    assert_eq!(
        state(&mut session, Some(&SimpleScore::new(100))),
        TerminationState {
            reached: true,
            progress: Some(1.0)
        }
    );
    assert_eq!(state(&mut session, None), unknown);
    assert_eq!(
        state(
            &mut session,
            Some(&SimpleScore::new(100).with_structural(-1))
        ),
        unknown
    );
    session.reset();
    session.observe_initialized_score(&SimpleScore::new(60));
    assert_eq!(
        state(&mut session, Some(&SimpleScore::new(80))).progress,
        Some(0.5)
    );
}
#[test]
fn weighted_target_and_feasibility_reuse_exact_multilevel_and_decimal_math() {
    let start = HardSoftScore::new(-20, -400);
    let target = HardSoftScore::new(-10, -300);
    let expression = StatefulSearchTermination::Condition(StatefulSearchLimit::ScoreGoal(
        ScoreGoalProgressConfig::target(target, vec![0.75]).unwrap(),
    ));
    let mut session = StatefulTerminationSession::new(&expression);
    session.observe_initialized_score(&start);
    let reference = ScoreTargetProgress::new(start, target, vec![0.75]).unwrap();
    for best in [
        start,
        HardSoftScore::new(-14, -340),
        HardSoftScore::new(-11, 1000),
        target,
        HardSoftScore::new(-9, -10000),
        HardSoftScore::new(-21, 1000),
    ] {
        let status = state(&mut session, Some(&best));
        assert_eq!(status.progress, Some(reference.progress(best)));
        assert_eq!(status.reached, reference.is_reached(best));
    }
    let start = BendableScore::new([-100, -200], [-300]);
    let expression = StatefulSearchTermination::Condition(StatefulSearchLimit::ScoreGoal(
        ScoreGoalProgressConfig::<BendableScore<2, 1>>::feasibility(vec![1.0]).unwrap(),
    ));
    let mut session = StatefulTerminationSession::new(&expression);
    session.observe_initialized_score(&start);
    let reference = FeasibilityProgress::new(start, vec![1.0]).unwrap();
    for best in [
        start,
        BendableScore::new([0, -1], [1000]),
        BendableScore::new([1, -1], [999999]),
        BendableScore::new([0, 0], [-999999]),
    ] {
        let status = state(&mut session, Some(&best));
        assert_eq!(status.progress, Some(reference.progress(best)));
        assert_eq!(status.reached, reference.is_reached(best));
    }
    let rounded = BendableScore::new([0, -1], [1000]);
    assert_eq!(
        state(&mut session, Some(&rounded)),
        TerminationState {
            reached: false,
            progress: Some(1.0)
        }
    );
    let parse = |s: &str| s.parse::<SimpleDecimalScore>().unwrap();
    let start = parse("0.000000000000000000000001");
    let target = parse("0.000000000000000000000009");
    let expression = StatefulSearchTermination::Condition(StatefulSearchLimit::ScoreGoal(
        ScoreGoalProgressConfig::target(target.clone(), vec![]).unwrap(),
    ));
    let mut session = StatefulTerminationSession::new(&expression);
    session.observe_initialized_score(&start);
    let reference = ScoreTargetProgress::new(start, target, vec![]).unwrap();
    for text in [
        "0.000000000000000000000001",
        "0.000000000000000000000005",
        "0.00000000000000000000000899999999999999999999",
        "0.000000000000000000000009",
    ] {
        let best = parse(text);
        let status = state(&mut session, Some(&best));
        assert_eq!(status.progress, Some(reference.progress(best.clone())));
        assert_eq!(status.reached, reference.is_reached(best));
    }
}
#[test]
fn independent_sessions_and_goal_occurrences_do_not_arm_stagnation() {
    let children = [
        StatefulSearchTermination::Condition(StatefulSearchLimit::ScoreGoal(
            ScoreGoalProgressConfig::target(SimpleScore::new(100), vec![]).unwrap(),
        )),
        StatefulSearchTermination::Condition(StatefulSearchLimit::ScoreGoal(
            ScoreGoalProgressConfig::target(SimpleScore::new(200), vec![]).unwrap(),
        )),
        StatefulSearchTermination::Condition(StatefulSearchLimit::SignificantImprovement(
            SignificantImprovementConfig::new(Duration::from_secs(1), SimpleScore::new(10))
                .unwrap(),
        )),
    ];
    let expression = StatefulSearchTermination::All(&children);
    let mut a = StatefulTerminationSession::new(&expression);
    let mut b = StatefulTerminationSession::new(&expression);
    a.observe_initialized_score(&SimpleScore::ZERO);
    b.observe_initialized_score(&SimpleScore::new(100));
    let query = SearchProgress {
        elapsed: Duration::from_secs(100),
        ..Default::default()
    };
    assert_eq!(
        a.status(query, Some(&SimpleScore::new(150))).unwrap(),
        TerminationState {
            reached: false,
            progress: Some(0.75)
        }
    );
    assert_eq!(
        b.status(query, Some(&SimpleScore::new(150))).unwrap(),
        TerminationState {
            reached: false,
            progress: Some(0.5)
        }
    );
    a.start_search(query.elapsed);
    a.observe_best(query.elapsed, &SimpleScore::ZERO);
    assert!(
        !a.status(
            SearchProgress {
                elapsed: Duration::from_secs(102),
                ..query
            },
            Some(&SimpleScore::new(150))
        )
        .unwrap()
        .reached
    );
    assert!(
        a.status(
            SearchProgress {
                elapsed: Duration::from_secs(102),
                ..query
            },
            Some(&SimpleScore::new(200))
        )
        .unwrap()
        .reached
    );
    assert!(
        !b.status(
            SearchProgress {
                elapsed: Duration::from_secs(1000),
                ..query
            },
            Some(&SimpleScore::new(200))
        )
        .unwrap()
        .reached
    );
}

#[test]
fn goal_polling_borrows_owned_scores_without_clone_or_rescoring() {
    use std::{
        cmp::Ordering,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering as AtomicOrdering},
        },
    };
    #[derive(Debug)]
    struct Counted {
        value: i64,
        clones: Arc<AtomicUsize>,
    }
    impl Clone for Counted {
        fn clone(&self) -> Self {
            self.clones.fetch_add(1, AtomicOrdering::SeqCst);
            Self {
                value: self.value,
                clones: self.clones.clone(),
            }
        }
    }
    impl PartialEq for Counted {
        fn eq(&self, other: &Self) -> bool {
            self.value == other.value
        }
    }
    impl Eq for Counted {}
    impl PartialOrd for Counted {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            Some(self.cmp(other))
        }
    }
    impl Ord for Counted {
        fn cmp(&self, other: &Self) -> Ordering {
            self.value.cmp(&other.value)
        }
    }
    impl Score for Counted {
        type Level = i64;
        const HARD_LEVEL_COUNT: usize = 0;
        fn ordinary_levels(&self) -> impl Iterator<Item = i64> {
            std::iter::once(self.value)
        }
        fn structural(&self) -> i64 {
            0
        }
        fn hard_cmp(&self, _: &Self) -> Ordering {
            Ordering::Equal
        }
        fn is_feasible(&self) -> bool {
            true
        }
    }
    let clones = Arc::new(AtomicUsize::new(0));
    let score = |value| Counted {
        value,
        clones: clones.clone(),
    };
    let expression = StatefulSearchTermination::Condition(StatefulSearchLimit::ScoreGoal(
        ScoreGoalProgressConfig::target(score(100), vec![]).unwrap(),
    ));
    let mut session = StatefulTerminationSession::new(&expression);
    assert_eq!(clones.load(AtomicOrdering::SeqCst), 0);
    session.observe_initialized_score(&score(0));
    assert_eq!(
        clones.load(AtomicOrdering::SeqCst),
        2,
        "one baseline and one target clone per initialized occurrence"
    );
    for _ in 0..1000 {
        session.observe_initialized_score(&score(25));
        assert_eq!(state(&mut session, Some(&score(50))).progress, Some(0.5));
    }
    assert_eq!(clones.load(AtomicOrdering::SeqCst), 2);
}
