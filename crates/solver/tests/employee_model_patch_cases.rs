// Adapted from pinned Timefold Community JSON patch tests. Apache-2.0.
// Original patch payloads, resource inputs and assertions; see compat/provenance.json.
use timetuck::{JsonPatch, JsonPatchMode, JsonValue};
fn text(value: &JsonValue) -> String {
    value
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| value.to_string())
}

#[test]
fn test_add_employee() {
    let patch = r##"[{"op": "add", "path": "/employees/-", "value": {"id": "john doe", "contracts": ["fullTimeContract"], "skills": [{"id": "Ambulance"}]}}]"##;
    let mut model_input: JsonValue = serde_json::from_str(include_str!(
        "fixtures/model_patch/CONTRACT_RULES_input.json"
    ))
    .unwrap();
    let number_of_employees = model_input["employees"].as_array().unwrap().len();
    JsonPatch::parse(patch, JsonPatchMode::Model)
        .unwrap()
        .apply(&mut model_input)
        .unwrap();
    let model_input_patched = &model_input;
    let number_of_employees_after_patch =
        model_input_patched["employees"].as_array().unwrap().len();
    assert_eq!(number_of_employees + 1, number_of_employees_after_patch);
    let added_employee =
        (model_input_patched["employees"][number_of_employees_after_patch - 1]).clone();
    assert_eq!("john doe", text(&added_employee["id"]));
    assert_eq!("fullTimeContract", text(&added_employee["contracts"][0]));
    assert_eq!("Ambulance", text(&added_employee["skills"][0]["id"]));
}

#[test]
fn test_add_contract_to_employee() {
    let patch =
        r##"[{"op": "add", "path": "/employees/[id=Ann Cole]/contracts/-", "value": "test"}]"##;
    let mut model_input: JsonValue = serde_json::from_str(include_str!(
        "fixtures/model_patch/CONTRACT_RULES_input.json"
    ))
    .unwrap();
    let number_of_employees = model_input["employees"].as_array().unwrap().len();
    JsonPatch::parse(patch, JsonPatchMode::Model)
        .unwrap()
        .apply(&mut model_input)
        .unwrap();
    let model_input_patched = &model_input;
    let number_of_employees_after_patch =
        model_input_patched["employees"].as_array().unwrap().len();
    assert_eq!(number_of_employees, number_of_employees_after_patch);
    let employee = (model_input_patched["employees"][0]).clone();
    assert_eq!("fullTimeContract", text(&employee["contracts"][0]));
    assert_eq!("test", text(&employee["contracts"][1]));
}

#[test]
fn test_add_unavailable_time_spans_to_employee() {
    let patch = r##"[{"op": "add", "path": "/employees/[id=Ann Cole]/unavailableTimeSpans/-", "value": {"start": "2027-02-01T00:00:00Z", "end": "2027-02-02T00:00:00Z"}}]"##;
    let mut model_input: JsonValue = serde_json::from_str(include_str!(
        "fixtures/model_patch/CONTRACT_RULES_input.json"
    ))
    .unwrap();
    let number_of_employees = model_input["employees"].as_array().unwrap().len();
    JsonPatch::parse(patch, JsonPatchMode::Model)
        .unwrap()
        .apply(&mut model_input)
        .unwrap();
    let model_input_patched = &model_input;
    let number_of_employees_after_patch =
        model_input_patched["employees"].as_array().unwrap().len();
    assert_eq!(number_of_employees, number_of_employees_after_patch);
    let employee = (model_input_patched["employees"][0]).clone();
    assert_eq!(
        1,
        employee["unavailableTimeSpans"].as_array().unwrap().len()
    );
}

#[test]
fn test_remove_contract_from_employee() {
    let patch = r##"[{"op": "remove", "path": "/employees/[id=Ann Cole]/contracts/0"}]"##;
    let mut model_input: JsonValue = serde_json::from_str(include_str!(
        "fixtures/model_patch/CONTRACT_RULES_input.json"
    ))
    .unwrap();
    let number_of_employees = model_input["employees"].as_array().unwrap().len();
    JsonPatch::parse(patch, JsonPatchMode::Model)
        .unwrap()
        .apply(&mut model_input)
        .unwrap();
    let model_input_patched = &model_input;
    let number_of_employees_after_patch =
        model_input_patched["employees"].as_array().unwrap().len();
    assert_eq!(number_of_employees, number_of_employees_after_patch);
    let employee = (model_input_patched["employees"][0]).clone();
    assert_eq!(0, employee["contracts"].as_array().unwrap().len());
}

