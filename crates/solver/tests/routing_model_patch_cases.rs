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
fn test_add_vehicle() {
    let patch = r##"[{"op": "add", "path": "/vehicles/-", "value": {"id": "AnnExtra", "vehicleType": "VAN", "shifts": [{"id": "cd4c29ed", "startLocation": [33.771758735816796, -84.37174927400983], "minStartTime": "2025-08-18T08:00:00-04:00", "maxEndTime": "2025-08-18T17:00:00-04:00", "skills": [{"name": "electrician", "level": 1, "multiplier": null}], "tags": [], "requiredBreaks": [{"id": "cd4c29ed Lunch", "minStartTime": "2025-08-18T12:00:00-04:00", "maxEndTime": null, "duration": "PT1H", "costImpact": "PAID", "type": "FLOATING"}], "temporarySkillSets": [], "temporaryTagSets": [], "itinerary": []}], "historicalTimeUtilized": "PT0S", "historicalTimeCapacity": "PT0S"}}]"##;
    let mut model_input: JsonValue =
        serde_json::from_str(include_str!("fixtures/model_patch/VISIT_GROUP_input.json")).unwrap();
    let number_of_vehicles = model_input["vehicles"].as_array().unwrap().len();
    JsonPatch::parse(patch, JsonPatchMode::Model)
        .unwrap()
        .apply(&mut model_input)
        .unwrap();
    let model_input_patched = &model_input;
    let number_of_vehicles_after_patch = model_input_patched["vehicles"].as_array().unwrap().len();
    assert_eq!(number_of_vehicles + 1, number_of_vehicles_after_patch);
    let added_vehicle =
        (model_input_patched["vehicles"][number_of_vehicles_after_patch - 1]).clone();
    assert_eq!("AnnExtra", text(&added_vehicle["id"]));
    assert_eq!("VAN", text(&added_vehicle["vehicleType"]));
}

#[test]
fn test_add_shift_to_vehicle() {
    let patch = r##"[{"op": "add", "path": "/vehicles/[id=Ann]/shifts/-", "value": {"id": "cd4c29ex", "startLocation": [33.771758735816796, -84.37174927400983], "minStartTime": "2025-08-18T08:00:00-04:00", "maxEndTime": "2025-08-18T17:00:00-04:00", "skills": [{"name": "electrician", "level": 1, "multiplier": null}], "tags": [], "requiredBreaks": [{"id": "cd4c29ed Lunch", "minStartTime": "2025-08-18T12:00:00-04:00", "maxEndTime": null, "duration": "PT1H", "costImpact": "PAID", "type": "FLOATING"}], "temporarySkillSets": [], "temporaryTagSets": [], "itinerary": []}}]"##;
    let mut model_input: JsonValue =
        serde_json::from_str(include_str!("fixtures/model_patch/VISIT_GROUP_input.json")).unwrap();
    let number_of_vehicles = model_input["vehicles"].as_array().unwrap().len();
    JsonPatch::parse(patch, JsonPatchMode::Model)
        .unwrap()
        .apply(&mut model_input)
        .unwrap();
    let model_input_patched = &model_input;
    let number_of_vehicles_after_patch = model_input_patched["vehicles"].as_array().unwrap().len();
    assert_eq!(number_of_vehicles, number_of_vehicles_after_patch);
    let vehicle = (model_input_patched["vehicles"][0]).clone();
    assert_eq!(2, vehicle["shifts"].as_array().unwrap().len());
}

#[test]
fn test_add_tag_to_vehicle_by_shift_index() {
    let patch =
        r##"[{"op": "add", "path": "/vehicles/[id=Ann]/shifts/0/tags/-", "value": "emergency"}]"##;
    let mut model_input: JsonValue =
        serde_json::from_str(include_str!("fixtures/model_patch/VISIT_GROUP_input.json")).unwrap();
    let number_of_vehicles = model_input["vehicles"].as_array().unwrap().len();
    JsonPatch::parse(patch, JsonPatchMode::Model)
        .unwrap()
        .apply(&mut model_input)
        .unwrap();
    let model_input_patched = &model_input;
    let number_of_vehicles_after_patch = model_input_patched["vehicles"].as_array().unwrap().len();
    assert_eq!(number_of_vehicles, number_of_vehicles_after_patch);
    let vehicle = (model_input_patched["vehicles"][0]).clone();
    assert_eq!(
        1,
        vehicle
            .pointer("/shifts/0/tags")
            .unwrap()
            .as_array()
            .unwrap()
            .len()
    );
    assert_eq!(
        "emergency",
        text(&vehicle.pointer("/shifts/0/tags").unwrap()[0])
    );
}

