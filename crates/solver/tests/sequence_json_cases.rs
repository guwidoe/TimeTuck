// Adapted pinned Community SequenceRoundTripTest, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::sync::Arc;
use timetuck::*;

#[derive(Debug, Eq, PartialEq)]
struct Item {
    id: String,
    index: i64,
}
type Chain = SequenceChain<String, i64, Arc<Item>, i128>;
fn codec() -> SequenceJsonCodec<'static, Item> {
    SequenceJsonCodec::new(
        |item: &Item| &item.id,
        |item| Ok(serde_json::json!({ "id": item.id, "index": item.index })),
        |value| {
            let object = value.as_object().ok_or("expected Item object")?;
            if object.keys().any(|name| name != "id" && name != "index") {
                return Err("unknown Item property".into());
            }
            Ok(Item {
                id: object
                    .get("id")
                    .and_then(serde_json::Value::as_str)
                    .ok_or("missing or invalid Item id")?
                    .into(),
                index: object
                    .get("index")
                    .and_then(serde_json::Value::as_i64)
                    .ok_or("missing or invalid integral Item index")?,
            })
        },
    )
}
fn source_chain() -> Chain {
    // Exact source Item identities/indices. Native collector computes every run,
    // boundary flag and neighboring break; the codec has no fixture topology.
    let mut collector = SequenceCollector::<String, i64, Arc<Item>>::default();
    for (id, index) in [
        ("sequence1Item1", 0),
        ("sequence1Item2", 1),
        ("sequence2Item1", 3),
        ("sequence2Item2", 4),
        ("sequence2Item3", 5),
        ("sequence3Item1", 7),
    ] {
        let item = Arc::new(Item {
            id: id.into(),
            index,
        });
        collector.insert(&(item.id.clone(), item.index, item));
    }
    collector.finish()
}
fn assert_serialization(actual: &str, expected: &str) {
    // Source isEqualToIgnoringWhitespace asserts the wire representation,
    // including object-vs-ID references and property order, not just values.
    let compact = |s: &str| {
        s.chars()
            .filter(|ch| !ch.is_whitespace())
            .collect::<String>()
    };
    assert_eq!(compact(actual), compact(expected));
}
fn assert_chain_roundtrip(codec: &SequenceJsonCodec<'_, Item>, original: &Chain, expected: &str) {
    let serialized = codec.encode_chain(original).unwrap();
    assert_serialization(&serialized, expected);
    let deserialized = codec.decode_chain(&serialized).unwrap();
    assert_eq!(deserialized.sequences().len(), original.sequences().len());
    assert!(matches!(
        deserialized.breaks(),
        Err(ReducedChainBreaksUnavailable)
    ));
}
fn assert_sequence_roundtrip(
    codec: &SequenceJsonCodec<'_, Item>,
    chain: &Chain,
    index: usize,
    expected: &str,
) {
    let original = &chain.sequences()[index];
    let serialized = codec.encode_sequence(chain, index).unwrap();
    assert_serialization(&serialized, expected);
    let deserialized = codec.decode_sequence(&serialized).unwrap();
    assert_eq!(deserialized.items().len(), original.items().len());
    assert_eq!(deserialized.count(), original.count());
    if original.is_first() {
        assert!(deserialized.is_first());
        assert!(deserialized.previous_break().is_none());
    } else {
        assert!(!deserialized.is_first());
        assert!(deserialized.previous_break().is_some());
    }
    if original.is_last() {
        assert!(deserialized.is_last());
        assert!(deserialized.next_break().is_none());
    } else {
        assert!(!deserialized.is_last());
        assert!(deserialized.next_break().is_some());
    }
    // Stronger original checks: reference strings restore actual shared Arc
    // identity, and every decoded item keeps its source id/index payload.
    for (actual, expected) in deserialized.items().iter().zip(original.items()) {
        assert_eq!(actual.as_ref(), expected.value.as_ref());
    }
    if let Some(next) = deserialized.next_break() {
        assert!(Arc::ptr_eq(
            next.previous_sequence_end(),
            deserialized.items().last().unwrap()
        ));
    }
    if let Some(previous) = deserialized.previous_break() {
        assert!(Arc::ptr_eq(
            previous.next_sequence_start(),
            &deserialized.items()[0]
        ));
    }
}
fn assert_break_roundtrip(
    codec: &SequenceJsonCodec<'_, Item>,
    chain: &Chain,
    index: usize,
    expected: &str,
) {
    let original = &chain.breaks()[index];
    let serialized = codec.encode_break(chain, index).unwrap();
    assert_serialization(&serialized, expected);
    let deserialized = codec.decode_break(&serialized).unwrap();
    assert_eq!(deserialized.is_first(), original.is_first());
    assert_eq!(deserialized.is_last(), original.is_last());
    // Required nonnull endpoints are represented by actual Arc values, with
    // stronger payload equality instead of a vacuous pointer nonnull check.
    assert_eq!(
        deserialized.previous_sequence_end().as_ref(),
        original.previous_sequence_end().value.as_ref()
    );
    assert_eq!(
        deserialized.next_sequence_start().as_ref(),
        original.next_sequence_start().value.as_ref()
    );
}
#[test]
fn round_trip() {
    let chain = source_chain();
    let codec = codec();
    assert_chain_roundtrip(
        &codec,
        &chain,
        include_str!("fixtures/sequence_json/chain.json"),
    );
    for (index, expected) in [
        include_str!("fixtures/sequence_json/sequence-0.json"),
        include_str!("fixtures/sequence_json/sequence-1.json"),
        include_str!("fixtures/sequence_json/sequence-2.json"),
    ]
    .into_iter()
    .enumerate()
    {
        assert_sequence_roundtrip(&codec, &chain, index, expected);
    }
    for (index, expected) in [
        include_str!("fixtures/sequence_json/break-0.json"),
        include_str!("fixtures/sequence_json/break-1.json"),
    ]
    .into_iter()
    .enumerate()
    {
        assert_break_roundtrip(&codec, &chain, index, expected);
    }
}

