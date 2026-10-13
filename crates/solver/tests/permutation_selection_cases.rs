// Adapted Timefold Community tests (Apache-2.0); see compat/provenance.json.
use std::num::NonZeroU128;
use timetuck::*;
fn order(list: &EntryList<&str>, draws: &[u128], expected: &[&str]) {
    let mut draws = draws.iter().copied();
    let mut iter = list.randomized_entries_with(|_| draws.next().unwrap());
    let actual: Vec<_> = iter.by_ref().map(|e| *e.unwrap().value()).collect();
    assert_eq!(actual, expected);
    assert!(iter.next().is_none());
}
#[test]
fn randomized_iterator() {
    let mut list = EntryList::new();
    let mut iter = list.randomized_entries(0).peekable();
    assert!(iter.peek().is_none());
    assert!(iter.next().is_none());
    drop(iter);
    list.push("A");
    order(&list, &[], &["A"]);
    let b = list.push("B");
    order(&list, &[1], &["B", "A"]);
    order(&list, &[0], &["A", "B"]);
    list.push("C");
    // Semantic swap-table draws preserve all six source permutation outputs.
    order(&list, &[0, 1], &["A", "B", "C"]);
    order(&list, &[0, 0], &["A", "C", "B"]);
    order(&list, &[1, 0], &["B", "A", "C"]);
    order(&list, &[1, 1], &["B", "C", "A"]);
    order(&list, &[2, 0], &["C", "A", "B"]);
    order(&list, &[2, 1], &["C", "B", "A"]);
    list.remove(&b).unwrap();
    order(&list, &[1], &["C", "A"]);
    order(&list, &[0], &["A", "C"]);
}
#[test]
fn test_next_long() {
    let mut random = SampleRandom::new(37);
    let _ = random.wide_index(NonZeroU128::new(10 + i32::MAX as u128).unwrap());
}
#[test]
fn with_method_calls_properly_chain() {
    let config = PillarSwapSelection::default()
        .with_primary(PillarSelectionPolicy::subsets(5, i32::MAX as usize).unwrap())
        .with_probability_weight(1.0)
        .unwrap()
        .with_secondary(PillarSelectionPolicy::subsets(1, 10).unwrap());
    assert_eq!(config.probability_weight(), 1.0);
    assert_eq!(config.primary().sizes().unwrap().0, 5);
    assert_eq!(config.secondary().sizes().unwrap().1, 10);
    // Policies and weight feed real live pillar grouping/move snapshots.
    let p = FiniteProblem::new(
        (0..11)
            .map(|v| Variable::new(vec![0, 1]).with_initial(i32::from(v >= 5)))
            .collect(),
    )
    .unwrap();
    let a = p.initial_assignment();
    let selected = config.select(&p, &a, 10_000).unwrap();
    assert!(!selected.is_empty());
    assert_eq!(selected.active_len(), selected.len());
    for movement in selected.items() {
        let mut b = a.clone();
        drop(p.apply(&mut b, movement).unwrap());
        assert_eq!(b, a);
    }
}
