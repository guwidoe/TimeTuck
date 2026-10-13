// Adapted pinned Community TerminationFactoryTest, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::time::Duration;
use timetuck::*;

#[test]
fn build_inner_termination() {
    let outer = InnerTerminationConfig {
        children: vec![InnerTerminationCondition::SpentTime(TimeLimitSpec::Units(
            TimeLimitUnits {
                seconds: Some(1),
                ..Default::default()
            },
        ))],
        ..Default::default()
    };
    let inner = outer.build().unwrap();
    assert_eq!(inner.children().len(), 1);
    let SearchTermination::Condition(SearchLimit::Time(limit)) = inner.children()[0] else {
        panic!("inner child must be an elapsed-time condition")
    };
    assert_eq!(limit, Duration::from_millis(1000));
}
