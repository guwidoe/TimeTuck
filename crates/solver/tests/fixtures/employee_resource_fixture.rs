// Adapted source createInputEmployeeSchedule; Apache-2.0. Native fixed UTC-day
// seconds replace tomorrow's date; tests depend on ordering/equal captured fields.
fn employee(id: &str, skills: &[&str]) -> ScheduleEmployee {
    ScheduleEmployee {
        id: id.into(),
        skills: skills.iter().map(|s| s.to_string()).collect(),
    }
}
fn shift(id: &str, skill: &str) -> EmployeeShift {
    EmployeeShift {
        id: id.into(),
        required_skill: skill.into(),
        start: 8 * 3600,
        end: 16 * 3600,
        employee: None,
    }
}
fn input() -> NativeEmployeeSchedule {
    NativeEmployeeSchedule {
        employees: vec![
            employee("Ann", &["EMERGENCY", "AMBULANCE"]),
            employee("Beth", &["AMBULANCE", "DERMATOLOGY"]),
            employee("Carl", &["CARDIOLOGY", "ANESTHESIA"]),
        ],
        shifts: vec![
            shift("Ambulance shift 1", "AMBULANCE"),
            shift("Dermatology shift 1", "DERMATOLOGY"),
            shift("Cardiology shift 1", "CARDIOLOGY"),
            shift("Anesthesia shift 1", "ANESTHESIA"),
        ],
    }
}
fn assigned(response: &EmployeeScheduleResponse) {
    assert!(response.score.is_feasible());
    assert_eq!(response.score.medium(), 0);
    assert!(response.model.shifts.iter().all(|s| s.employee.is_some()));
}
