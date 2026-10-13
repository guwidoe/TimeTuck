// Apache-2.0; activated full pinned EmployeeScheduleResourceTest watching method.
use std::collections::BTreeSet;
use timetuck::*;
include!("fixtures/employee_resource_fixture.rs");
#[test]
fn solve_employee_schedule_with_watching() {
    let mut resource = EmployeeScheduleResource::new();
    assert_eq!(
        resource
            .subscribe("unknown-dataset", 16)
            .err()
            .unwrap()
            .status_code(),
        404
    );
    let metadata = resource.post(&input(), false).unwrap();
    let mut subscription = resource.subscribe(&metadata.id, 16).unwrap();
    // The actual subscription is installed before the synchronous solve consumes commands.
    assert_eq!(resource.active_subscription_count(), 1);
    let solve_metadata = resource.solve_existing(&metadata.id).unwrap();
    assigned(&solve_metadata);
    assert_eq!(solve_metadata.score.medium(), 0);
    let response = resource.get(&metadata.id).unwrap();
    assert!(
        response
            .model
            .shifts
            .iter()
            .all(|shift| shift.employee.is_some())
    );
    let mut received = Vec::new();
    let mut event_stream_error = None;
    loop {
        match subscription.next_event() {
            Ok(Some(event)) => received.push(event),
            Ok(None) => break,
            Err(error) => {
                event_stream_error = Some(error);
                break;
            }
        }
    }
    let statuses = received
        .iter()
        .map(|event| format!("{:?}", event.status))
        .collect::<BTreeSet<_>>();
    for status in [
        EmployeeDatasetStatus::Computed,
        EmployeeDatasetStatus::SolvingStarted,
        EmployeeDatasetStatus::SolvingActive,
        EmployeeDatasetStatus::Completed,
    ] {
        assert!(statuses.contains(&format!("{status:?}")));
    }
    assert!(event_stream_error.is_none());
    subscription.cancel();
    assert_eq!(
        resource
            .subscribe(&metadata.id, 16)
            .err()
            .unwrap()
            .status_code(),
        410
    );
}
