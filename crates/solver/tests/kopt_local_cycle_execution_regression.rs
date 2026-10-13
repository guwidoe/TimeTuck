//! Native physical reconnection checks; first wrap geometry adapts a pinned
//! Community TEST fixture (Apache-2.0). See compat/provenance.json and
//! THIRD_PARTY_NOTICES.md. No complete upstream-method credit.
use std::collections::BTreeSet;
use timetuck::*;
fn at(entity: usize, index: usize) -> ListPosition {
    ListPosition { entity, index }
}
#[test]
fn local_wrap_reconnections_expand_all_physical_interiors_and_execute() {
    let p = ListProblem::new_indexed(
        (0..9).collect(),
        vec![
            ListEntity::new(vec![0, 1, 2, 3, 4, 5]).with_pinned_prefix(1),
            ListEntity::new(vec![6, 7, 8]),
        ],
    )
    .unwrap();
    let a = p.initial_assignment();
    // Two local paths from A and two from B; reconnection makes one complete
    // physical cycle crossing both original local seams.
    let plan = ListKOptLocalCyclePlan::prepare(
        &p,
        &a,
        &[
            (at(0, 5), at(0, 1)),
            (at(0, 2), at(0, 3)),
            (at(1, 0), at(1, 1)),
        ],
        &[(1, 3), (2, 5), (4, 0)],
    )
    .unwrap();
    assert_eq!(plan.physical_cycle_count(), 1);
    let forward = plan.cycle_positions(at(0, 1), false).unwrap();
    let values = forward
        .iter()
        .map(|p| a.lists()[p.entity][p.index])
        .collect::<Vec<_>>();
    assert_eq!(values, vec![1, 2, 7, 8, 6, 5, 4, 3]);
    let reverse = plan.cycle_positions(at(0, 1), true).unwrap();
    let mut expected = forward.clone();
    expected[1..].reverse();
    assert_eq!(reverse, expected);
    let movement = plan.movement(&p, &a, at(0, 1), false, &[3, 5]).unwrap();
    let mut actual = a.clone();
    {
        let trial = p.apply(&mut actual, &movement).unwrap();
        assert_eq!(trial.lists(), &[vec![0, 1, 2, 7], vec![8, 6, 5, 4, 3]]);
    }
    assert_eq!(actual, a);
    p.apply(&mut actual, &movement).unwrap().commit();
    assert_eq!(actual.lists(), &[vec![0, 1, 2, 7], vec![8, 6, 5, 4, 3]]);
    assert!(matches!(
        plan.movement(&p, &actual, at(0, 1), false, &[3, 5]),
        Err(ListKOptError::Partition)
    ));
    assert!(matches!(
        plan.movement(&p, &a, at(0, 1), false, &[3, 4]),
        Err(ListKOptError::Partition)
    ));
}
fn matchings(
    slots: Vec<usize>,
    pairs: &mut Vec<(usize, usize)>,
    out: &mut Vec<Vec<(usize, usize)>>,
) {
    if slots.is_empty() {
        out.push(pairs.clone());
        return;
    }
    for index in 1..slots.len() {
        let mut remaining = slots[1..].to_vec();
        let partner = remaining.remove(index - 1);
        pairs.push((slots[0], partner));
        matchings(remaining, pairs, out);
        pairs.pop();
    }
}
#[test]
fn every_eight_port_matching_preserves_each_value_and_uncut_adjacency() {
    let p = ListProblem::new_indexed(
        (0..12).collect(),
        vec![
            ListEntity::new(vec![0, 1, 2, 3, 4, 5]),
            ListEntity::new(vec![6, 7, 8, 9, 10, 11]),
        ],
    )
    .unwrap();
    let a = p.initial_assignment();
    let cuts = [
        (at(0, 0), at(0, 1)),
        (at(0, 3), at(0, 4)),
        (at(1, 0), at(1, 1)),
        (at(1, 3), at(1, 4)),
    ];
    let mut all = Vec::new();
    matchings((0..8).collect(), &mut Vec::new(), &mut all);
    assert_eq!(all.len(), 105);
    let mut executable = 0;
    for added in all {
        let plan = ListKOptLocalCyclePlan::prepare(&p, &a, &cuts, &added).unwrap();
        let components = plan.physical_cycles();
        let mut seen = BTreeSet::new();
        let mut actual_edges = BTreeSet::new();
        for cycle in &components {
            for (index, &position) in cycle.iter().enumerate() {
                let value = a.lists()[position.entity][position.index];
                assert!(seen.insert(value));
                let next = cycle[(index + 1) % cycle.len()];
                let next = a.lists()[next.entity][next.index];
                actual_edges.insert((value.min(next), value.max(next)));
            }
        }
        assert_eq!(seen, (0..12).collect());
        for row in a.lists() {
            for index in [1, 2, 4, 5] {
                let x = row[index];
                let y = row[(index + 1) % row.len()];
                assert!(actual_edges.contains(&(x.min(y), x.max(y))));
            }
        }
        for &(left, right) in &added {
            let endpoints = cuts.iter().flat_map(|&(a, b)| [a, b]).collect::<Vec<_>>();
            let x = a.lists()[endpoints[left].entity][endpoints[left].index];
            let y = a.lists()[endpoints[right].entity][endpoints[right].index];
            assert!(actual_edges.contains(&(x.min(y), x.max(y))));
        }
        if components.len() == 1 {
            executable += 1;
            let movement = plan.movement(&p, &a, at(0, 0), false, &[6, 6]).unwrap();
            let mut actual = a.clone();
            p.apply(&mut actual, &movement).unwrap().commit();
            assert_eq!(
                actual
                    .lists()
                    .iter()
                    .flatten()
                    .copied()
                    .collect::<BTreeSet<_>>(),
                seen
            );
        } else {
            assert!(matches!(
                plan.movement(&p, &a, at(0, 0), false, &[6, 6]),
                Err(ListKOptError::Disconnected)
            ));
        }
    }
    assert!(executable > 0 && executable < 105);
}
#[test]
fn singleton_loops_parallel_edges_and_final_receiving_ranges_are_real() {
    let p = ListProblem::new_indexed(
        vec![0, 1],
        vec![ListEntity::new(vec![0]), ListEntity::new(vec![1])],
    )
    .unwrap();
    let a = p.initial_assignment();
    let plan = ListKOptLocalCyclePlan::prepare(
        &p,
        &a,
        &[(at(0, 0), at(0, 0)), (at(1, 0), at(1, 0))],
        &[(0, 2), (1, 3)],
    )
    .unwrap();
    assert_eq!(plan.physical_cycle_count(), 1);
    assert_eq!(
        plan.cycle_positions(at(0, 0), false).unwrap(),
        [at(0, 0), at(1, 0)]
    );
    let movement = plan.movement(&p, &a, at(0, 0), false, &[2, 0]).unwrap();
    let mut actual = a.clone();
    p.apply(&mut actual, &movement).unwrap().commit();
    assert_eq!(actual.lists(), &[vec![0, 1], vec![]]);
    let restricted = ListProblem::new_indexed(
        vec![0, 1],
        vec![
            ListEntity::new(vec![0]).with_value_range(vec![0]),
            ListEntity::new(vec![1]).with_value_range(vec![1]),
        ],
    )
    .unwrap();
    assert!(
        plan.movement(&restricted, &a, at(0, 0), false, &[2, 0])
            .is_err()
    );
    let parallel = ListProblem::new_indexed(vec![0, 1], vec![ListEntity::new(vec![0, 1])]).unwrap();
    let before = parallel.initial_assignment();
    let plan = ListKOptLocalCyclePlan::prepare(
        &parallel,
        &before,
        &[(at(0, 0), at(0, 1)), (at(0, 1), at(0, 0))],
        &[(0, 2), (1, 3)],
    )
    .unwrap();
    assert_eq!(plan.physical_cycle_count(), 1);
    assert_eq!(
        plan.cycle_positions(at(0, 0), false).unwrap(),
        [at(0, 0), at(0, 1)]
    );
    assert!(
        ListKOptLocalCyclePlan::prepare(
            &parallel,
            &before,
            &[(at(0, 0), at(0, 1)), (at(0, 0), at(0, 1))],
            &[(0, 3), (1, 2)]
        )
        .is_err()
    );
}