#[test]
fn test_add_tag_to_vehicle_by_shift_id() {
    let patch = r##"[{"op": "add", "path": "/vehicles/[id=Ann]/shifts/[id=cd4c29ed]/tags/-", "value": "emergency"}]"##;
    let mut model_input: JsonValue =
        serde_json::from_str(include_str!("fixtures/model_patch/VISIT_GROUP_input.json")).unwrap();
    let number_of_vehicles = model_input["vehicles"].as_array().unwrap().len();
    JsonPatch::parse(patch, JsonPatchMode::Model)
        .unwrap()
        .apply(&mut model_input)
        .unwrap();
    let model_input_patched = &model_input;
    let number_of_vehicles_after_patch = model_input_patched["vehicles"].as_array().unwrap().len();
    assert_eq!(number_of_vehicles, number_of_vehicles_after_patch);
    let vehicle = (model_input_patched["vehicles"][0]).clone();
    assert_eq!(
        1,
        vehicle
            .pointer("/shifts/0/tags")
            .unwrap()
            .as_array()
            .unwrap()
            .len()
    );
    assert_eq!(
        "emergency",
        text(&vehicle.pointer("/shifts/0/tags").unwrap()[0])
    );
}

#[test]
fn test_add_skill_to_vehicle_by_shift_id() {
    let patch = r##"[{"op": "add", "path": "/vehicles/[id=Ann]/shifts/[id=cd4c29ed]/skills/-", "value": {"name": "musician", "level": 1, "multiplier": null}}]"##;
    let mut model_input: JsonValue =
        serde_json::from_str(include_str!("fixtures/model_patch/VISIT_GROUP_input.json")).unwrap();
    let number_of_vehicles = model_input["vehicles"].as_array().unwrap().len();
    JsonPatch::parse(patch, JsonPatchMode::Model)
        .unwrap()
        .apply(&mut model_input)
        .unwrap();
    let model_input_patched = &model_input;
    let number_of_vehicles_after_patch = model_input_patched["vehicles"].as_array().unwrap().len();
    assert_eq!(number_of_vehicles, number_of_vehicles_after_patch);
    let vehicle = (model_input_patched["vehicles"][0]).clone();
    assert_eq!(
        2,
        vehicle
            .pointer("/shifts/0/skills")
            .unwrap()
            .as_array()
            .unwrap()
            .len()
    );
    assert_eq!(
        "electrician",
        text(&vehicle.pointer("/shifts/0/skills").unwrap()[0]["name"])
    );
    assert_eq!(
        "musician",
        text(&vehicle.pointer("/shifts/0/skills").unwrap()[1]["name"])
    );
}

#[test]
fn test_replace_skills_for_vehicle_by_shift_id() {
    let patch = r##"[{"op": "replace", "path": "/vehicles/[id=Ann]/shifts/[id=cd4c29ed]/skills", "value": [{"name": "musician", "level": 1, "multiplier": null}]}]"##;
    let mut model_input: JsonValue =
        serde_json::from_str(include_str!("fixtures/model_patch/VISIT_GROUP_input.json")).unwrap();
    let number_of_vehicles = model_input["vehicles"].as_array().unwrap().len();
    JsonPatch::parse(patch, JsonPatchMode::Model)
        .unwrap()
        .apply(&mut model_input)
        .unwrap();
    let model_input_patched = &model_input;
    let number_of_vehicles_after_patch = model_input_patched["vehicles"].as_array().unwrap().len();
    assert_eq!(number_of_vehicles, number_of_vehicles_after_patch);
    let vehicle = (model_input_patched["vehicles"][0]).clone();
    assert_eq!(
        1,
        vehicle
            .pointer("/shifts/0/skills")
            .unwrap()
            .as_array()
            .unwrap()
            .len()
    );
    assert_eq!(
        "musician",
        text(&vehicle.pointer("/shifts/0/skills").unwrap()[0]["name"])
    );
}

