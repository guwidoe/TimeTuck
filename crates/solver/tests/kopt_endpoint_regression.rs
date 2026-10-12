//! Local-cycle endpoint controls; first seam geometry adapts a pinned Community TEST.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;
fn at(entity: usize, index: usize) -> ListPosition {
    ListPosition { entity, index }
}
#[test]
fn local_wrap_order_is_distinct_from_physical_combined_value_order() {
    let p = ListProblem::new(
        (1..=6).collect(),
        vec![
            ListEntity::new(vec![1, 2, 3, 6]),
            ListEntity::new(vec![4, 5]),
        ],
    )
    .unwrap();
    let a = p.initial_assignment();
    let t = ListKOptEndpointTopology::prepare_local_cycles(
        &p,
        &a,
        &[
            (at(0, 3), at(0, 0)),
            (at(0, 1), at(0, 2)),
            (at(1, 0), at(1, 1)),
        ],
        &[(1, 3), (2, 5), (4, 0)],
    )
    .unwrap();
    assert_eq!(t.endpoint_tour_order(), &[1, 0, 2, 3, 4, 5]);
    assert_eq!(t.endpoint_slot_ranks(), &[1, 0, 2, 3, 4, 5]);
    assert_eq!(t.endpoint_partners(), &[4, 3, 5, 1, 0, 2]);
    assert_eq!(t.endpoint_cycle_count(), 1);
    assert_eq!(
        t.endpoint_cycle_order(0, 4).unwrap(),
        vec![0, 4, 3, 1, 5, 2]
    );
    assert_eq!(a, p.initial_assignment());
}
#[test]
fn repeated_value_occurrences_and_two_parallel_local_edges_retain_distinct_slots() {
    let p = ListProblem::new(vec![1, 2], vec![ListEntity::new(vec![1, 2])]).unwrap();
    let a = p.initial_assignment();
    let t = ListKOptEndpointTopology::prepare_local_cycles(
        &p,
        &a,
        &[(at(0, 0), at(0, 1)), (at(0, 1), at(0, 0))],
        &[(0, 3), (1, 2)],
    )
    .unwrap();
    assert_eq!(
        t.endpoint_positions(),
        &[at(0, 0), at(0, 1), at(0, 1), at(0, 0)]
    );
    assert_eq!(t.endpoint_tour_order(), &[0, 1, 3, 2]);
    assert!(t.endpoint_cycle_count() > 0);
}
#[test]
fn singleton_cycles_and_pinned_prefixes_validate_without_mutation() {
    let p = ListProblem::new(
        vec![1, 2, 3],
        vec![
            ListEntity::new(vec![1, 2]).with_pinned_prefix(1),
            ListEntity::new(vec![3]),
        ],
    )
    .unwrap();
    let a = p.initial_assignment();
    let t = ListKOptEndpointTopology::prepare_local_cycles(
        &p,
        &a,
        &[(at(0, 1), at(0, 1)), (at(1, 0), at(1, 0))],
        &[(0, 2), (1, 3)],
    )
    .unwrap();
    assert_eq!(t.selected_entities(), &[0, 1]);
    assert_eq!(t.endpoint_cycle_count(), 1);
    assert!(
        ListKOptEndpointTopology::prepare_local_cycles(
            &p,
            &a,
            &[(at(0, 0), at(0, 1)), (at(1, 0), at(1, 0))],
            &[(0, 2), (1, 3)]
        )
        .is_err()
    );
    assert_eq!(a, p.initial_assignment());
}
#[test]
fn bad_local_edges_and_nonmatching_added_slots_reject_before_a_proposal() {
    let p = ListProblem::new(
        (0..5).collect(),
        vec![ListEntity::new(vec![0, 1, 2]), ListEntity::new(vec![3, 4])],
    )
    .unwrap();
    let a = p.initial_assignment();
    for cuts in [
        vec![(at(0, 0), at(1, 0)), (at(1, 0), at(1, 1))],
        vec![(at(0, 0), at(0, 1)), (at(0, 0), at(0, 1))],
        vec![(at(0, 0), at(0, 9)), (at(1, 0), at(1, 1))],
    ] {
        assert!(
            ListKOptEndpointTopology::prepare_local_cycles(&p, &a, &cuts, &[(0, 2), (1, 3)])
                .is_err()
        );
    }
    let cuts = [(at(0, 0), at(0, 1)), (at(1, 0), at(1, 1))];
    for added in [
        vec![(0, 0), (1, 3)],
        vec![(0, 2), (0, 3)],
        vec![(0, 9), (1, 3)],
    ] {
        assert!(ListKOptEndpointTopology::prepare_local_cycles(&p, &a, &cuts, &added).is_err());
    }
    assert_eq!(a, p.initial_assignment());
}

fn matchings(slots: &[usize]) -> Vec<Vec<(usize, usize)>> {
    if slots.is_empty() {
        return vec![vec![]];
    }
    let mut out = vec![];
    for i in 1..slots.len() {
        let rest = slots[1..]
            .iter()
            .copied()
            .filter(|x| *x != slots[i])
            .collect::<Vec<_>>();
        for mut tail in matchings(&rest) {
            tail.insert(0, (slots[0], slots[i]));
            out.push(tail);
        }
    }
    out
}
#[test]
fn every_small_slot_matching_agrees_with_independent_component_connectivity() {
    for k in 2..=4 {
        let p = ListProblem::new(
            (0..2 * k).collect(),
            vec![ListEntity::new((0..2 * k).collect())],
        )
        .unwrap();
        let a = p.initial_assignment();
        let cuts = (0..k)
            .map(|i| (at(0, 2 * i), at(0, 2 * i + 1)))
            .collect::<Vec<_>>();
        for added in matchings(&(0..2 * k).collect::<Vec<_>>()) {
            let t = ListKOptEndpointTopology::prepare_local_cycles(&p, &a, &cuts, &added).unwrap();
            let mut adjacency = vec![vec![]; 2 * k];
            for &(x, y) in &added {
                adjacency[x].push(y);
                adjacency[y].push(x);
            }
            for i in 0..k {
                let x = 2 * i + 1;
                let y = (2 * i + 2) % (2 * k);
                adjacency[x].push(y);
                adjacency[y].push(x);
            }
            let mut labels = vec![usize::MAX; 2 * k];
            let mut count = 0;
            for first in 0..2 * k {
                if labels[first] != usize::MAX {
                    continue;
                }
                let mut todo = vec![first];
                labels[first] = count;
                while let Some(x) = todo.pop() {
                    for &y in &adjacency[x] {
                        if labels[y] == usize::MAX {
                            labels[y] = count;
                            todo.push(y);
                        }
                    }
                }
                count += 1;
            }
            assert_eq!(t.endpoint_cycle_count(), count);
            for x in 0..2 * k {
                for y in 0..2 * k {
                    assert_eq!(
                        t.endpoint_cycle_ids()[x] == t.endpoint_cycle_ids()[y],
                        labels[x] == labels[y]
                    );
                }
            }
            let next = t.endpoint_partners()[0];
            if count == 1 {
                let order = t.endpoint_cycle_order(0, next).unwrap();
                assert_eq!(
                    order
                        .iter()
                        .copied()
                        .collect::<std::collections::BTreeSet<_>>()
                        .len(),
                    2 * k
                );
                for i in 0..2 * k {
                    assert!(adjacency[order[i]].contains(&order[(i + 1) % (2 * k)]));
                }
            } else {
                assert!(matches!(
                    t.endpoint_cycle_order(0, next),
                    Err(ListKOptError::Disconnected)
                ));
            }
        }
    }
}