#[test]
fn test_remove_contract_from_employee_by_name() {
    let patch =
        r##"[{"op": "remove", "path": "/employees/[id=Ann Cole]/contracts/[fullTimeContract]"}]"##;
    let mut model_input: JsonValue = serde_json::from_str(include_str!(
        "fixtures/model_patch/CONTRACT_RULES_input.json"
    ))
    .unwrap();
    let number_of_employees = model_input["employees"].as_array().unwrap().len();
    JsonPatch::parse(patch, JsonPatchMode::Model)
        .unwrap()
        .apply(&mut model_input)
        .unwrap();
    let model_input_patched = &model_input;
    let number_of_employees_after_patch =
        model_input_patched["employees"].as_array().unwrap().len();
    assert_eq!(number_of_employees, number_of_employees_after_patch);
    let employee = (model_input_patched["employees"][0]).clone();
    assert_eq!(0, employee["contracts"].as_array().unwrap().len());
}

#[test]
fn test_add_skill_employee() {
    let patch = r##"[{"op": "add", "path": "/employees/[id=Ann Cole]/skills/-", "value": {"id": "Test"}}]"##;
    let mut model_input: JsonValue = serde_json::from_str(include_str!(
        "fixtures/model_patch/CONTRACT_RULES_input.json"
    ))
    .unwrap();
    let number_of_employees = model_input["employees"].as_array().unwrap().len();
    JsonPatch::parse(patch, JsonPatchMode::Model)
        .unwrap()
        .apply(&mut model_input)
        .unwrap();
    let model_input_patched = &model_input;
    let number_of_employees_after_patch =
        model_input_patched["employees"].as_array().unwrap().len();
    assert_eq!(number_of_employees, number_of_employees_after_patch);
    let emp = (model_input_patched.pointer("/employees/0").unwrap()).clone();
    let skills = (emp["skills"]).clone();
    assert_eq!(2, skills.as_array().unwrap().len());
    let skill_ids: Vec<_> = skills
        .as_array()
        .unwrap()
        .iter()
        .map(|item| text(&item["id"]))
        .collect();
    assert_eq!(skill_ids, ["Ambulance", "Test"]);
}

#[test]
fn test_add_skill_employee_not_existing() {
    let patch = r##"[{"op": "add", "path": "/employees/[id=Not existing]/skills/-", "value": {"id": "Test"}}]"##;
    let mut model_input: JsonValue = serde_json::from_str(include_str!(
        "fixtures/model_patch/CONTRACT_RULES_input.json"
    ))
    .unwrap();
    let number_of_employees = model_input["employees"].as_array().unwrap().len();
    JsonPatch::parse(patch, JsonPatchMode::Model)
        .unwrap()
        .apply(&mut model_input)
        .unwrap();
    let model_input_patched = &model_input;
    let number_of_employees_after_patch =
        model_input_patched["employees"].as_array().unwrap().len();
    assert_eq!(number_of_employees, number_of_employees_after_patch);
}

#[test]
fn test_add_prefered_time_span_for_employee() {
    let patch = r##"[{"op": "add", "path": "/employees/[id=Ann Cole]/preferredTimeSpans/-", "value": {"start": "2025-08-05T00:00:00-04:00", "end": "2025-08-07T00:00:00-04:00", "includeShiftTags": ["Night"]}}]"##;
    let mut model_input: JsonValue = serde_json::from_str(include_str!(
        "fixtures/model_patch/CONTRACT_RULES_input.json"
    ))
    .unwrap();
    let number_of_employees = model_input["employees"].as_array().unwrap().len();
    JsonPatch::parse(patch, JsonPatchMode::Model)
        .unwrap()
        .apply(&mut model_input)
        .unwrap();
    let model_input_patched = &model_input;
    let number_of_employees_after_patch =
        model_input_patched["employees"].as_array().unwrap().len();
    assert_eq!(number_of_employees, number_of_employees_after_patch);
    let emp = (model_input_patched.pointer("/employees/0").unwrap()).clone();
    let preferred_time_spans = (emp["preferredTimeSpans"]).clone();
    assert_eq!(3, preferred_time_spans.as_array().unwrap().len());
}

