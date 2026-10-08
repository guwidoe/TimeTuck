// Proposal-sequence fixtures adapted from Timefold Community, Apache-2.0.
// These partial adaptations do NOT earn upstream method credit: selector size,
// unbounded-source metadata and lifecycle are outside this iterator API.
// Exact source identities and modifications are in compat/provenance.json.
use std::cell::Cell;
use timetuck::*;
fn pos(entity: usize, index: usize) -> ListPosition {
    ListPosition { entity, index }
}
fn problem(lists: Vec<Vec<i32>>, optional: bool) -> ListProblem<i32> {
    let mut values = lists.iter().flatten().copied().collect::<Vec<_>>();
    if optional {
        values.push(4);
    }
    let p = ListProblem::new(values, lists.into_iter().map(ListEntity::new).collect()).unwrap();
    if optional { p.allowing_unassigned() } else { p }
}
fn code(m: ListMove<i32>) -> String {
    let entity = |e: usize| char::from(b'A' + e as u8);
    match m {
        ListMove::SubListChange {
            entity: e,
            start,
            end,
            destination,
            reverse,
        } => format!(
            "|{}| {{{}[{}..{}]{}{}[{}]}}",
            end - start,
            entity(e),
            start,
            end,
            if reverse { "-reversing->" } else { "->" },
            entity(destination.entity),
            destination.index
        ),
        ListMove::Unassign {
            entity: e,
            start,
            end,
        } => format!(
            "|{}| {{{}[{}..{}]->null}}",
            end - start,
            entity(e),
            start,
            end
        ),
        ListMove::SubListSwap {
            left,
            right,
            reverse,
        } => format!(
            "{{{}[{}+{}]}} {} {{{}[{}+{}]}}",
            entity(left.entity),
            left.start,
            left.end - left.start,
            if reverse { "<-reversing->" } else { "<->" },
            entity(right.entity),
            right.start,
            right.end - right.start
        ),
        _ => panic!("unexpected proposal"),
    }
}

#[test]
fn change_unrestricted() {
    let p = problem(vec![vec![1, 2, 3, 4], vec![]], false);
    let a = p.initial_assignment();
    let index = SubListSelection::new(&p, &a, SubListSize::new(1, usize::MAX).unwrap()).unwrap();
    let mut ranks = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0].into_iter();
    let bound = Cell::new(0);
    let draw = |n| {
        bound.set(n);
        ranks.next().unwrap()
    };
    let mut iter = index.change_samples_with(
        vec![0].into_iter().cycle(),
        vec![Some(pos(1, 0))].into_iter().cycle(),
        usize::MAX,
        false,
        draw,
    );
    for expected in [
        "|4| {A[0..4]->B[0]}",
        "|3| {A[0..3]->B[0]}",
        "|3| {A[1..4]->B[0]}",
        "|2| {A[0..2]->B[0]}",
        "|2| {A[1..3]->B[0]}",
        "|2| {A[2..4]->B[0]}",
        "|1| {A[0..1]->B[0]}",
        "|1| {A[1..2]->B[0]}",
        "|1| {A[2..3]->B[0]}",
        "|1| {A[3..4]->B[0]}",
    ] {
        assert_eq!(code(iter.next().unwrap()), expected);
    }
    assert!(iter.next().is_some());
    assert_eq!(bound.get(), index.entity_len(0));
    drop(iter);
    assert!(ranks.next().is_none());
}