#[test]
fn test_replace_required_breaks_for_vehicle_by_shift_id() {
    let patch = r##"[{"op": "replace", "path": "/vehicles/[id=Ann]/shifts/[id=cd4c29ed]/requiredBreaks", "value": [{"id": "cd4c29ed Lunch", "minStartTime": "2025-08-18T12:00:00-04:00", "maxEndTime": null, "duration": "PT1H", "costImpact": "PAID", "type": "FLOATING"}, {"id": "cd4c29ed Dinner", "minStartTime": "2025-08-18T18:00:00-04:00", "maxEndTime": null, "duration": "PT1H", "costImpact": "PAID", "type": "FLOATING"}]}]"##;
    let mut model_input: JsonValue =
        serde_json::from_str(include_str!("fixtures/model_patch/VISIT_GROUP_input.json")).unwrap();
    let number_of_vehicles = model_input["vehicles"].as_array().unwrap().len();
    JsonPatch::parse(patch, JsonPatchMode::Model)
        .unwrap()
        .apply(&mut model_input)
        .unwrap();
    let model_input_patched = &model_input;
    let number_of_vehicles_after_patch = model_input_patched["vehicles"].as_array().unwrap().len();
    assert_eq!(number_of_vehicles, number_of_vehicles_after_patch);
    let vehicle = (model_input_patched["vehicles"][0]).clone();
    assert_eq!(
        2,
        vehicle
            .pointer("/shifts/0/requiredBreaks")
            .unwrap()
            .as_array()
            .unwrap()
            .len()
    );
    assert_eq!(
        "cd4c29ed Lunch",
        text(&vehicle.pointer("/shifts/0/requiredBreaks").unwrap()[0]["id"])
    );
    assert_eq!(
        "cd4c29ed Dinner",
        text(&vehicle.pointer("/shifts/0/requiredBreaks").unwrap()[1]["id"])
    );
}

#[test]
fn test_remove_required_breaks_for_vehicle_by_shift_id() {
    let patch = r##"[{"op": "remove", "path": "/vehicles/[id=Ann]/shifts/[id=cd4c29ed]/requiredBreaks/[id=cd4c29ed Lunch]"}]"##;
    let mut model_input: JsonValue =
        serde_json::from_str(include_str!("fixtures/model_patch/VISIT_GROUP_input.json")).unwrap();
    let number_of_vehicles = model_input["vehicles"].as_array().unwrap().len();
    JsonPatch::parse(patch, JsonPatchMode::Model)
        .unwrap()
        .apply(&mut model_input)
        .unwrap();
    let model_input_patched = &model_input;
    let number_of_vehicles_after_patch = model_input_patched["vehicles"].as_array().unwrap().len();
    assert_eq!(number_of_vehicles, number_of_vehicles_after_patch);
    let vehicle = (model_input_patched["vehicles"][0]).clone();
    assert_eq!(
        0,
        vehicle
            .pointer("/shifts/0/requiredBreaks")
            .unwrap()
            .as_array()
            .unwrap()
            .len()
    );
}

#[test]
fn test_remove_required_breaks_for_vehicle_by_shift_id_index() {
    let patch = r##"[{"op": "remove", "path": "/vehicles/[id=Ann]/shifts/[id=cd4c29ed]/requiredBreaks/0"}]"##;
    let mut model_input: JsonValue =
        serde_json::from_str(include_str!("fixtures/model_patch/VISIT_GROUP_input.json")).unwrap();
    let number_of_vehicles = model_input["vehicles"].as_array().unwrap().len();
    JsonPatch::parse(patch, JsonPatchMode::Model)
        .unwrap()
        .apply(&mut model_input)
        .unwrap();
    let model_input_patched = &model_input;
    let number_of_vehicles_after_patch = model_input_patched["vehicles"].as_array().unwrap().len();
    assert_eq!(number_of_vehicles, number_of_vehicles_after_patch);
    let vehicle = (model_input_patched["vehicles"][0]).clone();
    assert_eq!(
        0,
        vehicle
            .pointer("/shifts/0/requiredBreaks")
            .unwrap()
            .as_array()
            .unwrap()
            .len()
    );
}