#[test]
fn test_remove_skill_from_employee() {
    let patch = r##"[{"op": "remove", "path": "/employees/[id=Ann Cole]/skills/0"}]"##;
    let mut model_input: JsonValue = serde_json::from_str(include_str!(
        "fixtures/model_patch/CONTRACT_RULES_input.json"
    ))
    .unwrap();
    let number_of_employees = model_input["employees"].as_array().unwrap().len();
    JsonPatch::parse(patch, JsonPatchMode::Model)
        .unwrap()
        .apply(&mut model_input)
        .unwrap();
    let model_input_patched = &model_input;
    let number_of_employees_after_patch =
        model_input_patched["employees"].as_array().unwrap().len();
    assert_eq!(number_of_employees, number_of_employees_after_patch);
    let emp = (model_input_patched.pointer("/employees/0").unwrap()).clone();
    let skills = (emp["skills"]).clone();
    assert_eq!(0, skills.as_array().unwrap().len());
}

#[test]
fn test_remove_skill_from_employee_by_skill_id() {
    let patch = r##"[{"op": "remove", "path": "/employees/[id=Ann Cole]/skills/[id=Ambulance]"}]"##;
    let mut model_input: JsonValue = serde_json::from_str(include_str!(
        "fixtures/model_patch/CONTRACT_RULES_input.json"
    ))
    .unwrap();
    let number_of_employees = model_input["employees"].as_array().unwrap().len();
    JsonPatch::parse(patch, JsonPatchMode::Model)
        .unwrap()
        .apply(&mut model_input)
        .unwrap();
    let model_input_patched = &model_input;
    let number_of_employees_after_patch =
        model_input_patched["employees"].as_array().unwrap().len();
    assert_eq!(number_of_employees, number_of_employees_after_patch);
    let emp = (model_input_patched.pointer("/employees/0").unwrap()).clone();
    let skills = (emp["skills"]).clone();
    assert_eq!(0, skills.as_array().unwrap().len());
}

#[test]
fn test_replace_skill_employee() {
    let patch = r##"[{"op": "replace", "path": "/employees/[id=Ann Cole]/skills", "value": [{"id": "Test"}]}]"##;
    let mut model_input: JsonValue = serde_json::from_str(include_str!(
        "fixtures/model_patch/CONTRACT_RULES_input.json"
    ))
    .unwrap();
    let number_of_employees = model_input["employees"].as_array().unwrap().len();
    JsonPatch::parse(patch, JsonPatchMode::Model)
        .unwrap()
        .apply(&mut model_input)
        .unwrap();
    let model_input_patched = &model_input;
    let number_of_employees_after_patch =
        model_input_patched["employees"].as_array().unwrap().len();
    assert_eq!(number_of_employees, number_of_employees_after_patch);
    let emp = (model_input_patched.pointer("/employees/0").unwrap()).clone();
    let skills = (emp["skills"]).clone();
    assert_eq!(1, skills.as_array().unwrap().len());
    assert_eq!("Test", text(&skills[0]["id"]));
}

#[test]
fn test_replace_time_zone_for_employee() {
    let patch = r##"[{"op": "replace", "path": "/employees/[id=Ann Cole]/timeZoneId", "value": "-08:00"}]"##;
    let mut model_input: JsonValue = serde_json::from_str(include_str!(
        "fixtures/model_patch/CONTRACT_RULES_input.json"
    ))
    .unwrap();
    let number_of_employees = model_input["employees"].as_array().unwrap().len();
    JsonPatch::parse(patch, JsonPatchMode::Model)
        .unwrap()
        .apply(&mut model_input)
        .unwrap();
    let model_input_patched = &model_input;
    let number_of_employees_after_patch =
        model_input_patched["employees"].as_array().unwrap().len();
    assert_eq!(number_of_employees, number_of_employees_after_patch);
    let emp = (model_input_patched.pointer("/employees/0").unwrap()).clone();
    let timezone_id = text(&emp["timeZoneId"]);
    assert_eq!("-08:00", timezone_id);
}

#[test]
fn test_remove_employee_with_skill() {
    let patch = r##"[{"op": "remove", "path": "/employees/[skills/id=Ambulance]"}]"##;
    let mut model_input: JsonValue = serde_json::from_str(include_str!(
        "fixtures/model_patch/CONTRACT_RULES_input.json"
    ))
    .unwrap();
    let number_of_employees = model_input["employees"].as_array().unwrap().len();
    JsonPatch::parse(patch, JsonPatchMode::Model)
        .unwrap()
        .apply(&mut model_input)
        .unwrap();
    let model_input_patched = &model_input;
    let number_of_employees_after_patch =
        model_input_patched["employees"].as_array().unwrap().len();
    assert_eq!(number_of_employees - 5, number_of_employees_after_patch);
}

