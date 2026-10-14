//! Original executable binding controls; no additional upstream-method credit.
#[path = "fixtures/model_config_fixture.rs"]
mod fixture;
use fixture::*;
use serde_json::{Value, json};
use std::{cell::Cell, time::Duration};
use timetuck::*;

#[test]
fn live_members_round_trip_scalars_arrays_objects_and_generated_nested_schema() {
    let binding = default_binding();
    let input = json!({"constraintWeight":9,"primitive":42,"primitiveArray":[-3,4],
        "duration":"PT1.25S","string":"quoted \"999999999999999999999999999\"", "stringList":["a","b"],
        "stringArray":["c"],"object":{"id":7,"value":"d"},
        "objectList":[{"id":8,"value":"e"}],"objectArray":[{"id":9,"value":"f"}]}).to_string();
    let mut record = binding.decode(&input).unwrap();
    assert_eq!(record.primitive_array, [-3, 4]);
    assert_eq!(record.duration.nanoseconds(), 1_250_000_000);
    assert_eq!(record.object.as_ref().unwrap().id, 7);
    assert_eq!(record.object_list[0].value, "e");
    assert_eq!(record.object_array[0].id, 9);
    assert_eq!(record.string_list, ["a", "b"]);
    assert_eq!(record.string_array, ["c"]);
    record.constraint_weight = 11;
    record.object_array.push(ObjectParameter {
        id: 10,
        value: "g".into(),
    });
    let encoded = binding.encode(&record).unwrap();
    assert_eq!(binding.decode(&encoded).unwrap(), record);
    assert_eq!(
        serde_json::from_str::<Value>(&encoded).unwrap()["constraintWeight"],
        11
    );
    let schema: Value = serde_json::from_str(&binding.schema_json().unwrap()).unwrap();
    let defs = &schema["$defs"];
    assert_eq!(defs["Duration"]["format"], "duration");
    assert_eq!(defs["ObjectParameter"]["required"], json!(["id", "value"]));
    let fields = &defs[CONFIG_TYPE]["properties"];
    assert_eq!(
        fields["objectList"]["items"]["$ref"],
        "#/$defs/ObjectParameter"
    );
    assert_eq!(
        fields["objectArray"]["items"]["$ref"],
        "#/$defs/ObjectParameter"
    );
    assert_eq!(fields["primitiveArray"]["items"]["type"], "integer");
    assert_eq!(fields["duration"]["$ref"], "#/$defs/Duration");
    assert_eq!(binding.decode("{}").unwrap(), DefaultOverrides::default());
}