#[test]
fn test_remove_shift_for_vehicle_by_shift_id() {
    let patch = r##"[{"op": "remove", "path": "/vehicles/[id=Ann]/shifts/[id=cd4c29ed]"}]"##;
    let mut model_input: JsonValue =
        serde_json::from_str(include_str!("fixtures/model_patch/VISIT_GROUP_input.json")).unwrap();
    let number_of_vehicles = model_input["vehicles"].as_array().unwrap().len();
    JsonPatch::parse(patch, JsonPatchMode::Model)
        .unwrap()
        .apply(&mut model_input)
        .unwrap();
    let model_input_patched = &model_input;
    let number_of_vehicles_after_patch = model_input_patched["vehicles"].as_array().unwrap().len();
    assert_eq!(number_of_vehicles, number_of_vehicles_after_patch);
    let vehicle = (model_input_patched["vehicles"][0]).clone();
    assert_eq!(
        0,
        vehicle
            .pointer("/shifts")
            .unwrap()
            .as_array()
            .unwrap()
            .len()
    );
}

#[test]
fn test_remove_shift_for_vehicle_by_index() {
    let patch = r##"[{"op": "remove", "path": "/vehicles/[id=Ann]/shifts/0"}]"##;
    let mut model_input: JsonValue =
        serde_json::from_str(include_str!("fixtures/model_patch/VISIT_GROUP_input.json")).unwrap();
    let number_of_vehicles = model_input["vehicles"].as_array().unwrap().len();
    JsonPatch::parse(patch, JsonPatchMode::Model)
        .unwrap()
        .apply(&mut model_input)
        .unwrap();
    let model_input_patched = &model_input;
    let number_of_vehicles_after_patch = model_input_patched["vehicles"].as_array().unwrap().len();
    assert_eq!(number_of_vehicles, number_of_vehicles_after_patch);
    let vehicle = (model_input_patched["vehicles"][0]).clone();
    assert_eq!(
        0,
        vehicle
            .pointer("/shifts")
            .unwrap()
            .as_array()
            .unwrap()
            .len()
    );
}

#[test]
fn test_add_visit() {
    let patch = r##"[{"op": "add", "path": "/visits/-", "value": {"id": "6e57fcdx", "name": "Cole Inc. extra", "location": [33.68906251585709, -84.44268080179887], "timeWindows": [{"minStartTime": "2025-08-18T09:00:00-04:00", "maxEndTime": "2025-08-18T17:00:00-04:00"}], "serviceDuration": "PT1H15M", "requiredSkills": [{"name": "plumber", "minLevel": null}], "priority": "10", "pinningRequested": false}}]"##;
    let mut model_input: JsonValue =
        serde_json::from_str(include_str!("fixtures/model_patch/VISIT_GROUP_input.json")).unwrap();
    let number_of_visits = model_input["visits"].as_array().unwrap().len();
    JsonPatch::parse(patch, JsonPatchMode::Model)
        .unwrap()
        .apply(&mut model_input)
        .unwrap();
    let model_input_patched = &model_input;
    let number_of_visits_after_patch = model_input_patched["visits"].as_array().unwrap().len();
    assert_eq!(number_of_visits + 1, number_of_visits_after_patch);
    let added_visit = (model_input_patched["visits"][number_of_visits_after_patch - 1]).clone();
    assert_eq!("6e57fcdx", text(&added_visit["id"]));
    assert_eq!("Cole Inc. extra", text(&added_visit["name"]));
}

#[test]
fn test_add_visits_required_skill() {
    let patch = r##"[{"op": "add", "path": "/visits/[id=6e57fcd4]/requiredSkills/-", "value": {"name": "electrician", "minLevel": null}}]"##;
    let mut model_input: JsonValue =
        serde_json::from_str(include_str!("fixtures/model_patch/VISIT_GROUP_input.json")).unwrap();
    let number_of_visits = model_input["visits"].as_array().unwrap().len();
    JsonPatch::parse(patch, JsonPatchMode::Model)
        .unwrap()
        .apply(&mut model_input)
        .unwrap();
    let model_input_patched = &model_input;
    let number_of_visits_after_patch = model_input_patched["visits"].as_array().unwrap().len();
    assert_eq!(number_of_visits, number_of_visits_after_patch);
    let visit = (model_input_patched["visits"][0]).clone();
    assert_eq!(
        2,
        visit
            .pointer("/requiredSkills")
            .unwrap()
            .as_array()
            .unwrap()
            .len()
    );
    assert_eq!(
        "plumber",
        text(&visit.pointer("/requiredSkills").unwrap()[0]["name"])
    );
    assert_eq!(
        "electrician",
        text(&visit.pointer("/requiredSkills").unwrap()[1]["name"])
    );
}