#[test]
fn test_remove_employee_with_id() {
    let patch = r##"[{"op": "remove", "path": "/employees/[id=Ann Cole]"}]"##;
    let mut model_input: JsonValue = serde_json::from_str(include_str!(
        "fixtures/model_patch/CONTRACT_RULES_input.json"
    ))
    .unwrap();
    let number_of_employees = model_input["employees"].as_array().unwrap().len();
    JsonPatch::parse(patch, JsonPatchMode::Model)
        .unwrap()
        .apply(&mut model_input)
        .unwrap();
    let model_input_patched = &model_input;
    let number_of_employees_after_patch =
        model_input_patched["employees"].as_array().unwrap().len();
    assert_eq!(number_of_employees - 1, number_of_employees_after_patch);
}

#[test]
fn test_remove_employee_by_index() {
    let patch = r##"[{"op": "remove", "path": "/employees/0"}]"##;
    let mut model_input: JsonValue = serde_json::from_str(include_str!(
        "fixtures/model_patch/CONTRACT_RULES_input.json"
    ))
    .unwrap();
    let number_of_employees = model_input["employees"].as_array().unwrap().len();
    JsonPatch::parse(patch, JsonPatchMode::Model)
        .unwrap()
        .apply(&mut model_input)
        .unwrap();
    let model_input_patched = &model_input;
    let number_of_employees_after_patch =
        model_input_patched["employees"].as_array().unwrap().len();
    assert_eq!(number_of_employees - 1, number_of_employees_after_patch);
}

#[test]
fn test_add_shift() {
    let patch = r##"[{"op": "add", "path": "/shifts/-", "value": {"id": "Sun M Ambulance", "start": "2025-08-10T06:00:00-04:00", "end": "2025-08-10T14:00:00-04:00", "requiredSkills": ["Ambulance"], "tags": ["Morning"]}}]"##;
    let mut model_input: JsonValue = serde_json::from_str(include_str!(
        "fixtures/model_patch/CONTRACT_RULES_input.json"
    ))
    .unwrap();
    let number_of_shifts = model_input["shifts"].as_array().unwrap().len();
    JsonPatch::parse(patch, JsonPatchMode::Model)
        .unwrap()
        .apply(&mut model_input)
        .unwrap();
    let model_input_patched = &model_input;
    let number_of_shifts_after_patch = model_input_patched["shifts"].as_array().unwrap().len();
    assert_eq!(number_of_shifts + 1, number_of_shifts_after_patch);
    let added_shift = (model_input_patched["shifts"][number_of_shifts_after_patch - 1]).clone();
    assert_eq!("Sun M Ambulance", text(&added_shift["id"]));
    assert_eq!("Ambulance", text(&added_shift["requiredSkills"][0]));
    assert_eq!("Morning", text(&added_shift["tags"][0]));
}

#[test]
fn test_add_tag_to_shift() {
    let patch =
        r##"[{"op": "add", "path": "/shifts/[id=Mon M Ambulance]/tags/-", "value": "Night"}]"##;
    let mut model_input: JsonValue = serde_json::from_str(include_str!(
        "fixtures/model_patch/CONTRACT_RULES_input.json"
    ))
    .unwrap();
    let number_of_shifts = model_input["shifts"].as_array().unwrap().len();
    JsonPatch::parse(patch, JsonPatchMode::Model)
        .unwrap()
        .apply(&mut model_input)
        .unwrap();
    let model_input_patched = &model_input;
    let number_of_employees_after_patch = model_input_patched["shifts"].as_array().unwrap().len();
    assert_eq!(number_of_shifts, number_of_employees_after_patch);
    let shifts = (model_input_patched["shifts"][0]).clone();
    assert_eq!(2, shifts["tags"].as_array().unwrap().len());
    assert_eq!("Morning", text(&shifts["tags"][0]));
    assert_eq!("Night", text(&shifts["tags"][1]));
}

#[test]
fn test_add_tag_to_employee() {
    let patch = r##"[{"op": "add", "path": "/employees/[id=Ann Cole]/tags/-", "value": "Night"}]"##;
    let mut model_input: JsonValue = serde_json::from_str(include_str!(
        "fixtures/model_patch/CONTRACT_RULES_input.json"
    ))
    .unwrap();
    let number_of_shifts = model_input["employees"].as_array().unwrap().len();
    JsonPatch::parse(patch, JsonPatchMode::Model)
        .unwrap()
        .apply(&mut model_input)
        .unwrap();
    let model_input_patched = &model_input;
    let number_of_employees_after_patch =
        model_input_patched["employees"].as_array().unwrap().len();
    assert_eq!(number_of_shifts, number_of_employees_after_patch);
    let tags = (model_input_patched["employees"][0]).clone();
    assert_eq!(1, tags["tags"].as_array().unwrap().len());
    assert_eq!("Night", text(&tags["tags"][0]));
    let patch =
        r##"[{"op": "replace", "path": "/employees/[id=Ann Cole]/tags", "value": ["Day"]}]"##;
    JsonPatch::parse(patch, JsonPatchMode::Model)
        .unwrap()
        .apply(&mut model_input)
        .unwrap();
    let model_input_patched = &model_input;
    let tags = (model_input_patched["employees"][0]).clone();
    assert_eq!(1, tags["tags"].as_array().unwrap().len());
    assert_eq!("Day", text(&tags["tags"][0]));
}

