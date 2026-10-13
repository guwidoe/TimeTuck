use std::collections::{BTreeMap, BTreeSet};
use timetuck::*;
fn schedule(count: usize) -> NativeEmployeeSchedule {
    NativeEmployeeSchedule {
        employees: vec![ScheduleEmployee {
            id: format!("employee-{count}"),
            skills: BTreeSet::from(["native-skill".into()]),
        }],
        shifts: (0..count)
            .map(|i| EmployeeShift {
                id: format!("shift-{i}"),
                required_skill: "native-skill".into(),
                start: 0,
                end: 10,
                employee: None,
            })
            .collect(),
    }
}
#[test]
fn descriptors_build_real_dispatchers_and_execute_current_models() {
    for count in [1, 3, 7] {
        let version = format!("v{}", count + 1);
        let model_id = format!("native-service_{version}");
        let path = format!("/{version}/schedules");
        let descriptor =
            ServiceModelDescriptor::new(Some(&model_id), Some(&version), &path).unwrap();
        assert_eq!(descriptor.model_id().model(), "native-service");
        assert_eq!(descriptor.model_id().version(), Some(version.as_str()));
        assert_eq!(descriptor.resource_type().as_str(), "schedules");
        let mut dispatcher = descriptor.employee_dispatcher(2).unwrap();
        let route = format!("/{version}/schedules");
        let query = BTreeMap::from([("priority".into(), count.to_string())]);
        let receipt = dispatcher
            .enqueue(
                "POST",
                &route,
                &query,
                EmployeeRouteCommand::Schedule(schedule(count)),
            )
            .unwrap();
        assert_eq!(receipt.status_code, 202);
        assert_eq!(receipt.priority, count as i32);
        let mut resource = EmployeeScheduleResource::new();
        let (actual, result) = dispatcher.execute_next(&mut resource).unwrap();
        assert_eq!(actual, receipt);
        let response = result.unwrap();
        assert_eq!(response.shift_count(), count);
        assert_eq!(response.assigned_shifts(), count);
        assert!(response.score.is_feasible());
        assert_eq!(response.score.medium(), 0);
        assert!(dispatcher.execute_next(&mut resource).is_none());
        let document = dispatcher.openapi();
        assert!(
            document["paths"]
                .as_object()
                .unwrap()
                .keys()
                .all(|p| p.starts_with(&format!("/{version}/")))
        );
        assert_eq!(document["paths"][&route]["post"]["operationId"], "schedule");
    }
}
#[test]
fn ownership_unversioned_path_and_factory_failures_are_real() {
    let mut model = "owned-service_v3".to_string();
    let mut version = "v3".to_string();
    let descriptor =
        ServiceModelDescriptor::new(Some(&model), Some(&version), "/schedules").unwrap();
    model.clear();
    version.clear();
    assert_eq!(descriptor.model_id().as_str(), "owned-service_v3");
    assert_eq!(descriptor.api_version().as_str(), "v3");
    assert!(
        descriptor
            .employee_dispatcher(2)
            .unwrap()
            .routes()
            .iter()
            .any(|r| r.path == "/v3/schedules")
    );
    let wrong =
        ServiceModelDescriptor::new(Some("owned-service_v3"), Some("v3"), "/v3/other").unwrap();
    assert_eq!(
        wrong.employee_dispatcher(2).err().unwrap().kind,
        ServiceDescriptorErrorKind::ResourceMismatch
    );
    assert_eq!(
        ServiceModelDescriptor::new(Some("owned-service_v3"), Some("v3"), "/v4/schedules")
            .unwrap_err()
            .kind,
        ServiceDescriptorErrorKind::VersionMismatch
    );
    assert_eq!(
        descriptor.employee_dispatcher(0).err().unwrap().kind,
        ServiceDescriptorErrorKind::Dispatcher
    );
}
#[test]
fn additional_native_policy_rejects_unsafe_segments_without_claiming_java_grammar() {
    for invalid in [None, Some("_v1"), Some("model_"), Some("two__parts")] {
        assert!(ServiceModelId::parse(invalid).is_err());
    }
    assert!(ServiceModelId::parse(Some(&"a".repeat(255))).is_ok());
    for invalid in ["v1/extra", "v1?query", " v1", ".."] {
        assert_eq!(
            ServiceApiVersion::parse(Some(invalid)).unwrap_err().kind,
            ServiceDescriptorErrorKind::ApiVersion
        );
    }
    for invalid in ["/v2/schedules/extra", "/schedules?query", "/../"] {
        assert!(ServiceResourcePath::parse(invalid).is_err());
    }
    let resource = ServiceResourcePath::parse("/v20/native-records").unwrap();
    assert_eq!(resource.version().unwrap().as_str(), "v20");
    assert_eq!(resource.resource().as_str(), "native-records");
}
