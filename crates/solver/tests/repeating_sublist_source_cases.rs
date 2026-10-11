// Adapted from pinned Community random sublist selector tests, Apache-2.0.
// Complete method/helper mappings are recorded in compat/provenance.json.
// Native proposal coordinates replace Java move objects; no PRNG trace claim.
use std::cell::{Cell, RefCell};
use std::num::NonZeroUsize;
use timetuck::*;

// Scripted repeating children model source selector mocks. Advertised size is
// their catalog size, independent of repeated occurrence order.
struct Scripted<I> {
    values: I,
    count: usize,
}
impl<C, I: Iterator + Clone> SelectionSource<C, I::Item> for Scripted<I> {
    fn is_never_ending(&self) -> bool {
        true
    }
    fn size(&mut self, _: &C) -> Option<usize> {
        Some(self.count)
    }
    fn open<'a>(&'a mut self, _: &'a C) -> Box<dyn Iterator<Item = I::Item> + 'a>
    where
        I::Item: 'a,
    {
        Box::new(self.values.clone())
    }
}
fn repeats<C, T>(source: &impl SelectionSource<C, T>, _: &C) -> bool {
    source.is_never_ending()
}
fn change_code(p: SubListChangeProposal) -> String {
    // Render PRE-removal proposal coordinates, including source no-op/interior
    // proposals. Do not reinterpret them as executable native coordinates.
    match p.destination {
        Some(destination) => code(ListMove::SubListChange {
            entity: p.source.entity,
            start: p.source.start,
            end: p.source.end,
            destination,
            reverse: p.reverse,
        }),
        None => code(ListMove::Unassign {
            entity: p.source.entity,
            start: p.source.start,
            end: p.source.end,
        }),
    }
}

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
    let c = ListSelectionContext::new(&p, &a).unwrap();
    let mut ranks = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0].into_iter();
    let bound = Cell::new(0);
    let draw = |n| {
        bound.set(n);
        ranks.next().unwrap()
    };
    {
        let draw = RefCell::new(draw);
        let mut source = SubListChangeSource::with_draw(
            EntitySubListSource::repeating_with_draw(
                Scripted {
                    values: vec![0].into_iter().cycle(),
                    count: p.entities().len(),
                },
                SubListSize::new(1, usize::MAX).unwrap(),
                NonZeroUsize::new(8).unwrap(),
                |n| (draw.borrow_mut())(n),
            ),
            Scripted {
                values: vec![Some(pos(1, 0))].into_iter().cycle(),
                count: 3,
            },
            false,
            |n| (draw.borrow_mut())(n),
        );
        assert_eq!(source.proposal_count(&c), Some(30));
        assert_eq!(
            10,
            a.lists()
                .iter()
                .map(|r| (r.len() * (r.len() + 1) / 2) as u128)
                .sum::<u128>()
        );
        assert!(repeats(&source, &c));
        let mut iter = source.open(&c);
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
            assert_eq!(change_code(iter.next().unwrap()), expected);
        }
        assert!(iter.next().is_some());
        assert_eq!(bound.get(), 10);
    }
    assert!(ranks.next().is_none());
}

#[test]
fn change_optional() {
    let p = problem(vec![vec![1, 2], vec![3]], true);
    let a = p.initial_assignment();
    let c = ListSelectionContext::new(&p, &a).unwrap();
    let mut ranks = [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 0,
    ]
    .into_iter();
    let bound = Cell::new(0);
    let draw = |n| {
        bound.set(n);
        ranks.next().unwrap()
    };
    {
        let draw = RefCell::new(draw);
        let mut source = SubListChangeSource::with_draw(
            EntitySubListSource::repeating_with_draw(
                Scripted {
                    values: [vec![0; 6], vec![1; 6], vec![0; 18]]
                        .concat()
                        .into_iter()
                        .cycle(),
                    count: p.entities().len(),
                },
                SubListSize::new(1, usize::MAX).unwrap(),
                NonZeroUsize::new(8).unwrap(),
                |n| (draw.borrow_mut())(n),
            ),
            Scripted {
                values: vec![
                    Some(pos(0, 0)),
                    Some(pos(0, 1)),
                    Some(pos(0, 2)),
                    Some(pos(1, 0)),
                    Some(pos(1, 1)),
                    None,
                ]
                .into_iter()
                .cycle(),
                count: 6,
            },
            false,
            |n| (draw.borrow_mut())(n),
        );
        assert_eq!(source.proposal_count(&c), Some(24));
        assert_eq!(
            4,
            a.lists()
                .iter()
                .map(|r| (r.len() * (r.len() + 1) / 2) as u128)
                .sum::<u128>()
        );
        assert!(repeats(&source, &c));
        let mut iter = source.open(&c);
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
            assert_eq!(change_code(iter.next().unwrap()), expected);
        }
        assert!(iter.next().is_some());
        assert_eq!(bound.get(), 3);
    }
    assert!(ranks.next().is_none());
}