#[test]
fn test_add_required_skills_to_shift() {
    let patch = r##"[{"op": "add", "path": "/shifts/[id=Mon M Ambulance]/requiredSkills/-", "value": "Surgery"}]"##;
    let mut model_input: JsonValue = serde_json::from_str(include_str!(
        "fixtures/model_patch/CONTRACT_RULES_input.json"
    ))
    .unwrap();
    let number_of_shifts = model_input["shifts"].as_array().unwrap().len();
    JsonPatch::parse(patch, JsonPatchMode::Model)
        .unwrap()
        .apply(&mut model_input)
        .unwrap();
    let model_input_patched = &model_input;
    let number_of_employees_after_patch = model_input_patched["shifts"].as_array().unwrap().len();
    assert_eq!(number_of_shifts, number_of_employees_after_patch);
    let shifts = (model_input_patched["shifts"][0]).clone();
    assert_eq!(2, shifts["requiredSkills"].as_array().unwrap().len());
    assert_eq!("Ambulance", text(&shifts["requiredSkills"][0]));
    assert_eq!("Surgery", text(&shifts["requiredSkills"][1]));
}

#[test]
fn test_remove_required_skills_from_shift() {
    let patch = r##"[{"op": "remove", "path": "/shifts/[id=Mon M Ambulance]/requiredSkills/0"}]"##;
    let mut model_input: JsonValue = serde_json::from_str(include_str!(
        "fixtures/model_patch/CONTRACT_RULES_input.json"
    ))
    .unwrap();
    let number_of_shifts = model_input["shifts"].as_array().unwrap().len();
    JsonPatch::parse(patch, JsonPatchMode::Model)
        .unwrap()
        .apply(&mut model_input)
        .unwrap();
    let model_input_patched = &model_input;
    let number_of_employees_after_patch = model_input_patched["shifts"].as_array().unwrap().len();
    assert_eq!(number_of_shifts, number_of_employees_after_patch);
    let shifts = (model_input_patched["shifts"][0]).clone();
    assert_eq!(0, shifts["requiredSkills"].as_array().unwrap().len());
}

#[test]
fn test_remove_required_skills_from_shift_by_name() {
    let patch =
        r##"[{"op": "remove", "path": "/shifts/[id=Mon M Ambulance]/requiredSkills/Ambulance"}]"##;
    let mut model_input: JsonValue = serde_json::from_str(include_str!(
        "fixtures/model_patch/CONTRACT_RULES_input.json"
    ))
    .unwrap();
    let number_of_shifts = model_input["shifts"].as_array().unwrap().len();
    JsonPatch::parse(patch, JsonPatchMode::Model)
        .unwrap()
        .apply(&mut model_input)
        .unwrap();
    let model_input_patched = &model_input;
    let number_of_employees_after_patch = model_input_patched["shifts"].as_array().unwrap().len();
    assert_eq!(number_of_shifts, number_of_employees_after_patch);
    let shifts = (model_input_patched["shifts"][0]).clone();
    assert_eq!(0, shifts["requiredSkills"].as_array().unwrap().len());
}

#[test]
fn test_remove_shift_by_id() {
    let patch = r##"[{"op": "remove", "path": "/shifts/[id=Mon M Ambulance]"}]"##;
    let mut model_input: JsonValue = serde_json::from_str(include_str!(
        "fixtures/model_patch/CONTRACT_RULES_input.json"
    ))
    .unwrap();
    let number_of_shifts = model_input["shifts"].as_array().unwrap().len();
    JsonPatch::parse(patch, JsonPatchMode::Model)
        .unwrap()
        .apply(&mut model_input)
        .unwrap();
    let model_input_patched = &model_input;
    let number_of_employees_after_patch = model_input_patched["shifts"].as_array().unwrap().len();
    assert_eq!(number_of_shifts - 1, number_of_employees_after_patch);
}
