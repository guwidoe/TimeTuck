//! Original JSON patch atomicity, pointer and selector regression cases.
use serde_json::json;
use timetuck::*;
fn patch(value: JsonValue, mode: JsonPatchMode) -> JsonPatch {
    JsonPatch::from_value(&value, mode).unwrap()
}
#[test]
fn strict_operations_escaped_paths_and_numeric_tests() {
    let mut input = json!({"a/b": {"~key": ["red", "green", "blue"]}, "n": 1});
    let p = patch(
        json!([
            {"op":"test","path":"/n","value":1.0},
            {"op":"add","path":"/a~1b/~0key/1","value":"gold"},
            {"op":"copy","from":"/a~1b/~0key/0","path":"/copy"},
            {"op":"move","from":"/a~1b/~0key/3","path":"/a~1b/~0key/0"},
            {"op":"replace","path":"/n","value":2},
            {"op":"remove","path":"/a~1b/~0key/2"}
        ]),
        JsonPatchMode::Strict,
    );
    p.apply(&mut input).unwrap();
    assert_eq!(
        input,
        json!({"a/b":{"~key":["blue","red","green"]},"copy":"red","n":2})
    );
    for (a, b, same) in [
        (
            json!(9007199254740993_u64),
            json!(9007199254740992.0),
            false,
        ),
        (
            json!(18446744073709551615_u64),
            json!(18446744073709551616.0),
            false,
        ),
        (
            json!(-9223372036854775808_i64),
            json!(-9223372036854775808.0),
            true,
        ),
        (json!([1,{"n":2}]), json!([1.0,{"n":2.0}]), true),
    ] {
        let mut a = a;
        assert_eq!(
            patch(
                json!([{"op":"test","path":"","value":b}]),
                JsonPatchMode::Strict
            )
            .apply(&mut a)
            .is_ok(),
            same
        );
    }
    let mut root = json!([1, 2]);
    patch(
        json!([{"op":"replace","path":"","value":{"x":3}}]),
        JsonPatchMode::Strict,
    )
    .apply(&mut root)
    .unwrap();
    assert_eq!(root, json!({"x":3}));
}
#[test]
fn failure_rolls_back_every_edit_including_root_and_moves() {
    let original = json!({"rows":[{"id":"a"},{"id":"b"},{"id":"c"}],"keep":"unchanged"});
    let mutations = json!([
        {"op":"add","path":"/new","value":null},
        {"op":"replace","path":"/keep","value":"changed"},
        {"op":"remove","path":"/rows/1"},
        {"op":"add","path":"/rows/0","value":{"id":"d"}},
        {"op":"move","from":"/rows/2","path":"/moved"},
        {"op":"copy","from":"/rows/0","path":"/rows/-"},
        {"op":"replace","path":"","value":{"finished":true}}
    ]);
    for count in 0..=mutations.as_array().unwrap().len() {
        let mut ops = mutations.as_array().unwrap()[..count].to_vec();
        ops.push(json!({"op":"test","path":"/missing","value":42}));
        let mut input = original.clone();
        let error = patch(ops.into(), JsonPatchMode::Strict)
            .apply(&mut input)
            .unwrap_err();
        assert_eq!(error.operation, count);
        assert_eq!(input, original);
    }
    for (from, to) in [
        ("/rows", "/rows/0/child"),
        ("", "/nested"),
        ("/rows/0", "/rows/8"),
    ] {
        let mut input = original.clone();
        assert!(
            patch(
                json!([{"op":"move","from":from,"path":to}]),
                JsonPatchMode::Strict
            )
            .apply(&mut input)
            .is_err()
        );
        assert_eq!(input, original);
    }
    let mut input = original.clone();
    patch(
        json!([{"op":"move","from":"/rows/1","path":"/rows/1"}]),
        JsonPatchMode::Strict,
    )
    .apply(&mut input)
    .unwrap();
    assert_eq!(input, original);
}
#[test]
fn model_fanout_nested_selectors_creation_order_and_rollback() {
    let original = json!({"people":[
        {"id":"a","skills":[{"name":"medic"}]},
        {"id":"b","skills":[{"name":"driver"}]},
        {"id":"c","skills":[{"name":"medic"}]}
    ],"words":["a","b","a"]});
    let ops = json!([
        {"op":"add","path":"/people/[skills/name=medic]/tags/-","value":"night"},
        {"op":"remove","path":"/words/[a]"},
        {"op":"replace","path":"/people/[id=a]","value":{"id":"d"}}
    ]);
    let p = patch(ops.clone(), JsonPatchMode::Model);
    let result = p.applied(&original).unwrap();
    assert_eq!(
        result,
        json!({"people":[
        {"id":"b","skills":[{"name":"driver"}]},
        {"id":"c","skills":[{"name":"medic"}],"tags":["night"]},
        {"id":"d"}
    ],"words":["b"]})
    );
    let mut ops = ops.as_array().unwrap().clone();
    ops.push(json!({"op":"test","path":"/people/[id=d]","value":false}));
    let mut input = original.clone();
    assert!(
        patch(ops.into(), JsonPatchMode::Model)
            .apply(&mut input)
            .is_err()
    );
    assert_eq!(input, original);
    patch(
        json!([{"op":"remove","path":"/people/[id=absent]/skills/0"}]),
        JsonPatchMode::Model,
    )
    .apply(&mut input)
    .unwrap();
    assert_eq!(input, original);
}
#[test]
fn invalid_patches_and_strict_paths_are_rejected_without_modification() {
    for value in [
        json!({}),
        json!([{}]),
        json!([{"op":"unknown","path":""}]),
        json!([{"op":"add","path":"/x"}]),
        json!([{"op":"remove","path":"x"}]),
        json!([{"op":"remove","path":"/~2"}]),
        json!([{"op":"copy","path":"/x"}]),
    ] {
        assert!(JsonPatch::from_value(&value, JsonPatchMode::Strict).is_err());
    }
    assert!(JsonPatch::parse("[", JsonPatchMode::Strict).is_err());
    assert!(
        JsonPatch::parse(r#"[{"op":"remove","path":"/[id=x"}]"#, JsonPatchMode::Model).is_err()
    );
    let original = json!({"rows":[1,2],"[literal]":7,"":8});
    for path in [
        "/rows/-",
        "/rows/01",
        "/rows/-1",
        "/rows/2",
        "/rows/9999999999999999999999999999",
        "/missing/x",
        "/rows/0/x",
    ] {
        let mut input = original.clone();
        assert!(
            patch(json!([{"op":"remove","path":path}]), JsonPatchMode::Strict)
                .apply(&mut input)
                .is_err()
        );
        assert_eq!(input, original);
    }
    let mut input = original;
    patch(
        json!([{"op":"test","path":"/[literal]","value":7},{"op":"test","path":"/","value":8}]),
        JsonPatchMode::Strict,
    )
    .apply(&mut input)
    .unwrap();
}
#[test]
fn in_place_patch_preserves_unrelated_allocations_and_can_be_reused() {
    let p = patch(
        json!([{"op":"replace","path":"/value","value":3}]),
        JsonPatchMode::Strict,
    );
    assert_eq!(p.len(), 1);
    assert!(!p.is_empty());
    for _ in 0..3 {
        let mut input = json!({"large":"x".repeat(1_000_000),"value":1});
        let ptr = input["large"].as_str().unwrap().as_ptr();
        p.apply(&mut input).unwrap();
        assert_eq!(ptr, input["large"].as_str().unwrap().as_ptr());
        assert_eq!(input["value"], 3);
    }
}

#[test]
fn selector_failures_restore_earlier_created_containers_and_reject_ambiguous_moves() {
    let original = json!({"people":[{"id":"x"},{"id":"x","tags":null}]});
    for ops in [
        json!([{"op":"add","path":"/people/[id=x]/tags/-","value":"night"}]),
        json!([{"op":"copy","from":"/people/[id=x]","path":"/copy"}]),
        json!([{"op":"move","from":"/people/0","path":"/people/[id=x]"}]),
        json!([{"op":"test","path":"/people/[id=absent]","value":null}]),
    ] {
        let mut input = original.clone();
        assert!(patch(ops, JsonPatchMode::Model).apply(&mut input).is_err());
        assert_eq!(input, original);
    }
    let mut input = original.clone();
    let fail = patch(
        json!([
            {"op":"remove","path":""},
            {"op":"test","path":"","value":false}
        ]),
        JsonPatchMode::Strict,
    );
    assert!(fail.apply(&mut input).is_err());
    assert_eq!(input, original);
    patch(json!([{"op":"remove","path":""}]), JsonPatchMode::Strict)
        .apply(&mut input)
        .unwrap();
    assert!(input.is_null());
}
