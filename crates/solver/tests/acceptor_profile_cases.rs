// Adapted pinned Community AcceptorFactoryTest and negative-size methods.
// SPDX-License-Identifier: Apache-2.0
use timetuck::*;
#[test]
fn build_composite_acceptor() {
    let tabu = TabuSettings {
        hard: 1,
        fading: Some(1),
        aspiration: true,
    };
    let p = AcceptorProfile {
        kinds: AcceptorKind::ALL.to_vec(),
        variable_tabu: Some(tabu),
        value_tabu: Some(tabu),
        move_tabu: Some(tabu),
        late_size: Some(10),
        temperature: Some(AnnealingTemperature::Loss(
            "-10hard/-10soft".parse::<HardSoftScore>().unwrap(),
        )),
        step_period: Some(1),
        step_mode: StepCountingMode::ImprovingStep,
        ..Default::default()
    }
    .prepare(true)
    .unwrap();
    assert!(p.is_composite());
    let policies = p.policies();
    assert_eq!(policies.len(), 9);
    assert!(matches!(policies[0], Acceptance::HillClimbing));
    assert!(matches!(
        policies[1],
        Acceptance::StepCountingHillClimbing {
            period: 1,
            mode: StepCountingMode::ImprovingStep
        }
    ));
    assert!(matches!(
        policies[2],
        Acceptance::FadingVariableTabu {
            hard_tenure: 1,
            fading_tenure: 1,
            ..
        }
    ));
    assert!(matches!(
        policies[3],
        Acceptance::FadingValueTabu {
            hard_tenure: 1,
            fading_tenure: 1,
            ..
        }
    ));
    assert!(matches!(
        policies[4],
        Acceptance::FadingMoveTabu {
            hard_tenure: 1,
            fading_tenure: 1,
            ..
        }
    ));
    assert!(
        matches!(policies[5], Acceptance::SimulatedAnnealing { starting_temperature, .. } if starting_temperature == HardSoftScore::new(10, 10))
    );
    assert!(matches!(
        policies[6],
        Acceptance::LateAcceptance {
            history_size: 10,
            ..
        }
    ));
    assert!(matches!(
        policies[7],
        Acceptance::DiversifiedLateAcceptance { history_size: 10 }
    ));
    assert!(matches!(policies[8], Acceptance::GreatDeluge { .. }));
}
#[test]
fn no_acceptor_configured() {
    let error = AcceptorProfile::<SimpleScore>::default()
        .prepare(true)
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("The acceptor does not specify any acceptorType")
    );
}
#[test]
fn late_acceptance() {
    for p in [
        AcceptorProfile {
            kinds: vec![AcceptorKind::LateAcceptance],
            ..Default::default()
        },
        AcceptorProfile {
            late_size: Some(10),
            ..Default::default()
        },
    ] {
        let prepared: PreparedAcceptors<SimpleScore> = p.prepare(true).unwrap();
        assert!(!prepared.is_composite());
        assert!(matches!(
            prepared.policies(),
            [Acceptance::LateAcceptance { .. }]
        ));
    }
}
#[test]
fn diversified_late_acceptance() {
    for late_size in [None, Some(10)] {
        let p = AcceptorProfile::<SimpleScore> {
            kinds: vec![AcceptorKind::DiversifiedLateAcceptance],
            late_size,
            ..Default::default()
        };
        let prepared = p.prepare(true).unwrap();
        assert!(!prepared.is_composite());
        assert!(matches!(
            prepared.policies(),
            [Acceptance::DiversifiedLateAcceptance { .. }]
        ));
    }
    let p = AcceptorProfile::<SimpleScore> {
        kinds: vec![AcceptorKind::DiversifiedLateAcceptance],
        late_size: Some(10),
        ..Default::default()
    };
    assert_eq!(
        p.prepare(false),
        Err(AcceptorProfileError::DiversifiedDisabled)
    );
}
#[test]
fn value_tabu_without_sizes() {
    assert!(
        AcceptorProfile::<SimpleScore> {
            kinds: vec![AcceptorKind::ValueTabu],
            ..Default::default()
        }
        .prepare(true)
        .is_err()
    );
}
#[test]
fn move_tabu_without_sizes() {
    assert!(
        AcceptorProfile::<SimpleScore> {
            kinds: vec![AcceptorKind::MoveTabu],
            ..Default::default()
        }
        .prepare(true)
        .is_err()
    );
}
#[test]
fn negative_late_acceptance_size() {
    assert!(
        AcceptorProfile::<SimpleScore> {
            late_size: Some(-1),
            ..Default::default()
        }
        .prepare(true)
        .is_err()
    );
}
#[test]
fn negative_step_counting_size() {
    assert!(
        AcceptorProfile::<SimpleScore> {
            step_period: Some(-1),
            step_mode: StepCountingMode::Step,
            ..Default::default()
        }
        .prepare(true)
        .is_err()
    );
}
