use timetuck::*;

#[test]
fn worker_flag_is_shared_only_with_its_thread_and_explicit_connections() {
    let flag = CooperativeThreadInterrupt::current();
    flag.clear();
    let linked = flag.cancellation_token();
    let ordinary = CancellationToken::new();
    let controller = flag.clone();
    std::thread::spawn(move || {
        let own = CooperativeThreadInterrupt::current();
        assert!(!own.is_interrupted());
        assert!(controller.interrupt());
        assert!(!controller.interrupt());
        assert!(!own.is_interrupted());
    })
    .join()
    .unwrap();
    assert!(CooperativeThreadInterrupt::current().is_interrupted());
    assert!(linked.is_cancelled());
    assert!(!ordinary.is_cancelled());
    linked.cancel();
    assert!(flag.clear());
    assert!(!flag.clear());
    assert!(linked.is_cancelled());
    assert!(!flag.cancellation_token().is_cancelled());
}

#[test]
fn signed_window_boundaries_and_validation_precedence() {
    assert_eq!(
        SignificantImprovementConfig::from_millis(-1, SimpleScore::new(-7)).unwrap_err(),
        SignificantImprovementError::NegativeTime
    );
    for millis in [0, 1, i64::MAX] {
        let config = SignificantImprovementConfig::from_millis(millis, SimpleScore::ZERO).unwrap();
        assert_eq!(config.window().as_millis(), millis as u128);
    }
}
