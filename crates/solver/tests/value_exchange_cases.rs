// Adapted from Community preview neighborhood example tests.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;

#[test]
fn produces_a_swap_within_the_only_range() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![0, 1, 2]).with_initial(0),
        Variable::new(vec![0, 1, 2]).with_initial(1),
    ])
    .unwrap();
    let a = p.initial_assignment();
    let moves: Vec<_> = value_exchange_moves(
        &p,
        &a,
        ValueExchangeConfig::new(SubPillarSelection::consecutive(2, 2).unwrap()),
    )
    .unwrap()
    .collect();
    assert!(moves.contains(&Move::Change {
        variable: 0,
        value: Some(2)
    }));
    assert!(!moves.contains(&Move::Change {
        variable: 0,
        value: Some(0)
    }));
    assert!(!moves.contains(&Move::Change {
        variable: 1,
        value: Some(1)
    }));
}

#[test]
fn produces_exact_relocation_in_range_and_none_out_of_range() {
    let p = ListProblem::new(
        vec![0, 1, 2],
        vec![
            ListEntity::new(vec![0, 1, 2]).with_value_range(vec![0, 1, 2]),
            ListEntity::new(vec![]).with_value_range(vec![0, 1]),
        ],
    )
    .unwrap();
    let a = p.initial_assignment();
    let moves: Vec<_> = list_sublist_change_moves_with_config(
        &p,
        &a,
        SubListChangeConfig::new(2, 2)
            .unwrap()
            .with_reversing(false),
    )
    .unwrap()
    .collect();
    assert!(moves.contains(&ListMove::SubListChange {
        entity: 0,
        start: 0,
        end: 2,
        destination: ListPosition {
            entity: 1,
            index: 0
        },
        reverse: false,
    }));
    assert!(!moves.contains(&ListMove::SubListChange {
        entity: 0,
        start: 1,
        end: 3,
        destination: ListPosition {
            entity: 1,
            index: 0
        },
        reverse: false,
    }));
}

#[test]
fn produces_exact_swap_at_a_known_cut_and_none_past_shared_length() {
    let p = ListProblem::new(
        (0..6).collect(),
        vec![
            ListEntity::new(vec![0, 2, 4]),
            ListEntity::new(vec![1, 3, 5]),
        ],
    )
    .unwrap();
    let a = p.initial_assignment();
    let moves: Vec<_> = list_tail_swap_moves(&p, &a, false)
        .unwrap()
        .filter(
            |m| matches!(m, ListMove::SubListSwap { left, right, .. } if left.start == right.start),
        )
        .collect();
    let tail_swap = |cut| ListMove::SubListSwap {
        left: ListRange {
            entity: 0,
            start: cut,
            end: 3,
        },
        right: ListRange {
            entity: 1,
            start: cut,
            end: 3,
        },
        reverse: false,
    };
    assert!(moves.contains(&tail_swap(1)));
    assert!(!moves.contains(&tail_swap(3)));
}
