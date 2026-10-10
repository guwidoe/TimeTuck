// Adapted from pinned Community Jackson score round-trip tests and helper fixtures.
// Apache-2.0; see compat/provenance.json. Native JSON codecs replace Java mapper registration.
use timetuck::*;
fn d(s: &str) -> Decimal {
    s.parse().unwrap()
}
fn serialized(value: JsonValue) -> JsonValue {
    serde_json::from_str(&value.to_string()).unwrap()
}
fn round_trip<S: JsonScore>(expected: Option<S>, text: Option<&str>) {
    let input = serde_json::json!({"score":score_to_json(expected.as_ref())});
    let encoded = input.to_string();
    let output: JsonValue = serde_json::from_str(&encoded).unwrap();
    assert_eq!(score_from_json::<S>(&output["score"]).unwrap(), expected);
    let wire = if let Some(text) = text {
        format!(r#"{{"score":"{text}"}}"#)
    } else {
        r#"{"score":null}"#.into()
    };
    assert_eq!(encoded, wire);
}
#[test]
fn simple_score_round_trip() {
    round_trip::<SimpleScore>(None, None);
    let score = SimpleScore::new(1234);
    round_trip::<SimpleScore>(Some(score), Some("1234"));
}
#[test]
fn simple_score_negative_structural() {
    let score = SimpleScore::new(1234).with_structural(-1);
    round_trip::<SimpleScore>(Some(score), Some("-1structural/1234"));
}
#[test]
fn hard_soft_score_round_trip() {
    round_trip::<HardSoftScore>(None, None);
    let score = HardSoftScore::new(1200, 34);
    round_trip::<HardSoftScore>(Some(score), Some("1200hard/34soft"));
}
#[test]
fn hard_soft_score_negative_structural() {
    let score = HardSoftScore::new(1200, 34).with_structural(-1);
    round_trip::<HardSoftScore>(Some(score), Some("-1structural/1200hard/34soft"));
}
#[test]
fn hard_medium_soft_score_round_trip() {
    round_trip::<HardMediumSoftScore>(None, None);
    let score = HardMediumSoftScore::new(1200, 30, 4);
    round_trip::<HardMediumSoftScore>(Some(score), Some("1200hard/30medium/4soft"));
}
#[test]
fn hard_medium_soft_score_negative_structural() {
    let score = HardMediumSoftScore::new(1200, 30, 4).with_structural(-1);
    round_trip::<HardMediumSoftScore>(Some(score), Some("-1structural/1200hard/30medium/4soft"));
}
#[test]
fn bendable_score_round_trip() {
    round_trip::<BendableScore<2, 1>>(None, None);
    let score = BendableScore::new([1000, 200], [34]);
    round_trip::<BendableScore<2, 1>>(Some(score), Some("[1000/200]hard/[34]soft"));
}
#[test]
fn bendable_score_negative_structural() {
    let score = BendableScore::new([1000, 200], [34]).with_structural(-1);
    round_trip::<BendableScore<2, 1>>(Some(score), Some("-1structural/[1000/200]hard/[34]soft"));
}
#[test]
fn simple_big_decimal_score_round_trip() {
    round_trip::<SimpleDecimalScore>(None, None);
    let score = SimpleDecimalScore::new(d("1234.4321"));
    round_trip::<SimpleDecimalScore>(Some(score), Some("1234.4321"));
}
#[test]
fn simple_big_decimal_score_negative_structural() {
    let score = SimpleDecimalScore::new(d("1234.4321")).with_structural(-1);
    round_trip::<SimpleDecimalScore>(Some(score), Some("-1structural/1234.4321"));
}
#[test]
fn hard_soft_big_decimal_score_round_trip() {
    round_trip::<HardSoftDecimalScore>(None, None);
    let score = HardSoftDecimalScore::new(d("1200.0021"), d("34.4300"));
    round_trip::<HardSoftDecimalScore>(Some(score), Some("1200.0021hard/34.4300soft"));
}
#[test]
fn hard_soft_big_decimal_score_negative_structural() {
    let score = HardSoftDecimalScore::new(d("1200.0021"), d("34.4300")).with_structural(-1);
    round_trip::<HardSoftDecimalScore>(Some(score), Some("-1structural/1200.0021hard/34.4300soft"));
}
#[test]
fn hard_medium_soft_big_decimal_score_round_trip() {
    round_trip::<HardMediumSoftDecimalScore>(None, None);
    let score = HardMediumSoftDecimalScore::new(d("1200.0021"), d("-3.1415"), d("34.4300"));
    round_trip::<HardMediumSoftDecimalScore>(
        Some(score),
        Some("1200.0021hard/-3.1415medium/34.4300soft"),
    );
}
#[test]
fn hard_medium_soft_big_decimal_score_negative_structural() {
    let score = HardMediumSoftDecimalScore::new(d("1200.0021"), d("-3.1415"), d("34.4300"))
        .with_structural(-1);
    round_trip::<HardMediumSoftDecimalScore>(
        Some(score),
        Some("-1structural/1200.0021hard/-3.1415medium/34.4300soft"),
    );
}
#[test]
fn bendable_big_decimal_score_round_trip() {
    round_trip::<BendableDecimalScore<2, 1>>(None, None);
    let score = BendableDecimalScore::new([d("1000.0001"), d("200.0020")], [d("34.4300")]);
    round_trip::<BendableDecimalScore<2, 1>>(
        Some(score),
        Some("[1000.0001/200.0020]hard/[34.4300]soft"),
    );
}
#[test]
fn bendable_big_decimal_score_negative_structural() {
    let score = BendableDecimalScore::new([d("1000.0001"), d("200.0020")], [d("34.4300")])
        .with_structural(-1);
    round_trip::<BendableDecimalScore<2, 1>>(
        Some(score),
        Some("-1structural/[1000.0001/200.0020]hard/[34.4300]soft"),
    );
}

#[test]
fn polymorphic_score() {
    let bendable = BendableScore::new([1000, 200], [34]);
    let hard_soft = HardSoftScore::new(-1, -20);
    let first = ScoreEnvelope::capture(&HardSoftScore::new(-20, -300));
    let second = ScoreEnvelope::capture(&BendableScore::new([-1, -20], [-300, -4000, -50000]));
    for (i, poly) in [first, second].into_iter().enumerate() {
        let output = serialized(serde_json::json!({
            "bendableScore":score_to_json(Some(&bendable)),
            "hardSoftScore":score_to_json(Some(&hard_soft)),
            "polymorphicScore":poly.to_json()
        }));
        assert_eq!(
            score_from_json::<BendableScore<2, 1>>(&output["bendableScore"]).unwrap(),
            Some(bendable)
        );
        assert_eq!(
            score_from_json::<HardSoftScore>(&output["hardSoftScore"]).unwrap(),
            Some(hard_soft)
        );
        let poly = ScoreEnvelope::from_json(&output["polymorphicScore"]).unwrap();
        if i == 0 {
            assert_eq!(
                poly.decode::<HardSoftScore>().unwrap(),
                HardSoftScore::new(-20, -300)
            );
        } else {
            assert_eq!(
                poly.decode::<BendableScore<2, 3>>().unwrap(),
                BendableScore::new([-1, -20], [-300, -4000, -50000])
            );
        }
    }
}
#[test]
fn constraint_weight_overrides() {
    let weights = ConstraintWeightOverrides::from_iter([
        ("constraint1".into(), HardSoftScore::new(1, 0)),
        ("constraint2".into(), HardSoftScore::new(0, 2)),
    ]);
    let encoded = weight_overrides_to_json(&weights).to_string();
    assert_eq!(
        encoded,
        r#"{"constraint1":"1hard/0soft","constraint2":"0hard/2soft"}"#
    );
    let value = serde_json::from_str(&encoded).unwrap();
    assert_eq!(
        weight_overrides_from_json::<HardSoftScore>(&value).unwrap(),
        weights
    );
}