#[test]
fn change_optional() {
    let p = problem(vec![vec![1, 2], vec![3]], true);
    let a = p.initial_assignment();
    let index = SubListSelection::new(&p, &a, SubListSize::new(1, usize::MAX).unwrap()).unwrap();
    let mut ranks = [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 0,
    ]
    .into_iter();
    let bound = Cell::new(0);
    let draw = |n| {
        bound.set(n);
        ranks.next().unwrap()
    };
    let mut iter = index.change_samples_with(
        [vec![0; 6], vec![1; 6], vec![0; 18]]
            .concat()
            .into_iter()
            .cycle(),
        vec![
            Some(pos(0, 0)),
            Some(pos(0, 1)),
            Some(pos(0, 2)),
            Some(pos(1, 0)),
            Some(pos(1, 1)),
            None,
        ]
        .into_iter()
        .cycle(),
        usize::MAX,
        false,
        draw,
    );
    for expected in [
        "|2| {A[0..2]->A[0]}",
        "|2| {A[0..2]->A[1]}",
        "|2| {A[0..2]->A[2]}",
        "|2| {A[0..2]->B[0]}",
        "|2| {A[0..2]->B[1]}",
        "|2| {A[0..2]->null}",
        "|1| {B[0..1]->A[0]}",
        "|1| {B[0..1]->A[1]}",
        "|1| {B[0..1]->A[2]}",
        "|1| {B[0..1]->B[0]}",
        "|1| {B[0..1]->B[1]}",
        "|1| {B[0..1]->null}",
        "|1| {A[0..1]->A[0]}",
        "|1| {A[0..1]->A[1]}",
        "|1| {A[0..1]->A[2]}",
        "|1| {A[0..1]->B[0]}",
        "|1| {A[0..1]->B[1]}",
        "|1| {A[0..1]->null}",
        "|1| {A[1..2]->A[0]}",
        "|1| {A[1..2]->A[1]}",
        "|1| {A[1..2]->A[2]}",
        "|1| {A[1..2]->B[0]}",
        "|1| {A[1..2]->B[1]}",
        "|1| {A[1..2]->null}",
    ] {
        assert_eq!(code(iter.next().unwrap()), expected);
    }
    assert!(iter.next().is_some());
    assert_eq!(bound.get(), index.entity_len(0));
    drop(iter);
    assert!(ranks.next().is_none());
}

#[test]
fn change_reversing() {
    let p = problem(vec![vec![1, 2, 3, 4], vec![]], false);
    let a = p.initial_assignment();
    let index = SubListSelection::new(&p, &a, SubListSize::new(1, usize::MAX).unwrap()).unwrap();
    let mut ranks = [
        0, 1, 1, 0, 2, 1, 3, 1, 4, 0, 5, 1, 6, 0, 7, 1, 8, 1, 9, 0, 0, 0,
    ]
    .into_iter();
    let bound = Cell::new(0);
    let draw = |n| {
        bound.set(n);
        ranks.next().unwrap()
    };
    let mut iter = index.change_samples_with(
        vec![0].into_iter().cycle(),
        vec![Some(pos(1, 0))].into_iter().cycle(),
        usize::MAX,
        true,
        draw,
    );
    for expected in [
        "|4| {A[0..4]-reversing->B[0]}",
        "|3| {A[0..3]->B[0]}",
        "|3| {A[1..4]-reversing->B[0]}",
        "|2| {A[0..2]-reversing->B[0]}",
        "|2| {A[1..3]->B[0]}",
        "|2| {A[2..4]-reversing->B[0]}",
        "|1| {A[0..1]->B[0]}",
        "|1| {A[1..2]-reversing->B[0]}",
        "|1| {A[2..3]-reversing->B[0]}",
        "|1| {A[3..4]->B[0]}",
    ] {
        assert_eq!(code(iter.next().unwrap()), expected);
    }
    assert!(iter.next().is_some());
    drop(iter);
    assert!(ranks.next().is_none());
}

#[test]
fn change_bounds() {
    let p = problem(vec![vec![1, 2, 3, 4], vec![]], false);
    let a = p.initial_assignment();
    let index = SubListSelection::new(&p, &a, SubListSize::new(2, 3).unwrap()).unwrap();
    let mut ranks = [0, 1, 2, 3, 4, 0].into_iter();
    let bound = Cell::new(0);
    let draw = |n| {
        bound.set(n);
        ranks.next().unwrap()
    };
    let mut iter = index.change_samples_with(
        vec![0].into_iter().cycle(),
        vec![Some(pos(1, 0))].into_iter().cycle(),
        usize::MAX,
        false,
        draw,
    );
    for expected in [
        "|3| {A[0..3]->B[0]}",
        "|3| {A[1..4]->B[0]}",
        "|2| {A[0..2]->B[0]}",
        "|2| {A[1..3]->B[0]}",
        "|2| {A[2..4]->B[0]}",
    ] {
        assert_eq!(code(iter.next().unwrap()), expected);
    }
    assert!(iter.next().is_some());
    assert_eq!(bound.get(), index.entity_len(0));
    drop(iter);
    assert!(ranks.next().is_none());
}

#[test]
fn change_skip_short() {
    let p = problem(vec![vec![1, 2, 3], vec![], vec![4]], false);
    let a = p.initial_assignment();
    let index = SubListSelection::new(&p, &a, SubListSize::new(2, 2).unwrap()).unwrap();
    let mut ranks = [0, 1, 0].into_iter();
    let bound = Cell::new(0);
    let draw = |n| {
        bound.set(n);
        ranks.next().unwrap()
    };
    let mut iter = index.change_samples_with(
        vec![2, 0].into_iter().cycle(),
        vec![Some(pos(1, 0))].into_iter().cycle(),
        usize::MAX,
        false,
        draw,
    );
    for expected in ["|2| {A[0..2]->B[0]}", "|2| {A[1..3]->B[0]}"] {
        assert_eq!(code(iter.next().unwrap()), expected);
    }
    assert!(iter.next().is_some());
    assert_eq!(bound.get(), index.entity_len(0));
    drop(iter);
    assert!(ranks.next().is_none());
}

