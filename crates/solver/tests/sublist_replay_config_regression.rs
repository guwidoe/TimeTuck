//! Original controls for bound validation and actual published move geometry.
use timetuck::*;

#[test]
fn configured_bounds_record_and_replay_an_executable_span() {
    let problem = ListProblem::new(
        vec![1, 2, 3, 4],
        vec![ListEntity::new(vec![1, 2, 3]), ListEntity::new(vec![4])],
    )
    .unwrap();
    let assignment = problem.initial_assignment();
    let context = ListSelectionContext::new(&problem, &assignment).unwrap();
    let source = SubListSelectionConfig {
        minimum: Some(2),
        maximum: Some(2),
        ..Default::default()
    }
    .build::<i32>(ConfiguredSelectionPolicy::default(), 0)
    .unwrap();
    let mut registry = SelectionRegistry::new();
    let mut recording = registry.register("spans", source).unwrap();
    for event in [
        SelectionLifecycleEvent::SolveStarted,
        SelectionLifecycleEvent::PhaseStarted,
        SelectionLifecycleEvent::StepStarted,
    ] {
        recording.event(event, &context);
    }
    let replay = SubListSelectionConfig {
        replay_ref: Some("spans".into()),
        ..Default::default()
    }
    .build_replaying(&registry)
    .unwrap();
    let mut cursor = recording.open(&context);
    let mut replay_cursor = replay.cursor();
    let span = cursor.next().unwrap();
    assert_eq!(span.end - span.start, 2);
    assert_eq!(*replay_cursor.next_replayed().unwrap().unwrap(), *span);
    let movement = ListMove::SubListChange {
        entity: span.entity,
        start: span.start,
        end: span.end,
        destination: ListPosition {
            entity: 1,
            index: 1,
        },
        reverse: false,
    };
    let mut changed = assignment.clone();
    let applied = problem.apply(&mut changed, &movement).unwrap();
    assert_eq!(applied.lists()[1].len(), 3);
    drop(applied);
    assert_eq!(changed, assignment);
}

#[test]
fn explicit_defaults_and_invalid_bounds_do_not_bypass_replay_validation() {
    for (minimum, maximum) in [
        (Some(2), None),
        (None, Some(usize::MAX)),
        (Some(0), Some(1)),
    ] {
        let config = SubListSelectionConfig {
            minimum,
            maximum,
            replay_ref: Some("missing".into()),
            ..Default::default()
        };
        assert!(matches!(
            config.build_replaying(&SelectionRegistry::new()),
            Err(SubListSelectionBuildError::ReplayOverrides)
        ));
    }
    let config = SubListSelectionConfig {
        minimum: Some(0),
        ..Default::default()
    };
    assert!(matches!(
        config.build::<i32>(ConfiguredSelectionPolicy::default(), 0),
        Err(SubListSelectionBuildError::Bounds(_))
    ));
    let config = SubListSelectionConfig {
        replay_ref: Some("missing".into()),
        ..Default::default()
    };
    assert!(matches!(
        config
            .clone()
            .build::<i32>(ConfiguredSelectionPolicy::default(), 0),
        Err(SubListSelectionBuildError::ReplayRequiresRegistry)
    ));
    assert!(matches!(
        config.build_replaying(&SelectionRegistry::new()),
        Err(SubListSelectionBuildError::Registration(
            SelectionRegistrationError::UnknownId(_)
        ))
    ));
}
