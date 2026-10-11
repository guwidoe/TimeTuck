//! Original stateful composition contracts; no upstream method credit.
use std::time::Duration;
use timetuck::*;
fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}
fn progress(n: u64, steps: u64) -> SearchProgress {
    SearchProgress {
        elapsed: ms(n),
        completed_steps: steps,
        ..Default::default()
    }
}
fn threshold(window: u64, difference: i64) -> StatefulSearchTermination<'static> {
    StatefulSearchTermination::Condition(StatefulSearchLimit::SignificantImprovement(
        SignificantImprovementConfig::new(ms(window), SimpleScore::new(difference)).unwrap(),
    ))
}
#[test]
fn stateless_query_requires_history_even_in_an_already_decided_branch() {
    let statics =
        ScoredSearchTermination::Condition(ScoredSearchLimit::Search(SearchLimit::Steps(0)));
    let plain = StatefulSearchTermination::Condition(StatefulSearchLimit::Static(&statics));
    assert_eq!(
        plain.try_status(progress(0, 0), None),
        Ok(statics.status(progress(0, 0), None))
    );
    let children = [plain, threshold(5, 10)];
    for tree in [
        StatefulSearchTermination::Any(&children),
        StatefulSearchTermination::All(&children),
    ] {
        assert_eq!(
            tree.try_status(progress(0, 0), None),
            Err(StatefulTerminationError::HistoryRequired)
        );
    }
}
#[test]
fn unarmed_history_does_not_ignore_an_and_leaf_and_static_best_cannot_arm_it() {
    let statics =
        ScoredSearchTermination::Condition(ScoredSearchLimit::Search(SearchLimit::Steps(0)));
    let children = [
        threshold(5, 10),
        StatefulSearchTermination::Condition(StatefulSearchLimit::Static(&statics)),
    ];
    let tree = StatefulSearchTermination::All(&children);
    let mut session = StatefulTerminationSession::new(&tree);
    let score = SimpleScore::new(1000);
    session.observe_best(ms(0), &score);
    assert!(
        !session
            .status(progress(500, 0), Some(&score))
            .unwrap()
            .reached
    );
    session.start_search(ms(500));
    assert!(
        !session
            .status(progress(1000, 0), Some(&score))
            .unwrap()
            .reached
    );
    session.observe_best(ms(1000), &score.with_structural(-1));
    assert!(
        !session
            .status(progress(1500, 0), Some(&score))
            .unwrap()
            .reached
    );
    session.observe_best(ms(1500), &score);
    assert!(
        !session
            .status(progress(1505, 0), Some(&score))
            .unwrap()
            .reached
    );
    assert!(
        session
            .status(progress(1506, 0), Some(&score))
            .unwrap()
            .reached
    );
    session.reset();
    assert!(
        !session
            .status(progress(1, 0), Some(&score))
            .unwrap()
            .reached
    );
}
#[test]
fn independent_occurrences_accumulate_thresholds_latch_expiry_and_clamp_time() {
    let children = [threshold(10, 10), threshold(30, 20)];
    let tree = StatefulSearchTermination::All(&children);
    let mut session = StatefulTerminationSession::new(&tree);
    session.start_search(ms(0));
    session.observe_best(ms(0), &SimpleScore::ZERO);
    session.observe_best(ms(5), &SimpleScore::new(6));
    session.observe_best(ms(10), &SimpleScore::new(10)); // first anchor resets exactly on deadline
    assert_eq!(
        session.status(progress(10, 0), None).unwrap().progress,
        Some(0.0)
    );
    session.observe_best(ms(20), &SimpleScore::new(19)); // below both accumulated thresholds
    assert!(!session.status(progress(21, 0), None).unwrap().reached); // first expired, second active
    session.observe_best(ms(22), &SimpleScore::new(20)); // only second can reset now
    assert_eq!(
        session.status(progress(22, 0), None).unwrap().progress,
        Some(0.0)
    );
    session.observe_best(ms(5), &SimpleScore::new(21)); // backward input does not restart the clock
    assert!(!session.status(progress(52, 0), None).unwrap().reached);
    assert!(session.status(progress(53, 0), None).unwrap().reached);
    session.observe_best(ms(54), &SimpleScore::new(1000));
    assert!(session.status(progress(54, 0), None).unwrap().reached);
}
#[test]
fn all_histories_poll_even_when_static_or_branch_is_already_true() {
    let yes = ScoredSearchTermination::Condition(ScoredSearchLimit::Search(SearchLimit::Steps(1)));
    let children = [
        StatefulSearchTermination::Condition(StatefulSearchLimit::Static(&yes)),
        threshold(5, 10),
    ];
    let tree = StatefulSearchTermination::Any(&children);
    let mut session = StatefulTerminationSession::new(&tree);
    session.start_search(ms(0));
    session.observe_best(ms(0), &SimpleScore::ZERO);
    assert!(session.status(progress(6, 1), None).unwrap().reached);
    session.observe_best(ms(7), &SimpleScore::new(100));
    // Static snapshots need not have monotone counters. History already expired
    // on the prior call even though the preceding static branch was true.
    assert!(session.status(progress(7, 0), None).unwrap().reached);
}
#[test]
fn exact_multilevel_and_decimal_thresholds_match_the_existing_primitive() {
    let config = SignificantImprovementConfig::new(ms(10), HardSoftScore::new(1, 0))
        .unwrap()
        .ignoring_trailing_levels(1)
        .unwrap();
    let tree =
        StatefulSearchTermination::Condition(StatefulSearchLimit::SignificantImprovement(config));
    let mut session = StatefulTerminationSession::new(&tree);
    let mut reference = SignificantImprovement::new(config);
    let initial = HardSoftScore::new(0, 100);
    reference.start(ms(0), initial);
    session.start_search(ms(0));
    session.observe_best(ms(0), &initial);
    for (time, score) in [
        (5, HardSoftScore::new(0, 200)),
        (10, HardSoftScore::new(1, -1000)),
        (20, HardSoftScore::new(1, -900)),
        (21, HardSoftScore::new(2, 0)),
    ] {
        let expected = reference.is_terminated(ms(time), &score);
        session.observe_best(ms(time), &score);
        assert_eq!(
            session
                .status(progress(time, 0), Some(&score))
                .unwrap()
                .reached,
            expected
        );
    }
    let config = SignificantImprovementConfig::new(
        ms(5),
        "0.000000000000000000000001"
            .parse::<SimpleDecimalScore>()
            .unwrap(),
    )
    .unwrap();
    let tree = StatefulSearchTermination::Condition(StatefulSearchLimit::SignificantImprovement(
        config.clone(),
    ));
    let mut session = StatefulTerminationSession::new(&tree);
    let mut reference = SignificantImprovement::new(config);
    let initial = "0".parse::<SimpleDecimalScore>().unwrap();
    session.start_search(ms(0));
    session.observe_best(ms(0), &initial);
    reference.start(ms(0), initial);
    for (time, text) in [
        (5, "0.000000000000000000000001"),
        (10, "0.0000000000000000000000015"),
        (11, "0.1"),
    ] {
        let score = text.parse::<SimpleDecimalScore>().unwrap();
        let expected = reference.is_terminated(ms(time), &score);
        session.observe_best(ms(time), &score);
        assert_eq!(
            session
                .status(progress(time, 0), Some(&score))
                .unwrap()
                .reached,
            expected
        );
    }
}