#[test]
fn swap_unrestricted() {
    let p = problem(vec![vec![1, 2, 3, 4]], false);
    let a = p.initial_assignment();
    let index = SubListSelection::new(&p, &a, SubListSize::new(1, usize::MAX).unwrap()).unwrap();
    let mut ranks = [
        0, 0, 0, 1, 0, 2, 0, 3, 0, 4, 0, 5, 0, 6, 0, 7, 0, 8, 0, 9, 1, 8, 2, 7, 3, 6, 4, 5, 5, 4,
        6, 3, 7, 2, 8, 1, 9, 0, 0, 0,
    ]
    .into_iter();
    let bound = Cell::new(0);
    let draw = |n| {
        bound.set(n);
        ranks.next().unwrap()
    };
    let mut iter = index.swap_samples_with(
        &index,
        (vec![0].into_iter().cycle(), vec![0].into_iter().cycle()),
        usize::MAX,
        false,
        draw,
    );
    for expected in [
        "{A[0+4]} <-> {A[0+4]}",
        "{A[0+4]} <-> {A[0+3]}",
        "{A[0+4]} <-> {A[1+3]}",
        "{A[0+4]} <-> {A[0+2]}",
        "{A[0+4]} <-> {A[1+2]}",
        "{A[0+4]} <-> {A[2+2]}",
        "{A[0+4]} <-> {A[0+1]}",
        "{A[0+4]} <-> {A[1+1]}",
        "{A[0+4]} <-> {A[2+1]}",
        "{A[0+4]} <-> {A[3+1]}",
        "{A[0+3]} <-> {A[2+1]}",
        "{A[1+3]} <-> {A[1+1]}",
        "{A[0+2]} <-> {A[0+1]}",
        "{A[1+2]} <-> {A[2+2]}",
        "{A[1+2]} <-> {A[2+2]}",
        "{A[0+1]} <-> {A[0+2]}",
        "{A[1+1]} <-> {A[1+3]}",
        "{A[0+3]} <-> {A[2+1]}",
        "{A[0+4]} <-> {A[3+1]}",
    ] {
        assert_eq!(code(iter.next().unwrap()), expected);
    }
    assert!(iter.next().is_some());
    assert_eq!(bound.get(), index.entity_len(0));
    drop(iter);
    assert!(ranks.next().is_none());
}

#[test]
fn swap_bounds() {
    let p = problem(vec![vec![1, 2, 3, 4]], false);
    let a = p.initial_assignment();
    let index = SubListSelection::new(&p, &a, SubListSize::new(2, 3).unwrap()).unwrap();
    let mut ranks = [0, 0, 0, 1, 0, 2, 0, 3, 0, 4, 1, 3, 2, 2, 3, 1, 4, 0, 0, 0].into_iter();
    let bound = Cell::new(0);
    let draw = |n| {
        bound.set(n);
        ranks.next().unwrap()
    };
    let mut iter = index.swap_samples_with(
        &index,
        (vec![0].into_iter().cycle(), vec![0].into_iter().cycle()),
        usize::MAX,
        false,
        draw,
    );
    for expected in [
        "{A[0+3]} <-> {A[0+3]}",
        "{A[0+3]} <-> {A[1+3]}",
        "{A[0+3]} <-> {A[0+2]}",
        "{A[0+3]} <-> {A[1+2]}",
        "{A[0+3]} <-> {A[2+2]}",
        "{A[1+3]} <-> {A[1+2]}",
        "{A[0+2]} <-> {A[0+2]}",
        "{A[1+2]} <-> {A[1+3]}",
        "{A[0+3]} <-> {A[2+2]}",
    ] {
        assert_eq!(code(iter.next().unwrap()), expected);
    }
    assert!(iter.next().is_some());
    assert_eq!(bound.get(), index.entity_len(0));
    drop(iter);
    assert!(ranks.next().is_none());
}