#[test]
fn aliases_defaults_nulls_decimal_numbers_and_fixed_length_storage_execute() {
    let binding = schema_binding();
    let mut record = binding.decode(r#"{"primitiveParam":4,"primitiveArrayParam":[1,2],"bigDecimalConstraintWeight":1.25,"string":"before"}"#).unwrap();
    assert_eq!(record.primitive, 4);
    assert_eq!(record.primitive_array, [1, 2]);
    assert_eq!(
        record
            .big_decimal_constraint_weight
            .numeric_cmp(&Decimal::new(125.into(), 2)),
        std::cmp::Ordering::Equal
    );
    binding
        .apply_json(&mut record, r#"{"string":null,"primitiveParam":8}"#)
        .unwrap();
    assert_eq!(record.string, None);
    assert_eq!(record.primitive, 8);
    assert_eq!(record.constraint_weight, DEFAULT_WEIGHT_ONE);
    let encoded: Value = serde_json::from_str(&binding.encode(&record).unwrap()).unwrap();
    assert!(encoded.get("primitive").is_none());
    assert_eq!(encoded["primitiveParam"], 8);
    assert!(binding.decode(r#"{"primitive":2}"#).is_err());
    #[derive(Debug, Eq, PartialEq)]
    struct Fixed([i32; 2]);
    let mut b = ModelConfigBinding::new("Fixed", || Fixed([0; 2])).unwrap();
    b.member(
        "values",
        Default::default(),
        |t: &Fixed| &t.0,
        |t, v| t.0 = v,
    )
    .unwrap();
    assert_eq!(b.decode(r#"{"values":[7,9]}"#).unwrap(), Fixed([7, 9]));
    assert!(b.decode(r#"{"values":[7]}"#).is_err());
    let schema: Value = serde_json::from_str(&b.schema_json().unwrap()).unwrap();
    assert_eq!(
        schema["$defs"]["Fixed"]["properties"]["values"]["minItems"],
        2
    );
    assert_eq!(
        schema["$defs"]["Fixed"]["properties"]["values"]["maxItems"],
        2
    );
}

#[test]
fn invalid_late_values_never_construct_or_write_and_updates_are_atomic() {
    let creates = Cell::new(0);
    let writes = Cell::new(0);
    let mut binding = ModelConfigBinding::new("Atomic", || {
        creates.set(creates.get() + 1);
        DefaultOverrides::default()
    })
    .unwrap();
    binding
        .member(
            "weight",
            Default::default(),
            |t: &DefaultOverrides| &t.constraint_weight,
            |t, v| {
                writes.set(writes.get() + 1);
                t.constraint_weight = v;
            },
        )
        .unwrap();
    binding
        .member(
            "array",
            Default::default(),
            |t: &DefaultOverrides| &t.primitive_array,
            |t, v| {
                writes.set(writes.get() + 1);
                t.primitive_array = v;
            },
        )
        .unwrap();
    binding
        .member(
            "object",
            Default::default(),
            |t: &DefaultOverrides| &t.object,
            |t, v| {
                writes.set(writes.get() + 1);
                t.object = v;
            },
        )
        .unwrap();
    for input in [
        r#"{"weight":99,"array":[1,2147483648]}"#,
        r#"{"weight":99,"object":{"id":1,"value":2}}"#,
        r#"{"weight":99,"object":{"id":1,"value":"x","extra":0}}"#,
        r#"{"weight":null}"#,
        r#"{"weight":99,"unknown":1}"#,
        "[]",
        "{",
        r#"{"weight":99,"array":[1.00000000000000000001]}"#,
    ] {
        let mut old = DefaultOverrides::default();
        let before = old.clone();
        assert!(binding.decode(input).is_err(), "{input}");
        assert!(binding.apply_json(&mut old, input).is_err(), "{input}");
        assert_eq!(old, before);
        assert_eq!(creates.get(), 0);
        assert_eq!(writes.get(), 0);
    }
    let mut old = DefaultOverrides::default();
    binding
        .apply_json(&mut old, r#"{"weight":99,"array":[1,2]}"#)
        .unwrap();
    assert_eq!(old.constraint_weight, 99);
    assert_eq!(old.primitive_array, [1, 2]);
    assert_eq!(creates.get(), 0);
    assert_eq!(writes.get(), 2);
}

#[test]
fn registration_errors_leave_prior_descriptor_and_binding_unchanged() {
    let reads = Cell::new(0);
    let creates = Cell::new(0);
    let mut b = ModelConfigBinding::new("Typed", || {
        creates.set(creates.get() + 1);
        DefaultOverrides::default()
    })
    .unwrap();
    b.member(
        "primitive",
        Default::default(),
        |t: &DefaultOverrides| {
            reads.set(reads.get() + 1);
            &t.primitive
        },
        |t, v| t.primitive = v,
    )
    .unwrap();
    let before = b.descriptor();
    for (name, options) in [
        ("primitive", ModelConfigMemberOptions::default()),
        (
            "other",
            ModelConfigMemberOptions {
                id: Some("primitive".into()),
                ..Default::default()
            },
        ),
        (" ", ModelConfigMemberOptions::default()),
        (
            "nullable",
            ModelConfigMemberOptions {
                nullable: Some(true),
                ..Default::default()
            },
        ),
        (
            "duration",
            ModelConfigMemberOptions {
                format: Some(ModelConfigDataFormat::Duration),
                ..Default::default()
            },
        ),
        (
            "reference",
            ModelConfigMemberOptions {
                constraint_name_ref: Some("".into()),
                ..Default::default()
            },
        ),
    ] {
        assert!(
            b.member(
                name,
                options,
                |t: &DefaultOverrides| &t.primitive,
                |t, v| t.primitive = v
            )
            .is_err()
        );
        assert_eq!(b.descriptor(), before);
    }
    assert_eq!(reads.get(), 0);
    assert_eq!(creates.get(), 0);
    assert_eq!(b.decode(r#"{"primitive":7}"#).unwrap().primitive, 7);
    assert_eq!(creates.get(), 1);
}

fn score_weight(_: &ModelConfigParameter, value: &Value) -> Result<SimpleScore, String> {
    let n = value.as_i64().ok_or("integer weight required")?;
    if n < 0 {
        return Err("native policy requires nonnegative conflict weight".into());
    }
    Ok(SimpleScore::new(n))
}
fn conflicts(values: &[Option<&String>]) -> i64 {
    let mut count = 0;
    for (i, a) in values.iter().enumerate() {
        if a.is_some() {
            for (j, b) in values.iter().enumerate() {
                if i != j && a == b {
                    count -= 1;
                }
            }
        }
    }
    count
}
fn real_solve(
    weight: &ConstraintWeightOverrides<SimpleScore>,
    target: &str,
    steps: u64,
    time: Option<Duration>,
    stream: bool,
) -> SolverResult<Assignment<String>, SimpleScore> {
    let problem = FiniteProblem::new(vec![
        Variable::new(vec!["v1".into(), "v2".into()])
            .with_initial("v1".into());
        2
    ])
    .unwrap();
    let plan = SolverSequence {
        phases: vec![SolverPhase::LocalSearch(LocalSearchConfig {
            max_steps: steps,
            max_evaluations: 64,
            max_time: time,
            ..Default::default()
        })],
        ..Default::default()
    };
    if stream {
        solve_stream(
            &problem,
            || {
                let mut b = StreamGraphBuilder::new();
                let s = b.source::<usize, (usize, Option<String>)>();
                let pairs =
                    b.join_equal(s.port(), s.port(), |(_, v)| v.clone(), |(_, v)| v.clone())?;
                let pairs = b.filter(&pairs, |((i, a), (j, _))| i != j && a.is_some())?;
                b.score_weighted(target, &pairs, SimpleScore::new(1), |_| -1)?;
                Ok((b.build_with_weights(weight)?, s))
            },
            plan,
        )
        .unwrap()
    } else {
        let mut n = ConstraintNetwork::new(2);
        n.add_weighted_constraint(target, vec![0, 1], SimpleScore::new(1), conflicts)
            .unwrap();
        n.apply_weight_overrides(weight).unwrap();
        solve(&problem, &n, plan).unwrap()
    }
}
#[test]
fn references_and_parameter_setters_change_actual_dense_and_stream_solves() {
    let binding = reference_binding();
    for stream in [false, true] {
        for (weight, steps, expected) in [(0, 8, 0), (3, 0, -6), (3, 8, 0)] {
            let record = binding
                .decode(&json!({"constraintWeightByDefault":weight,"parameter":steps}).to_string())
                .unwrap();
            let weights = binding
                .constraint_weights(&record, &["constraintWeightByDefault"], score_weight)
                .unwrap();
            assert_eq!(weights.get(CONFLICTS), Some(&SimpleScore::new(weight)));
            let output = real_solve(
                &weights,
                CONFLICTS,
                u64::try_from(record.parameter).unwrap(),
                None,
                stream,
            );
            assert_eq!(output.score, Some(SimpleScore::new(expected)));
            assert!(output.complete);
            let different = output.assignment.values()[0] != output.assignment.values()[1];
            assert_eq!(different, weight > 0 && steps > 0);
            let independent = conflicts(
                &output
                    .assignment
                    .values()
                    .iter()
                    .map(Option::as_ref)
                    .collect::<Vec<_>>(),
            ) * weight;
            assert_eq!(output.score, Some(SimpleScore::new(independent)));
        }
    }
    // Both reference weights remain selectable independently and read current members.
    let record = ReferenceOverrides {
        constraint_weight_by_annotation: 5,
        ..Default::default()
    };
    let weights = binding
        .constraint_weights(&record, &["constraintWeightByAnnotation"], score_weight)
        .unwrap();
    assert_eq!(weights.get(CONFLICTS), Some(&SimpleScore::new(5)));
}

#[test]
fn duration_and_renamed_parameter_control_actual_phase_execution() {
    let binding = schema_binding();
    for stream in [false, true] {
        for (duration, expected) in [("PT0S", -6), ("PT1H", 0)] {
            let record = binding
                .decode(
                    &json!({"constraintWeight":3,"primitiveParam":8,"duration":duration})
                        .to_string(),
                )
                .unwrap();
            let weights = binding
                .constraint_weights(&record, &["constraintWeight"], score_weight)
                .unwrap();
            let output = real_solve(
                &weights,
                "constraintWeight",
                record.primitive.try_into().unwrap(),
                Some(record.duration.to_std().unwrap()),
                stream,
            );
            assert_eq!(output.score, Some(SimpleScore::new(expected)));
        }
    }
}

#[test]
fn ambiguous_or_invalid_weight_selections_and_semantic_inputs_stop_before_solver() {
    let binding = reference_binding();
    let record = ReferenceOverrides::default();
    let calls = Cell::new(0);
    for ids in [
        vec!["constraintWeightByDefault", "constraintWeightByAnnotation"],
        vec!["parameter"],
        vec!["unknown"],
        vec!["constraintWeightByDefault", "constraintWeightByDefault"],
        vec!["constraintWeightByDefault", "parameter"],
    ] {
        let result = binding.constraint_weights(&record, &ids, |_, _| {
            calls.set(calls.get() + 1);
            Ok(SimpleScore::ZERO)
        });
        assert!(result.is_err());
        assert_eq!(calls.get(), 0);
    }
    let run = |input: &str| -> Result<(), String> {
        let record = binding.decode(input).map_err(|e| e.to_string())?;
        let steps = u64::try_from(record.parameter).map_err(|_| "negative step limit")?;
        let weights = binding
            .constraint_weights(&record, &["constraintWeightByDefault"], score_weight)
            .map_err(|e| e.to_string())?;
        calls.set(calls.get() + 1);
        real_solve(&weights, CONFLICTS, steps, None, true);
        Ok(())
    };
    for input in [
        r#"{"constraintWeightByDefault":-1}"#,
        r#"{"parameter":-1}"#,
        r#"{"parameter":1,"extra":0}"#,
    ] {
        assert!(run(input).is_err());
        assert_eq!(calls.get(), 0);
    }
}

#[test]
fn numeric_precision_is_rejected_instead_of_silently_changing_decimal_weight() {
    let binding = schema_binding();
    for token in [
        "0.10000000000000000001",
        "12345678901234567890123456789",
        "1e2147483648",
    ] {
        assert!(
            binding
                .decode(&format!(r#"{{"bigDecimalConstraintWeight":{token}}}"#))
                .is_err()
        );
    }
    for (token, expected) in [("0.125", "0.25"), ("1.25", "2.5"), ("1e-20", "2e-20")] {
        let record = binding
            .decode(&format!(r#"{{"bigDecimalConstraintWeight":{token}}}"#))
            .unwrap();
        let weights = binding
            .constraint_weights(&record, &["bigDecimalConstraintWeight"], |_, value| {
                value
                    .to_string()
                    .parse::<SimpleDecimalScore>()
                    .map_err(|e| e.to_string())
            })
            .unwrap();
        let mut b = StreamGraphBuilder::new();
        let s = b.source::<usize, ()>();
        b.score_weighted(
            "bigDecimalConstraintWeight",
            s.port(),
            "0".parse().unwrap(),
            |_| 2,
        )
        .unwrap();
        let mut g = b.build_with_weights(&weights).unwrap();
        let mut t = g.transaction();
        t.apply(&s, TupleChange::put(0, ())).unwrap();
        t.commit().unwrap();
        assert_eq!(g.score(), expected.parse::<SimpleDecimalScore>().unwrap());
        g.assert_consistent().unwrap();
    }
    let record = SchemaOverrides {
        big_decimal_constraint_weight: Decimal::new(10000000000000000001u64.into(), 20),
        ..Default::default()
    };
    assert!(binding.encode(&record).is_err());
}
