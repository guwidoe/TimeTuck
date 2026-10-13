use timetuck::*;
fn model(count: usize) -> NativeEmployeeSchedule {
    NativeEmployeeSchedule {
        employees: vec![ScheduleEmployee {
            id: "worker".into(),
            skills: ["skill".into()].into_iter().collect(),
        }],
        shifts: (0..count)
            .map(|i| EmployeeShift {
                id: format!("shift-{i}"),
                required_skill: "skill".into(),
                start: i as i64,
                end: i as i64 + 1,
                employee: None,
            })
            .collect(),
    }
}
#[test]
fn actual_consumption_scores_owned_current_models_and_events() {
    for n in [1, 3, 7] {
        let mut r = EmployeeScheduleResource::new();
        let mut input = model(n);
        let upload = r.post(&input, false).unwrap();
        assert_eq!(upload.assigned_shifts(), 0);
        assert_eq!(upload.score.medium(), -(n as i64));
        assert_eq!(r.events().len(), 1);
        input.shifts.clear();
        assert_eq!(r.get(&upload.id).unwrap().shift_count(), n);
        let solved = r.solve_existing(&upload.id).unwrap();
        assert_eq!(solved.status, EmployeeDatasetStatus::Completed);
        assert_eq!(solved.assigned_shifts(), n);
        assert_eq!(solved.score, solved.model.constraint_score().unwrap());
        assert_eq!(solved.score, HardMediumSoftScore::ZERO);
        assert!(
            r.events()
                .iter()
                .any(|e| e.kind == EmployeeResourceEventKind::Initialized
                    && e.model.assignment_penalty() == 0)
        );
        let mut copy = r.get(&upload.id).unwrap();
        copy.model.shifts.clear();
        assert_eq!(r.get(&upload.id).unwrap().shift_count(), n);
    }
}
#[test]
fn missing_skills_validation_and_query_failure_do_not_fake_completion() {
    let mut r = EmployeeScheduleResource::new();
    assert!(r.get("missing").is_err());
    assert!(r.solve_existing("missing").is_err());
    assert!(r.issue_type("unknown").is_none());
    let mut input = model(1);
    input.employees[0].skills.clear();
    let upload = r.post(&input, false).unwrap();
    assert!(r.solve_existing(&upload.id).is_err());
    assert_eq!(
        r.get(&upload.id).unwrap().status,
        EmployeeDatasetStatus::Failed
    );
    assert!(
        r.events()
            .iter()
            .all(|e| e.kind != EmployeeResourceEventKind::Final)
    );
    let mut r = EmployeeScheduleResource::new();
    let mut input = model(1);
    input.shifts[0].end = -2;
    let upload = r.post(&input, false).unwrap();
    assert!(r.solve_existing(&upload.id).is_err());
    assert_eq!(r.get(&upload.id).unwrap().assigned_shifts(), 0);
    assert!(
        r.events()
            .iter()
            .all(|e| e.kind != EmployeeResourceEventKind::Initialized)
    );
}
