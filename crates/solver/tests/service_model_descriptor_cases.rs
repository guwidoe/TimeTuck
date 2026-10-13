// Apache-2.0; full19 Community pure contract methods, not Quarkus support.
#[path = "fixtures/service_model_descriptor_fixture.rs"]
mod fixture;
use fixture::*;
use timetuck::*;
fn id_error(value: &str, message: &str) {
    let error = ServiceModelId::parse(Some(value)).unwrap_err();
    assert_eq!(error.kind, ServiceDescriptorErrorKind::ModelId);
    assert_eq!(error.message, message);
}
#[test]
fn test_valid_model_id() {
    assert!(ServiceModelId::parse(Some(VALID_MODEL_ID)).is_ok());
}
#[test]
fn test_empty_model_id() {
    id_error(ID_EXAMPLES[0], "Model name cannot be null or empty.");
}
#[test]
fn test_model_id_with_multiple_underscores() {
    id_error(ID_EXAMPLES[1], "Model name cannot contain underscore (_).");
}
#[test]
fn test_whitespace_model_id() {
    id_error(ID_EXAMPLES[2], EXPECTED_ID_CHARACTERS);
}
#[test]
fn test_model_id_with_emoji() {
    id_error(ID_EXAMPLES[3], EXPECTED_ID_CHARACTERS);
}
#[test]
fn test_system_model_id() {
    id_error(ID_EXAMPLES[4], EXPECTED_ID_CHARACTERS);
}
#[test]
fn test_model_id_with_dot_system() {
    id_error(ID_EXAMPLES[5], EXPECTED_ID_CHARACTERS);
}
#[test]
fn test_model_id_with_null_character() {
    id_error(ID_EXAMPLES[6], EXPECTED_ID_CHARACTERS);
}
#[test]
fn test_model_id_with_dollar_not_allowed() {
    id_error(ID_EXAMPLES[7], EXPECTED_ID_CHARACTERS);
}
#[test]
fn test_too_long_model_id() {
    id_error(&too_long_id(), "Model name and version is too long.");
}
#[test]
fn valid_version_does_not_throw() {
    assert!(ServiceApiVersion::parse(VERSION_EXAMPLES[0]).is_ok());
}
fn version_error(value: Option<&str>) {
    let error = ServiceApiVersion::parse(value).unwrap_err();
    assert_eq!(error.kind, ServiceDescriptorErrorKind::ApiVersion);
    assert!(error.message.contains("timefold.model.api-version"));
}
#[test]
fn null_version_throws() {
    version_error(VERSION_EXAMPLES[1]);
}
#[test]
fn empty_version_throws() {
    version_error(VERSION_EXAMPLES[2]);
}
#[test]
fn blank_version_throws() {
    version_error(VERSION_EXAMPLES[3]);
}
#[test]
fn versioned_path() {
    assert_eq!(
        ServiceResourcePath::parse(PATH_EXAMPLES[0])
            .unwrap()
            .resource(),
        &ServiceResourceType::new("schedules").unwrap()
    );
}
#[test]
fn non_versioned_path() {
    assert_eq!(
        ServiceResourcePath::parse(PATH_EXAMPLES[1])
            .unwrap()
            .resource(),
        &ServiceResourceType::new("schedules").unwrap()
    );
}
fn path_error(path: &str, message: &str) {
    let error = ServiceResourcePath::parse(path).unwrap_err();
    assert_eq!(error.kind, ServiceDescriptorErrorKind::ResourcePath);
    assert_eq!(error.message, message);
}
#[test]
fn empty_path() {
    path_error(
        PATH_EXAMPLES[2],
        "Could not derive model resource type: ModelRest @Path value is empty.",
    );
}
#[test]
fn missing_path_segment() {
    path_error(
        PATH_EXAMPLES[3],
        "Could not derive model resource type: ModelRest @Path does not contain path segments.",
    );
}
#[test]
fn version_only_path() {
    path_error(
        PATH_EXAMPLES[4],
        "Could not derive model resource type: ModelRest @Path only contains API version but no resource segment.",
    );
}
