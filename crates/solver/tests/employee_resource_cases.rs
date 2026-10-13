// Adapted pinned Community employee provider/resource TESTS; Apache-2.0.
use timetuck::*;
include!("fixtures/employee_resource_fixture.rs");
#[test]
fn assign_shift_to_employee_assigned_shift() {
    let employee = employee("Ann", &["AMBULANCE"]);
    let mut shift = shift("Ambulance shift 1", "AMBULANCE");
    shift.employee = Some(employee.id.clone());
    let s = NativeEmployeeSchedule {
        employees: vec![employee],
        shifts: vec![shift],
    };
    assert_eq!(-s.constraint_score().unwrap().medium(), 0);
}
#[test]
fn assign_shift_to_employee_unassigned_shift() {
    let s = NativeEmployeeSchedule {
        employees: vec![employee("Ann", &["AMBULANCE"])],
        shifts: vec![shift("Ambulance shift 1", "AMBULANCE")],
    };
    assert_eq!(-s.constraint_score().unwrap().medium(), 1);
}
#[test]
fn constraint_weights_available_after_dataset_computed() {
    let mut resource = EmployeeScheduleResource::new();
    resource.post(&input(), true).unwrap();
    let event = resource
        .events()
        .iter()
        .find(|e| e.kind == EmployeeResourceEventKind::Computed)
        .unwrap();
    assert_eq!(event.score.medium(), -4);
    assert!(!event.weights.is_empty());
}
#[test]
fn dataset_computed_event_carries_solve_requested_flag() {
    for solve in [true, false] {
        let mut r = EmployeeScheduleResource::new();
        r.post(&input(), solve).unwrap();
        let event = r
            .events()
            .iter()
            .find(|e| e.kind == EmployeeResourceEventKind::Computed)
            .unwrap();
        assert_eq!(event.solve_requested, solve);
    }
}
#[test]
fn solve_employee_schedule() {
    let mut resource = EmployeeScheduleResource::new();
    let metadata = resource.post(&input(), true).unwrap();
    let result = resource.get(&metadata.id).unwrap();
    assigned(&result);
    let events = resource.events();
    let init = events
        .iter()
        .filter(|e| e.kind == EmployeeResourceEventKind::Initialized)
        .collect::<Vec<_>>();
    assert_eq!(init.len(), 1);
    assert_eq!(
        init[0].producer.as_deref(),
        Some("Construction Heuristic (0)")
    );
    let best = events
        .iter()
        .filter(|e| e.kind == EmployeeResourceEventKind::Best)
        .collect::<Vec<_>>();
    assert_eq!(best.len(), 2);
    assert_eq!(best[0].producer.as_deref(), Some("Solving started"));
    assert_eq!(
        best[1].producer.as_deref(),
        Some("Construction Heuristic (0)")
    );
    let final_events = events
        .iter()
        .filter(|e| e.kind == EmployeeResourceEventKind::Final)
        .collect::<Vec<_>>();
    assert_eq!(final_events.len(), 1);
    assert_eq!(final_events[0].producer, None);
}
#[test]
fn upload_and_solve_employee_schedule() {
    let mut resource = EmployeeScheduleResource::new();
    let metadata = resource.post(&input(), false).unwrap();
    assert_eq!(metadata.status, EmployeeDatasetStatus::Computed);
    let solve_metadata = resource.solve_existing(&metadata.id).unwrap();
    assigned(&resource.get(&solve_metadata.id).unwrap());
}
#[test]
fn upload_only_schedule() {
    let mut resource = EmployeeScheduleResource::new();
    let metadata = resource.post(&input(), false).unwrap();
    assert!(
        resource
            .events()
            .iter()
            .any(|e| e.kind == EmployeeResourceEventKind::Computed)
    );
    let result = resource.get(&metadata.id).unwrap();
    assert_eq!(result.employee_count(), 3);
    assert_eq!(result.shift_count(), 4);
    assert_eq!(result.assigned_shifts(), 0);
}
#[test]
fn get_validation_result() {
    let mut input = input();
    let mut wrong = shift("Wrong shift", "AMBULANCE");
    wrong.start = 16 * 3600;
    wrong.end = 8 * 3600;
    input.shifts.push(wrong.clone());
    let mut r = EmployeeScheduleResource::new();
    let meta = r.post(&input, false).unwrap();
    let result = r.get(&meta.id).unwrap().validation;
    assert_eq!(result.status(), ServiceValidationStatus::Errors);
    assert_eq!(result.issues.len(), 1);
    let issue: &EmployeeShiftEndBeforeStartIssue = &result.issues[0];
    assert_eq!(issue.code(), "SHIFT_END_BEFORE_START");
    let json = result.to_json();
    assert_eq!(json["issues"][0]["shiftId"], wrong.id);
    assert_eq!(json["issues"][0]["startTime"], wrong.start);
    assert_eq!(json["issues"][0]["endTime"], wrong.end);
}
fn check_type(issue: ServiceValidationIssueType) {
    assert_eq!(issue.code, SHIFT_END_BEFORE_START);
    assert_eq!(issue.severity, ValidationSeverity::Error);
    let data = issue.to_pretty_json();
    let json: serde_json::Value = serde_json::from_str(&data).unwrap();
    assert_eq!(json["metadata"].as_array().unwrap().len(), 1);
    assert_eq!(json["metadata"][0]["message"], NATIVE_SHIFT_END_MESSAGE);
}
#[test]
fn get_issue_types() {
    let r = EmployeeScheduleResource::new();
    let types = r.issue_types();
    assert_eq!(types.len(), 1);
    check_type(types.into_iter().next().unwrap());
}
#[test]
fn get_issue_type_by_code() {
    check_type(
        EmployeeScheduleResource::new()
            .issue_type(SHIFT_END_BEFORE_START)
            .unwrap(),
    );
}
