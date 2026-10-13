// Apache-2.0; full OpenApiTest.testOperationHasPriorityQueryParam adaptation.
use timetuck::*;
#[test]
fn test_operation_has_priority_query_param() {
    let application_version = "v2-beta";
    let dispatcher = EmployeeRouteDispatcher::new(application_version, 8).unwrap();
    let directory =
        std::env::temp_dir().join(format!("timetuck-openapi-source-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let file = directory.join("service.json");
    std::fs::write(&file, serde_json::to_vec(&dispatcher.openapi()).unwrap()).unwrap();
    assert!(file.exists());
    let document: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
    let paths = document["paths"].as_object().unwrap();
    // Native names explicitly adapt the source public OperationId symbols.
    for operation_id in ["schedule", "solveDataset", "fromInput", "fromPatch"] {
        let operation = paths
            .values()
            .filter_map(|path| path.get("post"))
            .find(|op| op["operationId"] == operation_id)
            .unwrap_or_else(|| panic!("Could not find operation {operation_id}"));
        assert!(
            operation["parameters"]
                .as_array()
                .unwrap()
                .iter()
                .any(|p| p["name"] == "priority" && p["in"] == "query")
        );
    }
    for path in paths.keys() {
        assert!(path.starts_with(&format!("/{application_version}")));
    }
    std::fs::remove_dir_all(directory).unwrap();
}
