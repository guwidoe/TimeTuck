// Ordered-output fixtures adapted from Timefold Community 2.7.0
// SelectorBasedKOptListMoveTest, Apache-2.0. See compat/provenance.json.
// Java notifications and implicit partition selection are NOT adapted; no credit.
use timetuck::*;
#[test]
fn multi_entity_output_fixtures_with_explicit_partitions() {
    for (sizes, removed, added, successor, lengths, expected) in [
        (
            vec![4, 4],
            vec![(2, 3), (6, 7)],
            vec![(2, 6), (3, 7)],
            2,
            vec![5, 3],
            vec![vec![1, 2, 6, 5, 4], vec![3, 7, 8]],
        ),
        (
            vec![4, 4, 4],
            vec![(2, 3), (7, 8), (9, 10), (11, 12)],
            vec![(11, 3), (12, 7), (2, 9), (10, 8)],
            2,
            vec![4, 4, 4],
            vec![vec![1, 2, 9, 8], vec![10, 11, 3, 4], vec![5, 6, 7, 12]],
        ),
    ] {
        let n: usize = sizes.iter().sum();
        let mut at = 1;
        let entities = sizes
            .iter()
            .map(|&len| {
                let row = (at..at + len).collect();
                at += len;
                ListEntity::new(row)
            })
            .collect();
        let p = ListProblem::new((1..=n).collect(), entities).unwrap();
        let before = p.initial_assignment();
        let mut a = before.clone();
        let removed: Vec<_> = removed.into_iter().map(|(a, b)| (a - 1, b - 1)).collect();
        let added: Vec<_> = added.into_iter().map(|(a, b)| (a - 1, b - 1)).collect();
        let movement = p
            .k_opt_repartition_move(
                &a,
                &(0..sizes.len()).collect::<Vec<_>>(),
                &removed,
                &added,
                (0, successor - 1),
                &lengths,
            )
            .unwrap();
        {
            let trial = p.apply(&mut a, &movement).unwrap();
            assert_eq!(trial.lists(), expected);
        }
        assert_eq!(a, before);
    }
}