#[test]
fn change_reversing() {
    let p = problem(vec![vec![1, 2, 3, 4], vec![]], false);
    let a = p.initial_assignment();
    let c = ListSelectionContext::new(&p, &a).unwrap();
    let mut ranks = [
        0, 1, 1, 0, 2, 1, 3, 1, 4, 0, 5, 1, 6, 0, 7, 1, 8, 1, 9, 0, 0, 0,
    ]
    .into_iter();
    let bound = Cell::new(0);
    let draw = |n| {
        bound.set(n);
        ranks.next().unwrap()
    };
    {
        let draw = RefCell::new(draw);
        let mut source = SubListChangeSource::with_draw(
            EntitySubListSource::repeating_with_draw(
                Scripted {
                    values: vec![0].into_iter().cycle(),
                    count: p.entities().len(),
                },
                SubListSize::new(1, usize::MAX).unwrap(),
                NonZeroUsize::new(8).unwrap(),
                |n| (draw.borrow_mut())(n),
            ),
            Scripted {
                values: vec![Some(pos(1, 0))].into_iter().cycle(),
                count: 13,
            },
            true,
            |n| (draw.borrow_mut())(n),
        );
        assert_eq!(source.proposal_count(&c), Some(260));
        assert!(repeats(&source, &c));
        let mut iter = source.open(&c);
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
            assert_eq!(change_code(iter.next().unwrap()), expected);
        }
        assert!(iter.next().is_some());
    }
    assert!(ranks.next().is_none());
}

#[test]
fn change_bounds() {
    let p = problem(vec![vec![1, 2, 3, 4], vec![]], false);
    let a = p.initial_assignment();
    let c = ListSelectionContext::new(&p, &a).unwrap();
    let mut ranks = [0, 1, 2, 3, 4, 0].into_iter();
    let bound = Cell::new(0);
    let draw = |n| {
        bound.set(n);
        ranks.next().unwrap()
    };
    {
        let draw = RefCell::new(draw);
        let mut source = SubListChangeSource::with_draw(
            EntitySubListSource::repeating_with_draw(
                Scripted {
                    values: vec![0].into_iter().cycle(),
                    count: p.entities().len(),
                },
                SubListSize::new(2, 3).unwrap(),
                NonZeroUsize::new(8).unwrap(),
                |n| (draw.borrow_mut())(n),
            ),
            Scripted {
                values: vec![Some(pos(1, 0))].into_iter().cycle(),
                count: 51,
            },
            false,
            |n| (draw.borrow_mut())(n),
        );
        assert_eq!(source.proposal_count(&c), Some(255));
        assert!(repeats(&source, &c));
        let mut iter = source.open(&c);
        for expected in [
            "|3| {A[0..3]->B[0]}",
            "|3| {A[1..4]->B[0]}",
            "|2| {A[0..2]->B[0]}",
            "|2| {A[1..3]->B[0]}",
            "|2| {A[2..4]->B[0]}",
        ] {
            assert_eq!(change_code(iter.next().unwrap()), expected);
        }
        assert!(iter.next().is_some());
        assert_eq!(bound.get(), 5);
    }
    assert!(ranks.next().is_none());
}

#[test]
fn change_skip_short() {
    let p = problem(vec![vec![1, 2, 3], vec![], vec![4]], false);
    let a = p.initial_assignment();
    let c = ListSelectionContext::new(&p, &a).unwrap();
    let mut ranks = [0, 1, 0].into_iter();
    let bound = Cell::new(0);
    let draw = |n| {
        bound.set(n);
        ranks.next().unwrap()
    };
    {
        let draw = RefCell::new(draw);
        let mut source = SubListChangeSource::with_draw(
            EntitySubListSource::repeating_with_draw(
                Scripted {
                    values: vec![2, 0].into_iter().cycle(),
                    count: p.entities().len(),
                },
                SubListSize::new(2, 2).unwrap(),
                NonZeroUsize::new(8).unwrap(),
                |n| (draw.borrow_mut())(n),
            ),
            Scripted {
                values: vec![Some(pos(1, 0))].into_iter().cycle(),
                count: 13,
            },
            false,
            |n| (draw.borrow_mut())(n),
        );
        assert!(repeats(&source, &c));
        let mut iter = source.open(&c);
        for expected in ["|2| {A[0..2]->B[0]}", "|2| {A[1..3]->B[0]}"] {
            assert_eq!(change_code(iter.next().unwrap()), expected);
        }
        assert!(iter.next().is_some());
        assert_eq!(bound.get(), 2);
    }
    assert!(ranks.next().is_none());
}