#[test]
fn test_remove_visits_required_skill() {
    let patch =
        r##"[{"op": "remove", "path": "/visits/[id=6e57fcd4]/requiredSkills/[name=plumber]"}]"##;
    let mut model_input: JsonValue =
        serde_json::from_str(include_str!("fixtures/model_patch/VISIT_GROUP_input.json")).unwrap();
    let number_of_visits = model_input["visits"].as_array().unwrap().len();
    JsonPatch::parse(patch, JsonPatchMode::Model)
        .unwrap()
        .apply(&mut model_input)
        .unwrap();
    let model_input_patched = &model_input;
    let number_of_visits_after_patch = model_input_patched["visits"].as_array().unwrap().len();
    assert_eq!(number_of_visits, number_of_visits_after_patch);
    let visit = (model_input_patched["visits"][0]).clone();
    assert_eq!(
        0,
        visit
            .pointer("/requiredSkills")
            .unwrap()
            .as_array()
            .unwrap()
            .len()
    );
}

#[test]
fn test_remove_visits_by_id() {
    let patch = r##"[{"op": "remove", "path": "/visits/[id=6e57fcd4]"}]"##;
    let mut model_input: JsonValue =
        serde_json::from_str(include_str!("fixtures/model_patch/VISIT_GROUP_input.json")).unwrap();
    let number_of_visits = model_input["visits"].as_array().unwrap().len();
    JsonPatch::parse(patch, JsonPatchMode::Model)
        .unwrap()
        .apply(&mut model_input)
        .unwrap();
    let model_input_patched = &model_input;
    let number_of_visits_after_patch = model_input_patched["visits"].as_array().unwrap().len();
    assert_eq!(number_of_visits - 1, number_of_visits_after_patch);
}

#[test]
fn test_replace_visits_by_id() {
    let patch = r##"[{"op": "replace", "path": "/visits/[id=6e57fcd4]", "value": {"id": "6e57fcdx", "name": "Cole Inc. extra", "location": [33.68906251585709, -84.44268080179887], "timeWindows": [{"minStartTime": "2025-08-18T09:00:00-04:00", "maxEndTime": "2025-08-18T17:00:00-04:00"}], "serviceDuration": "PT1H15M", "requiredSkills": [{"name": "plumber", "minLevel": null}], "priority": "10", "pinningRequested": false}}]"##;
    let mut model_input: JsonValue =
        serde_json::from_str(include_str!("fixtures/model_patch/VISIT_GROUP_input.json")).unwrap();
    let number_of_visits = model_input["visits"].as_array().unwrap().len();
    JsonPatch::parse(patch, JsonPatchMode::Model)
        .unwrap()
        .apply(&mut model_input)
        .unwrap();
    let model_input_patched = &model_input;
    let number_of_visits_after_patch = model_input_patched["visits"].as_array().unwrap().len();
    assert_eq!(number_of_visits, number_of_visits_after_patch);
    let added_visit = (model_input_patched["visits"][number_of_visits_after_patch - 1]).clone();
    assert_eq!("6e57fcdx", text(&added_visit["id"]));
    assert_eq!("Cole Inc. extra", text(&added_visit["name"]));
}

