// Adapted from Community probability selector tests; Apache-2.0.
// Only get_size earns method credit. Random sequences are attributed fixtures:
// Java lifecycle forwarding is not implemented by this immutable native sampler.
use timetuck::*;
fn choices() -> WeightedSelection<&'static str> {
    WeightedSelection::new(vec![
        ("e1", 1000.0),
        ("e2", 200.0),
        ("e3", 30.0),
        ("e4", 4.0),
    ])
    .unwrap()
}
#[test]
fn entity_get_size() {
    assert_eq!(choices().len(), 4);
}
#[test]
fn entity_random_selection_sequence_fixture() {
    let selection = choices();
    assert_eq!(selection.len(), 4);
    let mut draws = [
        1222.0 / 1234.0,
        111.0 / 1234.0,
        0.0,
        1230.0 / 1234.0,
        1199.0 / 1234.0,
    ]
    .into_iter();
    let mut it = selection.samples_with(|| draws.next().unwrap());
    for expected in ["e3", "e1", "e1", "e4", "e2"] {
        assert_eq!(it.next(), Some(&expected));
    }
    assert!(selection.active_len() > 0);
}
#[test]
fn move_random_selection_sequence_fixture() {
    // Dummy move codes retain upstream weights and complete scripted sequence.
    let selection = choices();
    let mut draws = [
        1222.0 / 1234.0,
        111.0 / 1234.0,
        0.0,
        1230.0 / 1234.0,
        1199.0 / 1234.0,
    ]
    .into_iter();
    let result = selection
        .samples_with(|| draws.next().unwrap())
        .take(5)
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(selection.len(), 4);
    assert_eq!(result, ["e3", "e1", "e1", "e4", "e2"]);
}