#[test]
fn swap_unrestricted() {
    let p = problem(vec![vec![1, 2, 3, 4]], false);
    let a = p.initial_assignment();
    let c = ListSelectionContext::new(&p, &a).unwrap();
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
    {
        let draw = RefCell::new(draw);
        let mut source = SubListSwapSource::with_draw(
            EntitySubListSource::repeating_with_draw(
                Scripted {
                    values: vec![0].into_iter().cycle(),
                    count: p.entities().len(),
                },
                SubListSize::new(1, usize::MAX).unwrap(),
                NonZeroUsize::new(8).unwrap(),
                |n| (draw.borrow_mut())(n),
            ),
            EntitySubListSource::repeating_with_draw(
                Scripted {
                    values: vec![0].into_iter().cycle(),
                    count: p.entities().len(),
                },
                SubListSize::new(1, usize::MAX).unwrap(),
                NonZeroUsize::new(8).unwrap(),
                |n| (draw.borrow_mut())(n),
            ),
            false,
            |n| (draw.borrow_mut())(n),
        );
        assert_eq!(source.proposal_count(&c), Some(100));
        assert!(repeats(&source, &c));
        let mut iter = source.open(&c);
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
            assert_eq!(code(iter.next().unwrap().into_move()), expected);
        }
        assert!(iter.next().is_some());
        assert_eq!(bound.get(), 10);
    }
    assert!(ranks.next().is_none());
}

#[test]
fn swap_bounds() {
    let p = problem(vec![vec![1, 2, 3, 4]], false);
    let a = p.initial_assignment();
    let c = ListSelectionContext::new(&p, &a).unwrap();
    let mut ranks = [0, 0, 0, 1, 0, 2, 0, 3, 0, 4, 1, 3, 2, 2, 3, 1, 4, 0, 0, 0].into_iter();
    let bound = Cell::new(0);
    let draw = |n| {
        bound.set(n);
        ranks.next().unwrap()
    };
    {
        let draw = RefCell::new(draw);
        let mut source = SubListSwapSource::with_draw(
            EntitySubListSource::repeating_with_draw(
                Scripted {
                    values: vec![0].into_iter().cycle(),
                    count: p.entities().len(),
                },
                SubListSize::new(2, 3).unwrap(),
                NonZeroUsize::new(8).unwrap(),
                |n| (draw.borrow_mut())(n),
            ),
            EntitySubListSource::repeating_with_draw(
                Scripted {
                    values: vec![0].into_iter().cycle(),
                    count: p.entities().len(),
                },
                SubListSize::new(2, 3).unwrap(),
                NonZeroUsize::new(8).unwrap(),
                |n| (draw.borrow_mut())(n),
            ),
            false,
            |n| (draw.borrow_mut())(n),
        );
        assert_eq!(source.proposal_count(&c), Some(25));
        assert!(repeats(&source, &c));
        let mut iter = source.open(&c);
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
            assert_eq!(code(iter.next().unwrap().into_move()), expected);
        }
        assert!(iter.next().is_some());
        assert_eq!(bound.get(), 5);
    }
    assert!(ranks.next().is_none());
}