#[test]
fn test_replace_visits_by_index() {
    let patch = r##"[{"op": "replace", "path": "/visits/0", "value": {"id": "6e57fcdx", "name": "Cole Inc. extra", "location": [33.68906251585709, -84.44268080179887], "timeWindows": [{"minStartTime": "2025-08-18T09:00:00-04:00", "maxEndTime": "2025-08-18T17:00:00-04:00"}], "serviceDuration": "PT1H15M", "requiredSkills": [{"name": "plumber", "minLevel": null}], "priority": "10", "pinningRequested": false}}]"##;
    let mut model_input: JsonValue =
        serde_json::from_str(include_str!("fixtures/model_patch/VISIT_GROUP_input.json")).unwrap();
    let number_of_visits = model_input["visits"].as_array().unwrap().len();
    JsonPatch::parse(patch, JsonPatchMode::Model)
        .unwrap()
        .apply(&mut model_input)
        .unwrap();
    let model_input_patched = &model_input;
    let number_of_visits_after_patch = model_input_patched["visits"].as_array().unwrap().len();
    assert_eq!(number_of_visits, number_of_visits_after_patch);
    let added_visit = (model_input_patched["visits"][0]).clone();
    assert_eq!("6e57fcdx", text(&added_visit["id"]));
    assert_eq!("Cole Inc. extra", text(&added_visit["name"]));
}

#[test]
fn test_replace_visits_priority_by_id() {
    let patch = r##"[{"op": "replace", "path": "/visits/[id=6e57fcd4]/priority", "value": "5"}]"##;
    let mut model_input: JsonValue =
        serde_json::from_str(include_str!("fixtures/model_patch/VISIT_GROUP_input.json")).unwrap();
    let number_of_visits = model_input["visits"].as_array().unwrap().len();
    JsonPatch::parse(patch, JsonPatchMode::Model)
        .unwrap()
        .apply(&mut model_input)
        .unwrap();
    let model_input_patched = &model_input;
    let number_of_visits_after_patch = model_input_patched["visits"].as_array().unwrap().len();
    assert_eq!(number_of_visits, number_of_visits_after_patch);
    let visit = (model_input_patched["visits"][0]).clone();
    assert_eq!("5", text(&visit["priority"]));
}

#[test]
fn test_replace_visits_location_by_id() {
    let patch = r##"[{"op": "replace", "path": "/visits/[id=6e57fcd4]/location", "value": [50.68906251585709, -90.44268080179887]}]"##;
    let mut model_input: JsonValue =
        serde_json::from_str(include_str!("fixtures/model_patch/VISIT_GROUP_input.json")).unwrap();
    let number_of_visits = model_input["visits"].as_array().unwrap().len();
    JsonPatch::parse(patch, JsonPatchMode::Model)
        .unwrap()
        .apply(&mut model_input)
        .unwrap();
    let model_input_patched = &model_input;
    let number_of_visits_after_patch = model_input_patched["visits"].as_array().unwrap().len();
    assert_eq!(number_of_visits, number_of_visits_after_patch);
    let visit = (model_input_patched["visits"][0]).clone();
    assert_eq!("50.68906251585709", text(&visit["location"][0]));
    assert_eq!("-90.44268080179887", text(&visit["location"][1]));
}

#[test]
fn test_replace_visits_pinned_resource_by_priority() {
    let patch =
        r##"[{"op": "replace", "path": "/visits/[priority=6]/pinningRequested", "value": true}]"##;
    let mut model_input: JsonValue =
        serde_json::from_str(include_str!("fixtures/model_patch/VISIT_GROUP_input.json")).unwrap();
    let number_of_visits = model_input["visits"].as_array().unwrap().len();
    JsonPatch::parse(patch, JsonPatchMode::Model)
        .unwrap()
        .apply(&mut model_input)
        .unwrap();
    let model_input_patched = &model_input;
    let number_of_visits_after_patch = model_input_patched["visits"].as_array().unwrap().len();
    assert_eq!(number_of_visits, number_of_visits_after_patch);
    let pinned_requested_for_priority6: Vec<_> = model_input_patched["visits"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|item| text(&item["priority"]) == "6")
        .map(|item| item["pinningRequested"].as_bool().unwrap())
        .collect();
    assert_eq!(10, pinned_requested_for_priority6.len());
    assert!(pinned_requested_for_priority6.iter().all(|v| *v));
}

