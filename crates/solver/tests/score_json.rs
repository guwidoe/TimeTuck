//! Original persistence boundary, runtime shape and solver integration checks.
use serde_json::json;
use timetuck::*;
fn check<S: JsonScore>(score: S) {
    let value = score_to_json(Some(&score));
    assert!(value.is_string());
    let wire = value.to_string();
    let read = serde_json::from_str(&wire).unwrap();
    assert_eq!(score_from_json::<S>(&read).unwrap(), Some(score.clone()));
    let envelope = ScoreEnvelope::capture(&score);
    assert_eq!(envelope.format(), S::JSON_FORMAT);
    let read =
        ScoreEnvelope::from_json(&serde_json::from_str(&envelope.to_json().to_string()).unwrap())
            .unwrap();
    assert_eq!(read, envelope);
    let decoded = read.decode::<S>().unwrap();
    assert_eq!(decoded, score);
    assert_eq!(decoded.structural(), score.structural());
    assert_eq!(decoded.to_string(), score.to_string());
    assert_eq!(score_from_json::<S>(&JsonValue::Null).unwrap(), None);
}
#[test]
fn exact_integer_decimal_and_structural_state_survive_every_wire_family() {
    for structural in [i64::MIN, -1, 0, 1, i64::MAX] {
        check(SimpleScore::new(i64::MAX).with_structural(structural));
        check(HardSoftScore::new(i64::MIN, 9007199254740993).with_structural(structural));
        check(HardMediumSoftScore::new(1, i64::MAX, i64::MIN).with_structural(structural));
        check(
            BendableScore::new([i64::MIN, 9007199254740993], [i64::MAX])
                .with_structural(structural),
        );
        check(BendableScore::<0, 0>::ZERO.with_structural(structural));
        let d = || {
            "123456789012345678901234567890.0012300"
                .parse::<Decimal>()
                .unwrap()
        };
        check(SimpleDecimalScore::new(d()).with_structural(structural));
        check(
            HardSoftDecimalScore::new(d(), "-0.0000001".parse().unwrap())
                .with_structural(structural),
        );
        check(
            HardMediumSoftDecimalScore::new(
                d(),
                "2E+20".parse().unwrap(),
                "-3.400".parse().unwrap(),
            )
            .with_structural(structural),
        );
        check(
            BendableDecimalScore::new([d(), "0.000".parse().unwrap()], ["1E+30".parse().unwrap()])
                .with_structural(structural),
        );
        check(BendableDecimalScore::<0, 0>::ZERO.with_structural(structural));
    }
}
#[test]
fn malformed_types_shapes_and_payloads_are_rejected_at_the_boundary() {
    for value in [json!(0), json!(true), json!([]), json!({})] {
        assert_eq!(
            score_from_json::<SimpleScore>(&value),
            Err(ScoreJsonError::ExpectedStringOrNull)
        );
    }
    for value in [
        json!(null),
        json!({"score":"1"}),
        json!({"type":"simple"}),
        json!({"type":"unknown","score":"1"}),
        json!({"type":"simple","score":"1","typo":true}),
        json!({"type":"hard_soft","score":"1"}),
        json!({"type":"simple","score":1}),
        json!({"type":"simple","score":"1","hard_levels":1}),
        json!({"type":"bendable","score":"[1]hard/[2]soft"}),
        json!({"type":"bendable","score":"[1]hard/[2]soft","hard_levels":-1,"soft_levels":1}),
        json!({"type":"bendable","score":"[1]hard/[2]soft","hard_levels":18446744073709551615_u64,"soft_levels":1}),
        json!({"type":"bendable_decimal","score":"[NaN]hard/[2]soft","hard_levels":1,"soft_levels":1}),
    ] {
        assert!(ScoreEnvelope::from_json(&value).is_err(), "{value}");
    }
    let envelope = ScoreEnvelope::capture(&BendableScore::new([1, 2], [3]));
    assert!(matches!(
        envelope.decode::<BendableScore<1, 2>>(),
        Err(ScoreJsonError::FormatMismatch { .. })
    ));
    assert!(matches!(
        envelope.decode::<BendableDecimalScore<2, 1>>(),
        Err(ScoreJsonError::FormatMismatch { .. })
    ));
    // Runtime validation and typed decoding accept/reject the same bendable grammar.
    for text in [
        "[]hard/[]soft",
        "[1/2]hard/[3]soft",
        "1structural/[1/2]hard/[3]soft",
        "[1//2]hard/[3]soft",
        "[1/2]hard/[3/]soft",
        "[1/2]hard/3soft",
        "[1.0/2]hard/[3]soft",
    ] {
        let value = json!({"type":"bendable","score":text,"hard_levels":2,"soft_levels":1});
        assert_eq!(
            ScoreEnvelope::from_json(&value).is_ok(),
            text.parse::<BendableScore<2, 1>>().is_ok()
        );
    }
}
#[test]
fn decoded_weights_bind_real_constraints_and_preserve_explicit_zero() {
    let original = json!({"reward":"3","disabled":"0"});
    let weights = weight_overrides_from_json::<SimpleScore>(&original).unwrap();
    assert_eq!(weight_overrides_to_json(&weights), original);
    let p = FiniteProblem::new(vec![Variable::new(vec![1_i64, 2, 3])]).unwrap();
    let graph = || {
        let mut b = StreamGraphBuilder::new();
        let source = b.source::<usize, (usize, Option<i64>)>();
        b.score_weighted("reward", source.port(), SimpleScore::new(1), |(_, v)| {
            v.unwrap_or(0)
        })
        .unwrap();
        b.score_weighted("disabled", source.port(), SimpleScore::new(100), |_| {
            panic!("disabled callback")
        })
        .unwrap();
        Ok((b.build_with_weights(&weights).unwrap(), source))
    };
    let result = solve_stream(&p, graph, SolverConfig::default()).unwrap();
    assert_eq!(result.assignment.values(), [Some(3)]);
    assert_eq!(result.score, Some(SimpleScore::new(9)));
    for bad in [
        json!({"good":"2","bad":null}),
        json!({"good":"2","bad":3}),
        json!({"bad":"two"}),
    ] {
        assert_eq!(
            weight_overrides_from_json::<SimpleScore>(&bad),
            Err(ScoreJsonError::InvalidWeight {
                constraint: "bad".into()
            })
        );
        assert_eq!(weight_overrides_to_json(&weights), original);
    }
}
