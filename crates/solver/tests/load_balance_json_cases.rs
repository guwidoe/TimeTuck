// Adapted pinned Community persistence/jackson LoadBalanceRoundTripTest#roundTrip.
// Copyright Timefold BV and contributors. Apache-2.0; see THIRD_PARTY_NOTICES.md
// and compat/licenses/timefold-Apache-2.0.txt. Original controls earn no credit.
use timetuck::*;

#[test]
fn load_balance_round_trip() {
    let mut collector = LoadBalanceCollector::<&str>::default();
    for key in ["A", "B", "C", "A", "A", "B", "A"] {
        collector.insert(&(key, 1, 0));
    }
    let original = collector.finish();
    // Native fixture diagnostics; the JSON must be fed by the actual collector.
    assert_eq!(original.loads()[&"A"], BigInt::from(4));
    assert_eq!(original.loads()[&"B"], BigInt::from(2));
    assert_eq!(original.loads()[&"C"], BigInt::from(1));
    let serialized = load_balance_to_json(&original).unwrap().to_string();
    assert_eq!(serialized, r#"{"unfairness":2.16025}"#);
    let wire: JsonValue = serde_json::from_str(&serialized).unwrap();
    let deserialized = load_balance_from_json(&wire).unwrap();
    assert_eq!(deserialized.unfairness(), original.unfairness());
    assert_eq!(
        deserialized.loads(),
        Err(LoadBalanceJsonError::LoadsUnavailable)
    );
}

#[test]
fn original_exact_numeric_precision_scale_and_malformed_controls() {
    for decimal in ["9007199254740993.0000001", "1.2300", "1e-100", "0"] {
        let json: JsonValue =
            serde_json::from_str(&format!(r#"{{"unfairness":{decimal}}}"#)).unwrap();
        let decoded = load_balance_from_json(&json).unwrap();
        assert_eq!(decoded.unfairness(), &decimal.parse::<Decimal>().unwrap());
        assert_eq!(decoded.loads(), Err(LoadBalanceJsonError::LoadsUnavailable));
    }
    for (text, expected) in [
        ("null", LoadBalanceJsonError::ExpectedObject),
        ("[]", LoadBalanceJsonError::ExpectedObject),
        ("{}", LoadBalanceJsonError::MissingUnfairness),
        (
            r#"{"unfairness":null}"#,
            LoadBalanceJsonError::ExpectedNumericUnfairness,
        ),
        (
            r#"{"unfairness":"2.16025"}"#,
            LoadBalanceJsonError::ExpectedNumericUnfairness,
        ),
        (
            r#"{"unfairness":true}"#,
            LoadBalanceJsonError::ExpectedNumericUnfairness,
        ),
        (
            r#"{"unfairness":1e2147483649}"#,
            LoadBalanceJsonError::InvalidDecimal,
        ),
    ] {
        let json: JsonValue = serde_json::from_str(text).unwrap();
        assert_eq!(load_balance_from_json(&json), Err(expected));
    }
    // Empty input goes through real collector finishing and normal encoding.
    let original = LoadBalanceCollector::<&str>::default().finish();
    let json = load_balance_to_json(&original).unwrap();
    assert_eq!(json.to_string(), r#"{"unfairness":0}"#);
    assert_eq!(
        load_balance_from_json(&json).unwrap().unfairness(),
        &Decimal::ZERO
    );
}
