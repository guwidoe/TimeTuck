//! Original full-field assertions, independent of the native collector's grouping.
#![allow(dead_code)]
use timetuck::*;
pub type Chain = SequenceChain<i64, i64, i64, i128>;
pub fn check(chain: &Chain, runs: &[Vec<(i64, i64, i64)>], step: i128) {
    assert_eq!(chain.sequences().len(), runs.len());
    assert_eq!(chain.breaks().len(), runs.len().saturating_sub(1));
    assert_eq!(chain.first_sequence(), chain.sequences().first());
    assert_eq!(chain.last_sequence(), chain.sequences().last());
    assert_eq!(chain.first_break(), chain.breaks().first());
    assert_eq!(chain.last_break(), chain.breaks().last());
    for (i, (seq, items)) in chain.sequences().iter().zip(runs).enumerate() {
        let actual: Vec<_> = seq
            .items()
            .iter()
            .map(|x| (x.id, x.index, x.value))
            .collect();
        assert_eq!(&actual, items);
        assert_eq!(seq.count(), items.len());
        assert_eq!(
            (seq.first_item().id, seq.last_item().id),
            (items[0].0, items.last().unwrap().0)
        );
        assert_eq!(
            *seq.length(),
            items.last().unwrap().1 as i128 - items[0].1 as i128 + step
        );
        assert_eq!(
            (seq.is_first(), seq.is_last()),
            (i == 0, i + 1 == runs.len())
        );
        assert_eq!(seq.previous_break_index(), i.checked_sub(1));
        assert_eq!(seq.next_break_index(), (i + 1 < runs.len()).then_some(i));
    }
    for (i, b) in chain.breaks().iter().enumerate() {
        let a = runs[i].last().unwrap();
        let c = runs[i + 1][0];
        assert_eq!(
            (
                b.previous_sequence_end().id,
                b.previous_sequence_end().index,
                b.previous_sequence_end().value
            ),
            *a
        );
        assert_eq!(
            (
                b.next_sequence_start().id,
                b.next_sequence_start().index,
                b.next_sequence_start().value
            ),
            c
        );
        assert_eq!(*b.length(), c.1 as i128 - a.1 as i128);
        assert_eq!((b.is_first(), b.is_last()), (i == 0, i + 2 == runs.len()));
    }
}
// Pairwise connectivity reference (not the production adjacent-index scan).
pub fn components(mut items: Vec<(i64, i64, i64)>, step: i128) -> Vec<Vec<(i64, i64, i64)>> {
    items.sort_by_key(|x| (x.1, x.0));
    let n = items.len();
    let mut labels: Vec<_> = (0..n).collect();
    for i in 0..n {
        for j in i + 1..n {
            if items[j].1 as i128 - items[i].1 as i128 <= step {
                let old = labels[j];
                let new = labels[i];
                for x in &mut labels {
                    if *x == old {
                        *x = new;
                    }
                }
            }
        }
    }
    let mut groups = std::collections::BTreeMap::<usize, Vec<_>>::new();
    for (label, item) in labels.into_iter().zip(items) {
        groups.entry(label).or_default().push(item);
    }
    groups.into_values().collect()
}