#[test]
fn swap_reversing() {
    let p = problem(vec![vec![1, 2, 3], vec![5, 6]], false);
    let a = p.initial_assignment();
    let c = ListSelectionContext::new(&p, &a).unwrap();
    let mut ranks = [
        0, 2, 1, 0, 1, 0, 0, 0, 1, 1, 0, 0, 2, 0, 1, 3, 0, 1, 0, 0, 0,
    ]
    .into_iter();
    let bound = Cell::new(0);
    let draw = |n| {
        bound.set(n);
        ranks.next().unwrap()
    };
    {
        let draw = RefCell::new(draw);
        let mut source = SubListSwapSource::with_draw(
            EntitySubListSource::repeating_with_draw(
                Scripted {
                    values: vec![0].into_iter().cycle(),
                    count: p.entities().len(),
                },
                SubListSize::new(1, usize::MAX).unwrap(),
                NonZeroUsize::new(8).unwrap(),
                |n| (draw.borrow_mut())(n),
            ),
            EntitySubListSource::repeating_with_draw(
                Scripted {
                    values: vec![1].into_iter().cycle(),
                    count: p.entities().len(),
                },
                SubListSize::new(1, usize::MAX).unwrap(),
                NonZeroUsize::new(8).unwrap(),
                |n| (draw.borrow_mut())(n),
            ),
            true,
            |n| (draw.borrow_mut())(n),
        );
        assert_eq!(source.proposal_count(&c), Some(162));
        assert!(repeats(&source, &c));
        let mut iter = source.open(&c);
        for expected in [
            "{A[0+3]} <-reversing-> {B[1+1]}",
            "{A[0+3]} <-> {B[0+1]}",
            "{A[0+3]} <-reversing-> {B[0+2]}",
            "{A[0+2]} <-> {B[0+2]}",
            "{A[1+2]} <-reversing-> {B[0+2]}",
            "{A[0+1]} <-reversing-> {B[0+2]}",
        ] {
            assert_eq!(code(iter.next().unwrap().into_move()), expected);
        }
        assert!(iter.next().is_some());
    }
    assert!(ranks.next().is_none());
}

#[test]
fn swap_skip_short() {
    let p = problem(vec![vec![1, 2, 3], vec![], vec![4]], false);
    let a = p.initial_assignment();
    let c = ListSelectionContext::new(&p, &a).unwrap();
    let mut ranks = [0, 0, 0, 0, 0, 1, 1, 0, 1, 1, 0, 0].into_iter();
    let bound = Cell::new(0);
    let draw = |n| {
        bound.set(n);
        ranks.next().unwrap()
    };
    {
        let draw = RefCell::new(draw);
        let mut source = SubListSwapSource::with_draw(
            EntitySubListSource::repeating_with_draw(
                Scripted {
                    values: vec![0, 2].into_iter().cycle(),
                    count: p.entities().len(),
                },
                SubListSize::new(2, 2).unwrap(),
                NonZeroUsize::new(8).unwrap(),
                |n| (draw.borrow_mut())(n),
            ),
            EntitySubListSource::repeating_with_draw(
                Scripted {
                    values: vec![2, 0].into_iter().cycle(),
                    count: p.entities().len(),
                },
                SubListSize::new(2, 2).unwrap(),
                NonZeroUsize::new(8).unwrap(),
                |n| (draw.borrow_mut())(n),
            ),
            false,
            |n| (draw.borrow_mut())(n),
        );
        assert_eq!(source.proposal_count(&c), Some(4));
        assert!(repeats(&source, &c));
        let mut iter = source.open(&c);
        for expected in [
            "{A[0+2]} <-> {A[0+2]}",
            "{A[0+2]} <-> {A[0+2]}",
            "{A[0+2]} <-> {A[1+2]}",
            "{A[0+2]} <-> {A[1+2]}",
            "{A[1+2]} <-> {A[1+2]}",
        ] {
            assert_eq!(code(iter.next().unwrap().into_move()), expected);
        }
        assert!(iter.next().is_some());
        assert_eq!(bound.get(), 2);
    }
    assert!(ranks.next().is_none());
}

#[test]
fn swap_optional() {
    let p = problem(vec![vec![1, 2], vec![3]], true);
    let a = p.initial_assignment();
    let c = ListSelectionContext::new(&p, &a).unwrap();
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
    {
        let draw = RefCell::new(draw);
        let mut source = SubListSwapSource::with_draw(
            EntitySubListSource::repeating_with_draw(
                Scripted {
                    values: [vec![0; 12], vec![1; 4]].concat().into_iter().cycle(),
                    count: p.entities().len(),
                },
                SubListSize::new(1, usize::MAX).unwrap(),
                NonZeroUsize::new(8).unwrap(),
                |n| (draw.borrow_mut())(n),
            ),
            EntitySubListSource::repeating_with_draw(
                Scripted {
                    values: vec![0, 0, 0, 1].into_iter().cycle(),
                    count: p.entities().len(),
                },
                SubListSize::new(1, usize::MAX).unwrap(),
                NonZeroUsize::new(8).unwrap(),
                |n| (draw.borrow_mut())(n),
            ),
            false,
            |n| (draw.borrow_mut())(n),
        );
        assert_eq!(source.proposal_count(&c), Some(16));
        assert!(repeats(&source, &c));
        let mut iter = source.open(&c);
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
            assert_eq!(code(iter.next().unwrap().into_move()), expected);
        }
        assert!(iter.next().is_some());
    }
    assert!(ranks.next().is_none());
}
