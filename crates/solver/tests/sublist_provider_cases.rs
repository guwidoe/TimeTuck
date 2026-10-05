// Adapted from Timefold Community SubList{Change,Swap,Unassign}MoveProviderTest.
// Apache-2.0. See compat/provenance.json and THIRD_PARTY_NOTICES.md.
// Native deterministic enumeration replaces random draws; trace parity is not claimed.
use std::collections::BTreeSet;
use timetuck::*;
fn problem(n: usize, prefix: usize, optional: bool) -> ListProblem<usize> {
    let entities = vec![ListEntity::new((0..n).collect()).with_pinned_prefix(prefix)];
    if optional {
        ListProblem::new((0..n).collect(), entities)
            .unwrap()
            .allowing_unassigned()
    } else {
        ListProblem::new((0..n).collect(), entities).unwrap()
    }
}
fn changes(p: &ListProblem<usize>, c: SubListChangeConfig, limit: usize) -> Vec<ListMove<usize>> {
    list_sublist_change_moves_with_config(p, &p.initial_assignment(), c)
        .unwrap()
        .take(limit)
        .collect()
}
fn swaps(p: &ListProblem<usize>, c: SubListSwapConfig) -> Vec<ListMove<usize>> {
    list_sublist_swap_moves_with_config(p, &p.initial_assignment(), c)
        .unwrap()
        .take(300)
        .collect()
}
fn unassigns(p: &ListProblem<usize>, min: usize, max: usize) -> Vec<ListMove<usize>> {
    list_sublist_unassign_moves(
        p,
        &p.initial_assignment(),
        SubListSize::new(min, max).unwrap(),
    )
    .unwrap()
    .take(300)
    .collect()
}
fn relocation(m: &ListMove<usize>) -> (ListRange, ListPosition, bool) {
    match *m {
        ListMove::SubListChange {
            entity,
            start,
            end,
            destination,
            reverse,
        } => (ListRange { entity, start, end }, destination, reverse),
        _ => panic!("expected relocation"),
    }
}
fn exchange(m: &ListMove<usize>) -> (ListRange, ListRange, bool) {
    match *m {
        ListMove::SubListSwap {
            left,
            right,
            reverse,
        } => (left, right, reverse),
        _ => panic!("expected exchange"),
    }
}
fn removal(m: &ListMove<usize>) -> ListRange {
    match *m {
        ListMove::Unassign { entity, start, end } => ListRange { entity, start, end },
        _ => panic!("expected removal"),
    }
}
#[test]
fn change_lengths() {
    let ms = changes(
        &problem(10, 0, false),
        SubListChangeConfig::new(2, 5).unwrap(),
        300,
    );
    assert!(!ms.is_empty());
    assert!(ms.iter().all(|m| {
        let (r, _, _) = relocation(m);
        (2..=5).contains(&(r.end - r.start))
    }));
}
#[test]
fn change_pinned_prefix() {
    let ms = changes(&problem(8, 3, false), SubListChangeConfig::default(), 300);
    assert!(!ms.is_empty());
    assert!(ms.iter().all(|m| {
        let (r, d, _) = relocation(m);
        r.start >= 3 && d.index >= 3
    }));
}
#[test]
fn change_fully_pinned() {
    let p = ListProblem::new(
        (0..8).collect(),
        vec![
            ListEntity::new(vec![0, 1, 2]).with_pinned(true),
            ListEntity::new(vec![3, 4, 5, 6, 7]),
        ],
    )
    .unwrap();
    let ms = changes(&p, SubListChangeConfig::default(), 300);
    assert!(!ms.is_empty());
    assert!(ms.iter().all(|m| {
        let (r, d, _) = relocation(m);
        r.entity != 0 && d.entity != 0
    }));
}
#[test]
fn change_variety() {
    let ms = changes(
        &problem(10, 0, false),
        SubListChangeConfig::new(2, 5).unwrap(),
        300,
    );
    let spans: BTreeSet<_> = ms
        .iter()
        .map(|m| {
            let (r, _, _) = relocation(m);
            (r.start, r.end)
        })
        .collect();
    assert!(spans.len() > 1);
}
#[test]
fn change_reversing() {
    let p = problem(10, 0, false);
    let c = SubListChangeConfig::new(2, 5).unwrap();
    assert!(changes(&p, c, 300).iter().any(|m| relocation(m).2));
    assert!(
        changes(&p, c.with_reversing(false).with_unassignment(false), 300)
            .iter()
            .all(|m| !relocation(m).2)
    );
}
#[test]
fn change_default_unassignment() {
    assert!(
        changes(&problem(8, 0, true), SubListChangeConfig::default(), 1000)
            .iter()
            .any(|m| matches!(m, ListMove::Unassign { .. }))
    );
}
#[test]
fn change_disabled_unassignment() {
    let ms = changes(
        &problem(8, 0, true),
        SubListChangeConfig::new(2, i32::MAX as usize)
            .unwrap()
            .with_unassignment(false),
        500,
    );
    assert!(!ms.is_empty());
    assert!(ms.iter().all(|m| !matches!(m, ListMove::Unassign { .. })));
}
#[test]
fn change_required_unassignment_rejected() {
    let p = problem(0, 0, false);
    let a = p.initial_assignment();
    assert!(
        list_sublist_change_moves_with_config(
            &p,
            &a,
            SubListChangeConfig::new(2, 5)
                .unwrap()
                .with_unassignment(true)
        )
        .is_err()
    );
}
#[test]
fn change_invalid_sizes() {
    for (min, max) in [(0, 5), (5, 2), (1, 5)] {
        assert!(SubListChangeConfig::new(min, max).is_err());
    }
}
#[test]
fn swap_nonoverlap() {
    let ms = swaps(
        &problem(10, 0, false),
        SubListSwapConfig::new(1, 4).unwrap(),
    );
    assert!(!ms.is_empty());
    assert!(ms.iter().all(|m| {
        let (l, r, _) = exchange(m);
        l.entity != r.entity || l.end <= r.start || r.end <= l.start
    }));
}
#[test]
fn swap_independent_bounds() {
    let p = ListProblem::new(
        (0..24).collect(),
        vec![
            ListEntity::new((0..12).collect()),
            ListEntity::new((12..24).collect()),
        ],
    )
    .unwrap();
    let ms = swaps(&p, SubListSwapConfig::asymmetric(1, 1, 3, 5).unwrap());
    assert!(!ms.is_empty());
    assert!(ms.iter().all(|m| {
        let (l, r, _) = exchange(m);
        let sizes = [l.end - l.start, r.end - r.start];
        sizes.contains(&1) && sizes.iter().any(|n| (3..=5).contains(n))
    }));
}
#[test]
fn swap_variety() {
    let ms = swaps(
        &problem(10, 0, false),
        SubListSwapConfig::new(1, 4).unwrap(),
    );
    let pairs: BTreeSet<_> = ms
        .iter()
        .map(|m| {
            let (l, r, _) = exchange(m);
            (l.entity, l.start, l.end, r.entity, r.start, r.end)
        })
        .collect();
    assert!(pairs.len() > 1);
}
#[test]
fn swap_reversing() {
    let p = problem(10, 0, false);
    let c = SubListSwapConfig::asymmetric(2, 4, 2, 4).unwrap();
    assert!(swaps(&p, c).iter().any(|m| exchange(m).2));
    assert!(
        swaps(&p, c.with_reversing(false))
            .iter()
            .all(|m| !exchange(m).2)
    );
}
#[test]
fn swap_singletons_never_reverse() {
    let ms = swaps(
        &problem(8, 0, false),
        SubListSwapConfig::asymmetric(1, 1, 1, 1).unwrap(),
    );
    assert!(!ms.is_empty());
    assert!(ms.iter().all(|m| !exchange(m).2));
}
#[test]
fn swap_pinned_prefix() {
    let ms = swaps(&problem(9, 4, false), SubListSwapConfig::default());
    assert!(!ms.is_empty());
    assert!(ms.iter().all(|m| {
        let (l, r, _) = exchange(m);
        l.start >= 4 && r.start >= 4
    }));
}
#[test]
fn swap_invalid_sizes() {
    assert!(SubListSwapConfig::new(0, 5).is_err());
    assert!(SubListSwapConfig::new(5, 2).is_err());
    assert!(SubListSwapConfig::asymmetric(1, 5, 0, 5).is_err());
    assert!(SubListSwapConfig::asymmetric(1, 5, 5, 2).is_err());
}
#[test]
fn unassign_lengths() {
    let ms = unassigns(&problem(10, 0, true), 2, 5);
    assert!(!ms.is_empty());
    assert!(ms.iter().all(|m| {
        let r = removal(m);
        (2..=5).contains(&(r.end - r.start))
    }));
}
#[test]
fn unassign_variety() {
    let ms = unassigns(&problem(10, 0, true), 2, usize::MAX);
    let spans: BTreeSet<_> = ms
        .iter()
        .map(|m| {
            let r = removal(m);
            (r.start, r.end)
        })
        .collect();
    assert!(spans.len() > 1);
}
#[test]
fn unassign_pinned_prefix() {
    let ms = unassigns(&problem(8, 3, true), 2, usize::MAX);
    assert!(!ms.is_empty());
    assert!(ms.iter().all(|m| removal(m).start >= 3));
}
#[test]
fn unassign_required_rejected() {
    let p = problem(0, 0, false);
    assert!(
        list_sublist_unassign_moves(
            &p,
            &p.initial_assignment(),
            SubListSize::new(2, usize::MAX).unwrap()
        )
        .is_err()
    );
}
#[test]
fn unassign_invalid_sizes() {
    let p = problem(0, 0, true);
    let a = p.initial_assignment();
    for (min, max) in [(0, 5), (5, 2), (1, 5)] {
        assert!(
            SubListSize::new(min, max)
                .and_then(|size| list_sublist_unassign_moves(&p, &a, size))
                .is_err()
        );
    }
}