#[test]
fn identity_failures_do_not_pollute_later_document_scopes() {
    let chain = source_chain();
    let codec = codec();
    let serialized = codec.encode_sequence(&chain, 1).unwrap();
    let mut dangling: serde_json::Value = serde_json::from_str(&serialized).unwrap();
    dangling["next_break"]["previous_sequence_end"] = "not-declared".into();
    assert!(
        matches!(codec.decode_sequence(&dangling.to_string()), Err(SequenceJsonError::UnknownIdentity(id)) if id == "not-declared")
    );
    let mut conflict: serde_json::Value = serde_json::from_str(&serialized).unwrap();
    conflict["previous_break"]["next_sequence_start"] =
        serde_json::json!({"id":"sequence2Item1","index":99});
    assert!(
        matches!(codec.decode_sequence(&conflict.to_string()), Err(SequenceJsonError::ConflictingIdentity(id)) if id == "sequence2Item1")
    );
    let restored = codec.decode_sequence(&serialized).unwrap();
    assert!(Arc::ptr_eq(
        restored.previous_break().unwrap().next_sequence_start(),
        &restored.items()[0]
    ));
    assert_serialization(&codec.encode_sequence(&chain, 1).unwrap(), &serialized);
}
#[test]
fn empty_native_chain_stays_reduced_and_missing_sequence_is_not_fabricated() {
    let collector = SequenceCollector::<String, i64, Arc<Item>>::default();
    let chain = collector.finish();
    let codec = codec();
    let encoded = codec.encode_chain(&chain).unwrap();
    assert_serialization(&encoded, r#"{"sequences":[]}"#);
    let decoded = codec.decode_chain(&encoded).unwrap();
    assert!(decoded.sequences().is_empty());
    assert!(decoded.breaks().is_err());
    assert!(matches!(
        codec.encode_sequence(&chain, 0),
        Err(SequenceJsonError::IndexOutOfBounds)
    ));
    assert!(matches!(
        codec.encode_break(&chain, 0),
        Err(SequenceJsonError::IndexOutOfBounds)
    ));
}