#[test]
fn test_add_visit_group() {
    let patch = r##"[{"op": "add", "path": "/visitGroups/-", "value": {"id": "ce9aa5ex", "alignment": "START", "serviceDurationStrategy": "INDIVIDUAL", "visits": [{"id": "0d7f6e52", "name": "Jay Robinson 1 / 3", "location": [33.80118537771744, -84.40335014628359], "timeWindows": [{"minStartTime": "2025-08-18T08:00:00-04:00", "maxEndTime": "2025-08-18T16:00:00-04:00"}], "serviceDuration": "PT1H30M", "requiredSkills": [{"name": "plumber", "minLevel": null}], "priority": "8", "pinningRequested": false}]}}]"##;
    let mut model_input: JsonValue =
        serde_json::from_str(include_str!("fixtures/model_patch/VISIT_GROUP_input.json")).unwrap();
    let number_of_visit_groups = model_input["visitGroups"].as_array().unwrap().len();
    JsonPatch::parse(patch, JsonPatchMode::Model)
        .unwrap()
        .apply(&mut model_input)
        .unwrap();
    let model_input_patched = &model_input;
    let number_of_visit_groups_after_patch =
        model_input_patched["visitGroups"].as_array().unwrap().len();
    assert_eq!(
        number_of_visit_groups + 1,
        number_of_visit_groups_after_patch
    );
    let added_visit_group =
        (model_input_patched["visitGroups"][number_of_visit_groups_after_patch - 1]).clone();
    assert_eq!("ce9aa5ex", text(&added_visit_group["id"]));
    assert_eq!(1, added_visit_group["visits"].as_array().unwrap().len());
}

#[test]
fn test_add_visit_to_visit_group() {
    let patch = r##"[{"op": "add", "path": "/visitGroups/[id=ce9aa5e4]/visits/-", "value": {"id": "0d7f6e5x", "name": "Mick Robinson 1 / 3", "location": [33.80118537771744, -84.40335014628359], "timeWindows": [{"minStartTime": "2025-08-18T08:00:00-04:00", "maxEndTime": "2025-08-18T16:00:00-04:00"}], "serviceDuration": "PT1H30M", "requiredSkills": [{"name": "plumber", "minLevel": null}], "priority": "8", "pinningRequested": false}}]"##;
    let mut model_input: JsonValue =
        serde_json::from_str(include_str!("fixtures/model_patch/VISIT_GROUP_input.json")).unwrap();
    let number_of_visit_groups = model_input["visitGroups"].as_array().unwrap().len();
    JsonPatch::parse(patch, JsonPatchMode::Model)
        .unwrap()
        .apply(&mut model_input)
        .unwrap();
    let model_input_patched = &model_input;
    let number_of_visit_groups_after_patch =
        model_input_patched["visitGroups"].as_array().unwrap().len();
    assert_eq!(number_of_visit_groups, number_of_visit_groups_after_patch);
    let visit_group = (model_input_patched["visitGroups"][0]).clone();
    assert_eq!("ce9aa5e4", text(&visit_group["id"]));
    assert_eq!(4, visit_group["visits"].as_array().unwrap().len());
    assert_eq!("0d7f6e5x", text(&visit_group["visits"][3]["id"]));
    assert_eq!(
        "Mick Robinson 1 / 3",
        text(&visit_group["visits"][3]["name"])
    );
}

#[test]
fn test_replace_visit_from_visit_group() {
    let patch = r##"[{"op": "replace", "path": "/visitGroups/[id=ce9aa5e4]/visits/[id=0d7f6e52]", "value": {"id": "0d7f6e5x", "name": "Mick Robinson 1 / 3", "location": [33.80118537771744, -84.40335014628359], "timeWindows": [{"minStartTime": "2025-08-18T08:00:00-04:00", "maxEndTime": "2025-08-18T16:00:00-04:00"}], "serviceDuration": "PT1H30M", "requiredSkills": [{"name": "plumber", "minLevel": null}], "priority": "8", "pinningRequested": false}}]"##;
    let mut model_input: JsonValue =
        serde_json::from_str(include_str!("fixtures/model_patch/VISIT_GROUP_input.json")).unwrap();
    let number_of_visit_groups = model_input["visitGroups"].as_array().unwrap().len();
    JsonPatch::parse(patch, JsonPatchMode::Model)
        .unwrap()
        .apply(&mut model_input)
        .unwrap();
    let model_input_patched = &model_input;
    let number_of_visit_groups_after_patch =
        model_input_patched["visitGroups"].as_array().unwrap().len();
    assert_eq!(number_of_visit_groups, number_of_visit_groups_after_patch);
    let visit_group = (model_input_patched["visitGroups"][0]).clone();
    assert_eq!("ce9aa5e4", text(&visit_group["id"]));
    assert_eq!(3, visit_group["visits"].as_array().unwrap().len());
    assert_eq!("0d7f6e5x", text(&visit_group["visits"][2]["id"]));
    assert_eq!(
        "Mick Robinson 1 / 3",
        text(&visit_group["visits"][2]["name"])
    );
}

