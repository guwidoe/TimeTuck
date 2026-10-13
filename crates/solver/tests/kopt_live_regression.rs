//! Original native physical-port controls; no virtual-seam compatibility credit.
use timetuck::*;
fn at(entity: usize, index: usize) -> ListPosition {
    ListPosition { entity, index }
}
#[test]
fn live_interiors_execute_and_undo_for_varied_shapes() {
    for n in 7..15 {
        let p = ListProblem::new_indexed(
            (0..n + 4).collect(),
            vec![
                ListEntity::new((0..n).collect()).with_pinned_prefix(1),
                ListEntity::new((n..n + 4).collect()),
            ],
        )
        .unwrap();
        let before = p.initial_assignment();
        let cuts = [
            (at(0, n - 1), at(0, 1)),
            (at(0, 3), at(0, 4)),
            (at(1, 0), at(1, 1)),
        ];
        let partners = [(1, 3), (2, 5), (4, 0)];
        let bound = ListKOptLivePlan::prepare(&p, &before, &cuts, &partners).unwrap();
        let mut current = before.clone();
        p.apply(
            &mut current,
            &ListMove::Swap {
                left: at(0, 2),
                right: at(0, 5),
            },
        )
        .unwrap()
        .commit();
        for reverse in [false, true] {
            let physical = ListKOptLocalCyclePlan::prepare(&p, &current, &cuts, &partners).unwrap();
            let order = physical.cycle_positions(at(0, 1), reverse).unwrap();
            // Independent uncut-edge certificate: each source local adjacency
            // absent from the declared cuts must occur in the resulting cycle.
            let edge = |a: ListPosition, b: ListPosition| {
                let a = (a.entity, a.index);
                let b = (b.entity, b.index);
                if a < b { (a, b) } else { (b, a) }
            };
            let removed = cuts
                .iter()
                .map(|&(a, b)| edge(a, b))
                .collect::<std::collections::BTreeSet<_>>();
            let output_edges = order
                .iter()
                .zip(order.iter().cycle().skip(1))
                .take(order.len())
                .map(|(&a, &b)| edge(a, b))
                .collect::<std::collections::BTreeSet<_>>();
            for (entity, prefix) in [(0, 1), (1, 0)] {
                let row_len = current.lists()[entity].len();
                for index in prefix..row_len {
                    let next = if index + 1 == row_len {
                        prefix
                    } else {
                        index + 1
                    };
                    let retained = edge(at(entity, index), at(entity, next));
                    if !removed.contains(&retained) {
                        assert!(output_edges.contains(&retained));
                    }
                }
            }
            let payload = order
                .iter()
                .map(|pos| current.lists()[pos.entity][pos.index])
                .collect::<Vec<_>>();
            let movement = bound
                .movement(&p, &current, at(0, 1), reverse, &[2, n + 1])
                .unwrap();
            let saved = current.clone();
            {
                let trial = p.apply(&mut current, &movement).unwrap();
                assert_eq!(&trial.lists()[0][1..], &payload[..2]);
                assert_eq!(trial.lists()[1], payload[2..]);
                assert_eq!(trial.lists()[0][0], 0);
            }
            assert_eq!(current, saved);
        }
    }
}
#[test]
fn endpoint_and_shape_changes_are_rejected_without_edits() {
    let p = ListProblem::new_indexed(
        (0..12).collect(),
        vec![
            ListEntity::new((0..8).collect()).with_pinned_prefix(1),
            ListEntity::new((8..12).collect()),
        ],
    )
    .unwrap();
    let a = p.initial_assignment();
    let cuts = [
        (at(0, 7), at(0, 1)),
        (at(0, 3), at(0, 4)),
        (at(1, 0), at(1, 1)),
    ];
    let plan = ListKOptLivePlan::prepare(&p, &a, &cuts, &[(1, 3), (2, 5), (4, 0)]).unwrap();
    let mut changed = a.clone();
    p.apply(
        &mut changed,
        &ListMove::Swap {
            left: at(0, 1),
            right: at(0, 2),
        },
    )
    .unwrap()
    .commit();
    let saved = changed.clone();
    assert!(
        plan.movement(&p, &changed, at(0, 1), false, &[2, 9])
            .is_err()
    );
    assert_eq!(changed, saved);
    assert!(plan.movement(&p, &a, at(0, 1), false, &[2, 8]).is_err());
    let mut reshaped = a.clone();
    p.apply(
        &mut reshaped,
        &ListMove::Change {
            source: at(0, 2),
            destination: at(1, 4),
        },
    )
    .unwrap()
    .commit();
    let saved = reshaped.clone();
    assert!(
        plan.movement(&p, &reshaped, at(0, 1), false, &[2, 9])
            .is_err()
    );
    assert_eq!(reshaped, saved);
    let pin_changed = ListProblem::new_indexed(
        (0..12).collect(),
        vec![
            ListEntity::new((0..8).collect()).with_pinned_prefix(2),
            ListEntity::new((8..12).collect()),
        ],
    )
    .unwrap();
    assert!(
        plan.movement(&pin_changed, &a, at(0, 1), false, &[2, 9])
            .is_err()
    );
    let fully_pinned = ListProblem::new_indexed(
        (0..12).collect(),
        vec![
            ListEntity::new((0..8).collect()).with_pinned(true),
            ListEntity::new((8..12).collect()),
        ],
    )
    .unwrap();
    assert!(
        plan.movement(&fully_pinned, &a, at(0, 1), false, &[2, 9])
            .is_err()
    );
}
