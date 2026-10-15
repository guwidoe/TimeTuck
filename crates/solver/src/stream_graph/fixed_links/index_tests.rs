//! Original endpoint-index controls; no upstream source-method credit.
use super::*;
struct Payload(i64); // Deliberately not Clone: batch indexing only borrows values.
fn events() -> Vec<(bool, Event)> {
    [
        (false, 2, Some(2)),
        (true, 0, Some(3)),
        (false, 0, Some(4)),
        (false, 2, None),
        (true, 0, None),
        (true, usize::MAX, Some(9)),
        (false, 1, Some(5)),
        (false, usize::MAX, Some(6)),
        (true, 2, Some(7)),
    ]
    .into_iter()
    .map(|(side, id, value)| {
        (
            side,
            Rc::new(TupleChange {
                id,
                value: value.map(Payload),
            }) as Event,
        )
    })
    .collect()
}
fn oracle(
    inputs: &[(bool, Event)],
    side: bool,
    slots: &BTreeMap<usize, usize>,
) -> Vec<(usize, usize)> {
    let mut winners = BTreeMap::new();
    for (index, (input_side, event)) in inputs.iter().enumerate() {
        if *input_side == side {
            winners.insert(typed::<usize, Payload>(event).id, index);
        }
    }
    winners
        .into_iter()
        .filter_map(|(id, index)| slots.get(&id).map(|&slot| (slot, index)))
        .collect()
}
#[test]
fn numeric_paths_match_independent_last_winner_oracle_and_reuse_scratch_after_empty() {
    let input = events();
    let mut scratch = Vec::new();
    for slots in [
        BTreeMap::from([(0usize, 0), (1, 1), (2, 2)]),
        BTreeMap::from([(0, 0), (2, 1), (usize::MAX, 2)]),
        BTreeMap::new(),
    ] {
        let direct = contiguous_slots(&slots);
        for side in [false, true] {
            latest_slots::<usize, Payload>(&input, side, &slots, direct, &mut scratch);
            assert_eq!(scratch, oracle(&input, side, &slots));
            let pointer = scratch.as_ptr();
            let capacity = scratch.capacity();
            latest_slots::<usize, Payload>(&[], side, &slots, direct, &mut scratch);
            assert!(scratch.is_empty());
            latest_slots::<usize, Payload>(&input[..2], side, &slots, direct, &mut scratch);
            assert_eq!(scratch, oracle(&input[..2], side, &slots));
            assert_eq!((scratch.as_ptr(), scratch.capacity()), (pointer, capacity));
        }
    }
    let slots = BTreeMap::from([(0usize, 0), (1, 1), (2, 2)]);
    latest_slots::<usize, Payload>(&input, false, &slots, true, &mut scratch);
    let values: Vec<_> = scratch
        .iter()
        .map(|&(slot, index)| {
            (
                slot,
                typed::<usize, Payload>(&input[index].1)
                    .value
                    .as_ref()
                    .map(|v| v.0),
            )
        })
        .collect();
    assert_eq!(values, [(0, Some(4)), (1, Some(5)), (2, None)]);
    latest_slots::<usize, Payload>(&input, true, &slots, true, &mut scratch);
    assert_eq!(scratch, [(0, 4), (2, 8)]);
}
#[test]
fn only_nonempty_exact_usize_identity_maps_enable_direct_slots() {
    for count in [1usize, 256, 257] {
        let slots: BTreeMap<_, _> = (0..count).map(|id| (id, id)).collect();
        assert!(contiguous_slots(&slots));
    }
    for slots in [
        BTreeMap::<usize, usize>::new(),
        BTreeMap::from([(1, 0)]),
        BTreeMap::from([(0, 0), (2, 1)]),
        BTreeMap::from([(usize::MAX, 0)]),
        BTreeMap::from([(0, 1), (1, 0)]),
        BTreeMap::from([(0, 0), (1, 7)]),
    ] {
        assert!(!contiguous_slots(&slots));
    }
    assert!(!contiguous_slots(&BTreeMap::from([("a", 0), ("z", 1)])));
    #[derive(PartialEq, Eq, PartialOrd, Ord)]
    struct Custom(usize);
    assert!(!contiguous_slots(&BTreeMap::from([
        (Custom(0), 0),
        (Custom(1), 1)
    ])));
    assert!(!contiguous_slots(&BTreeMap::from([(0u32, 0), (1u32, 1)])));
}
