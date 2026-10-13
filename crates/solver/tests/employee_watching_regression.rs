use std::collections::BTreeMap;
use timetuck::*;
include!("fixtures/employee_resource_fixture.rs");
#[test]
fn independent_subscriptions_cancel_drop_and_bound_queues() {
    let mut resource = EmployeeScheduleResource::new();
    let a = resource.post(&input(), false).unwrap();
    let b = resource.post(&input(), false).unwrap();
    let mut cancel = resource.subscribe(&a.id, 16).unwrap();
    let mut active = resource.subscribe(&a.id, 16).unwrap();
    let mut other = resource.subscribe(&b.id, 16).unwrap();
    let mut small = resource.subscribe(&a.id, 1).unwrap();
    assert_eq!(resource.active_subscription_count(), 4);
    cancel.cancel();
    assert_eq!(resource.active_subscription_count(), 3);
    {
        let _temporary = resource.subscribe(&a.id, 16).unwrap();
        assert_eq!(resource.active_subscription_count(), 4);
    }
    assert_eq!(resource.active_subscription_count(), 3);
    resource.solve_existing(&a.id).unwrap();
    assert!(cancel.next_event().unwrap().is_none());
    assert!(cancel.is_closed());
    assert_eq!(small.next_event(), Err(EmployeeWatchError::Overflow));
    assert_eq!(small.pending_len(), 1);
    assert!(small.is_closed());
    let mut seen = 0;
    while let Some(event) = active.next_event().unwrap() {
        assert_eq!(event.id, a.id);
        seen += 1;
    }
    assert!(seen >= 4);
    assert!(active.is_closed());
    assert_eq!(resource.active_subscription_count(), 1);
    assert_eq!(other.next_event().unwrap().unwrap().id, b.id);
    assert!(other.next_event().unwrap().is_none());
    assert!(!other.is_closed());
    other.cancel();
    assert_eq!(resource.active_subscription_count(), 0);
    assert_eq!(
        resource.subscribe(&b.id, 0).err(),
        Some(EmployeeWatchError::InvalidCapacity)
    );
    assert_eq!(
        resource.subscribe(&b.id, 4097).err(),
        Some(EmployeeWatchError::InvalidCapacity)
    );
}
#[test]
fn failed_actual_solve_closes_subscription_and_keeps_real_error() {
    let mut schedule = input();
    schedule.employees.clear();
    let mut resource = EmployeeScheduleResource::new();
    let metadata = resource.post(&schedule, false).unwrap();
    let mut subscription = resource.subscribe(&metadata.id, 16).unwrap();
    assert!(resource.solve_existing(&metadata.id).is_err());
    let response = resource.get(&metadata.id).unwrap();
    assert_eq!(response.status, EmployeeDatasetStatus::Failed);
    assert!(response.error.is_some());
    let mut events = vec![];
    while let Some(event) = subscription.next_event().unwrap() {
        events.push(event);
    }
    let last = events.last().unwrap();
    assert_eq!(last.status, EmployeeDatasetStatus::Failed);
    assert_eq!(last.error, response.error);
    assert!(subscription.is_closed());
    assert_eq!(
        resource.subscribe(&metadata.id, 16).err(),
        Some(EmployeeWatchError::Gone)
    );
}
#[test]
fn shared_routes_consume_priorities_and_execute_all_four_operations() {
    let mut resource = EmployeeScheduleResource::new();
    let existing = resource.post(&input(), false).unwrap();
    let patch_target = resource.post(&input(), false).unwrap();
    let mut dispatcher = EmployeeRouteDispatcher::new("actual-v3", 4).unwrap();
    let commands = [
        (
            1,
            EmployeeRouteCommand::Schedule(input()),
            "/actual-v3/schedules".to_owned(),
        ),
        (
            4,
            EmployeeRouteCommand::SolveDataset(existing.id.clone()),
            format!("/actual-v3/schedules/{}", existing.id),
        ),
        (
            2,
            EmployeeRouteCommand::FromInput(input()),
            "/actual-v3/schedules/from-input".to_owned(),
        ),
        (
            3,
            EmployeeRouteCommand::FromPatch {
                id: patch_target.id.clone(),
                assignments: BTreeMap::from([("Ambulance shift 1".into(), Some("Beth".into()))]),
            },
            format!("/actual-v3/schedules/{}/from-patch", patch_target.id),
        ),
    ];
    for (priority, command, path) in commands {
        let receipt = dispatcher
            .enqueue(
                "POST",
                &path,
                &BTreeMap::from([("priority".into(), priority.to_string())]),
                command,
            )
            .unwrap();
        assert_eq!(receipt.priority, priority);
        assert_eq!(receipt.status_code, 202);
    }
    assert_eq!(dispatcher.pending_len(), 4);
    let mut priorities = vec![];
    while let Some((receipt, response)) = dispatcher.execute_next(&mut resource) {
        priorities.push(receipt.priority);
        let response = response.unwrap();
        assigned(&response);
        assert!(response.model.shifts.iter().all(|s| s.employee.is_some()));
        if receipt.priority == 4 {
            assert_eq!(response.id, existing.id);
        }
        if receipt.priority == 3 {
            assert_eq!(response.model.shifts[0].employee.as_deref(), Some("Beth"));
        }
    }
    assert_eq!(priorities, vec![4, 3, 2, 1]);
    for route in dispatcher.routes() {
        let document = dispatcher.openapi();
        assert_eq!(
            document["paths"][&route.path]["post"]["parameters"][0]["name"],
            route.priority_parameter
        );
    }
    let bad = dispatcher.enqueue(
        "POST",
        "/actual-v3/schedules",
        &BTreeMap::from([("priority".into(), "not-an-integer".into())]),
        EmployeeRouteCommand::Schedule(input()),
    );
    assert!(bad.is_err());
    assert_eq!(dispatcher.pending_len(), 0);
}

#[test]
fn subscriber_limit_and_resource_drop_close_owned_streams() {
    let mut resource = EmployeeScheduleResource::new();
    let metadata = resource.post(&input(), false).unwrap();
    let mut subscriptions = (0..256)
        .map(|_| resource.subscribe(&metadata.id, 1).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(resource.active_subscription_count(), 256);
    assert_eq!(
        resource.subscribe(&metadata.id, 1).err(),
        Some(EmployeeWatchError::SubscriberLimit)
    );
    subscriptions.pop();
    assert_eq!(resource.active_subscription_count(), 255);
    drop(resource);
    assert!(subscriptions.iter().all(EmployeeSubscription::is_closed));
    assert!(subscriptions.iter().all(|s| s.pending_len() == 1));
}
