// Apache-2.0; literal Community data-contract TEST fixtures, see provenance.
pub const EXPECTED_ID_CHARACTERS: &str =
    "Model name can only contain letters (a-z, A-Z) and numbers (0-9), and hyphens (-).";
pub const VALID_MODEL_ID: &str = "employee-scheduling_v1";
pub const ID_EXAMPLES: [&str; 8] = [
    "",
    "invalid_model_name",
    "model id with space",
    "🚀Rocket_123",
    "system.users",
    "example.system.model",
    "invalid\0id",
    "oplog.$main",
];
pub fn too_long_id() -> String {
    format!("{}_", "a".repeat(255))
}
pub const VERSION_EXAMPLES: [Option<&str>; 4] = [Some("v1"), None, Some(""), Some("   ")];
pub const PATH_EXAMPLES: [&str; 5] = ["/v1/schedules", "/schedules", "", "/", "/v1"];