#[test]
fn swap_reversing() {
    let p = problem(vec![vec![1, 2, 3], vec![5, 6]], false);
    let a = p.initial_assignment();
    let index = SubListSelection::new(&p, &a, SubListSize::new(1, usize::MAX).unwrap()).unwrap();
    let mut ranks = [
        0, 2, 1, 0, 1, 0, 0, 0, 1, 1, 0, 0, 2, 0, 1, 3, 0, 1, 0, 0, 0,
    ]
    .into_iter();
    let bound = Cell::new(0);
    let draw = |n| {
        bound.set(n);
        ranks.next().unwrap()
    };
    let mut iter = index.swap_samples_with(
        &index,
        (vec![0].into_iter().cycle(), vec![1].into_iter().cycle()),
        usize::MAX,
        true,
        draw,
    );
    for expected in [
        "{A[0+3]} <-reversing-> {B[1+1]}",
        "{A[0+3]} <-> {B[0+1]}",
        "{A[0+3]} <-reversing-> {B[0+2]}",
        "{A[0+2]} <-> {B[0+2]}",
        "{A[1+2]} <-reversing-> {B[0+2]}",
        "{A[0+1]} <-reversing-> {B[0+2]}",
    ] {
        assert_eq!(code(iter.next().unwrap()), expected);
    }
    assert!(iter.next().is_some());
    drop(iter);
    assert!(ranks.next().is_none());
}

#[test]
fn swap_skip_short() {
    let p = problem(vec![vec![1, 2, 3], vec![], vec![4]], false);
    let a = p.initial_assignment();
    let index = SubListSelection::new(&p, &a, SubListSize::new(2, 2).unwrap()).unwrap();
    let mut ranks = [0, 0, 0, 0, 0, 1, 1, 0, 1, 1, 0, 0].into_iter();
    let bound = Cell::new(0);
    let draw = |n| {
        bound.set(n);
        ranks.next().unwrap()
    };
    let mut iter = index.swap_samples_with(
        &index,
        (
            vec![0, 2].into_iter().cycle(),
            vec![2, 0].into_iter().cycle(),
        ),
        usize::MAX,
        false,
        draw,
    );
    for expected in [
        "{A[0+2]} <-> {A[0+2]}",
        "{A[0+2]} <-> {A[0+2]}",
        "{A[0+2]} <-> {A[1+2]}",
        "{A[0+2]} <-> {A[1+2]}",
        "{A[1+2]} <-> {A[1+2]}",
    ] {
        assert_eq!(code(iter.next().unwrap()), expected);
    }
    assert!(iter.next().is_some());
    assert_eq!(bound.get(), index.entity_len(0));
    drop(iter);
    assert!(ranks.next().is_none());
}

#[test]
fn swap_optional() {
    let p = problem(vec![vec![1, 2], vec![3]], true);
    let a = p.initial_assignment();
    let index = SubListSelection::new(&p, &a, SubListSize::new(1, usize::MAX).unwrap()).unwrap();
    let mut ranks = [
        0, 0, 0, 1, 0, 2, 0, 0, 1, 0, 1, 1, 1, 2, 1, 0, 2, 0, 2, 1, 2, 2, 2, 0, 0, 0, 0, 1, 0, 2,
        0, 0, 0, 0,
    ]
    .into_iter();
    let bound = Cell::new(0);
    let draw = |n| {
        bound.set(n);
        ranks.next().unwrap()
    };
    let mut iter = index.swap_samples_with(
        &index,
        (
            [vec![0; 12], vec![1; 4]].concat().into_iter().cycle(),
            vec![0, 0, 0, 1].into_iter().cycle(),
        ),
        usize::MAX,
        false,
        draw,
    );
    for expected in [
        "{A[0+2]} <-> {A[0+2]}",
        "{A[0+2]} <-> {A[0+1]}",
        "{A[0+2]} <-> {A[1+1]}",
        "{A[0+2]} <-> {B[0+1]}",
        "{A[0+1]} <-> {A[0+2]}",
        "{A[0+1]} <-> {A[0+1]}",
        "{A[0+1]} <-> {A[1+1]}",
        "{A[0+1]} <-> {B[0+1]}",
        "{A[0+2]} <-> {A[1+1]}",
        "{A[0+1]} <-> {A[1+1]}",
        "{A[1+1]} <-> {A[1+1]}",
        "{A[1+1]} <-> {B[0+1]}",
        "{B[0+1]} <-> {A[0+2]}",
        "{B[0+1]} <-> {A[0+1]}",
        "{B[0+1]} <-> {A[1+1]}",
        "{B[0+1]} <-> {B[0+1]}",
    ] {
        assert_eq!(code(iter.next().unwrap()), expected);
    }
    assert!(iter.next().is_some());
    drop(iter);
    assert!(ranks.next().is_none());
}