#[test]
fn test_remove_visit_from_visit_group() {
    let patch =
        r##"[{"op": "remove", "path": "/visitGroups/[id=ce9aa5e4]/visits/[id=0d7f6e52]"}]"##;
    let mut model_input: JsonValue =
        serde_json::from_str(include_str!("fixtures/model_patch/VISIT_GROUP_input.json")).unwrap();
    let number_of_visit_groups = model_input["visitGroups"].as_array().unwrap().len();
    JsonPatch::parse(patch, JsonPatchMode::Model)
        .unwrap()
        .apply(&mut model_input)
        .unwrap();
    let model_input_patched = &model_input;
    let number_of_visit_groups_after_patch =
        model_input_patched["visitGroups"].as_array().unwrap().len();
    assert_eq!(number_of_visit_groups, number_of_visit_groups_after_patch);
    let visit_group = (model_input_patched["visitGroups"][0]).clone();
    assert_eq!("ce9aa5e4", text(&visit_group["id"]));
    assert_eq!(2, visit_group["visits"].as_array().unwrap().len());
}

#[test]
fn test_remove_skill() {
    let patch = r##"[{"op": "remove", "path": "/skills/[electrician]"}]"##;
    let mut model_input: JsonValue =
        serde_json::from_str(include_str!("fixtures/model_patch/VISIT_GROUP_input.json")).unwrap();
    let number_of_visit_groups = model_input["visitGroups"].as_array().unwrap().len();
    JsonPatch::parse(patch, JsonPatchMode::Model)
        .unwrap()
        .apply(&mut model_input)
        .unwrap();
    let model_input_patched = &model_input;
    let number_of_visit_groups_after_patch =
        model_input_patched["visitGroups"].as_array().unwrap().len();
    assert_eq!(number_of_visit_groups, number_of_visit_groups_after_patch);
    let skills = (model_input_patched["skills"]).clone();
    assert_eq!(3, skills.as_array().unwrap().len());
}

#[test]
fn test_add_skill() {
    let patch = r##"[{"op": "add", "path": "/skills/-", "value": "musician"}]"##;
    let mut model_input: JsonValue =
        serde_json::from_str(include_str!("fixtures/model_patch/VISIT_GROUP_input.json")).unwrap();
    let number_of_visit_groups = model_input["visitGroups"].as_array().unwrap().len();
    JsonPatch::parse(patch, JsonPatchMode::Model)
        .unwrap()
        .apply(&mut model_input)
        .unwrap();
    let model_input_patched = &model_input;
    let number_of_visit_groups_after_patch =
        model_input_patched["visitGroups"].as_array().unwrap().len();
    assert_eq!(number_of_visit_groups, number_of_visit_groups_after_patch);
    let skills = (model_input_patched["skills"]).clone();
    assert_eq!(5, skills.as_array().unwrap().len());
}

#[test]
fn test_add_freeze_departures_before_time() {
    let patch = r##"[{"op": "add", "path": "/freezeDeparturesBeforeTime", "value": "2027-02-01T14:10:00Z"}]"##;
    let mut model_input: JsonValue =
        serde_json::from_str(include_str!("fixtures/model_patch/VISIT_GROUP_input.json")).unwrap();
    JsonPatch::parse(patch, JsonPatchMode::Model)
        .unwrap()
        .apply(&mut model_input)
        .unwrap();
    let model_input_patched = &model_input;
    let freeze_departures_before_time = (model_input_patched["freezeDeparturesBeforeTime"]).clone();
    assert_eq!("2027-02-01T14:10:00Z", text(&freeze_departures_before_time));
    let patch = r##"[{"op": "remove", "path": "/freezeDeparturesBeforeTime"}]"##;
    JsonPatch::parse(patch, JsonPatchMode::Model)
        .unwrap()
        .apply(&mut model_input)
        .unwrap();
    let model_input_patched = &model_input;
    assert!(
        model_input_patched
            .get("freezeDeparturesBeforeTime")
            .is_none()
    );
}
