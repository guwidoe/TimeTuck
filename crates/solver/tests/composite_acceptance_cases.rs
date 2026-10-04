// Adapted CompositeAcceptorTest.isAccepted; Apache-2.0, see provenance.
use timetuck::{Acceptance, Acceptor, SimpleScore};
#[test]
fn is_accepted() {
    for (children, expected) in [
        ([true, true, true], true),
        ([false, true, true], false),
        ([true, false, true], false),
        ([true, true, false], false),
        ([false, false, false], false),
    ] {
        let policies = children.map(|yes| {
            if yes {
                Acceptance::HillClimbing
            } else {
                Acceptance::StrictImprovement
            }
        });
        let mut composite = Acceptor::from_policies(&policies, SimpleScore::ZERO).unwrap();
        assert_eq!(
            composite.accepts(SimpleScore::ZERO, SimpleScore::ZERO, &[]),
            expected
        );
    }
}
