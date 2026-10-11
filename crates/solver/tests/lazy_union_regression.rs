//! Original draw-before-poll semantics and exact callback work regressions.
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;
#[derive(Clone)]
struct Source {
    id: usize,
    log: Rc<RefCell<Vec<(char, usize)>>>,
}
impl SelectionSource<(), Move<usize>> for Source {
    fn size(&mut self, _: &()) -> Option<usize> {
        Some(1)
    }
    fn event(&mut self, _: SelectionLifecycleEvent, _: &()) {
        self.log.borrow_mut().push(('e', self.id));
    }
    fn open<'a>(&'a mut self, _: &'a ()) -> Box<dyn Iterator<Item = Move<usize>> + 'a>
    where
        Move<usize>: 'a,
    {
        assert_ne!(self.id, 64, "disabled source opened");
        self.log.borrow_mut().push(('o', self.id));
        let mut remaining = true;
        Box::new(std::iter::from_fn(move || {
            self.log.borrow_mut().push(('p', self.id));
            if std::mem::take(&mut remaining) {
                Some(Move::Change {
                    variable: 0,
                    value: Some(self.id),
                })
            } else {
                None
            }
        }))
    }
}
#[test]
fn accepted_first_move_opens_one_of_sixty_four_families_and_default_stays_prefetched() {
    for lazy in [true, false] {
        let log = Rc::new(RefCell::new(vec![]));
        let mut union = SelectionUnion::new(
            (0..65)
                .map(|id| {
                    (
                        Source {
                            id,
                            log: log.clone(),
                        },
                        if id == 64 { 0.0 } else { 1.0 },
                    )
                })
                .collect(),
        )
        .unwrap();
        union.event(SelectionLifecycleEvent::StepStarted, &());
        assert_eq!(log.borrow().len(), 65);
        log.borrow_mut().clear();
        let movement = if lazy {
            let mut cursor = union.random_lazy_with(&(), || 0.0);
            assert!(log.borrow().is_empty());
            cursor.next().unwrap().unwrap()
        } else {
            let mut cursor = union.random_with(&(), || 0.0);
            assert!(log.borrow().is_empty());
            cursor.next().unwrap().unwrap()
        };
        let expected = if lazy { 1 } else { 64 };
        assert_eq!(
            log.borrow().iter().filter(|(k, _)| *k == 'o').count(),
            expected
        );
        assert_eq!(
            log.borrow().iter().filter(|(k, _)| *k == 'p').count(),
            expected
        );
        let problem = FiniteProblem::new(vec![Variable::new((0..64).collect())]).unwrap();
        let mut a = problem.initial_assignment();
        problem.apply(&mut a, &movement).unwrap().commit();
        assert_eq!(a.values(), [Some(0)]);
        // Dropped source cursors do not perform any additional open/poll work.
        assert_eq!(log.borrow().len(), 2 * expected);
    }
}
struct Polls {
    items: std::vec::IntoIter<usize>,
    count: Rc<Cell<usize>>,
    resurrect: bool,
}
impl Iterator for Polls {
    type Item = usize;
    fn next(&mut self) -> Option<usize> {
        self.count.set(self.count.get() + 1);
        self.items.next().or_else(|| {
            if std::mem::take(&mut self.resurrect) {
                None
            } else {
                Some(999)
            }
        })
    }
}
#[test]
fn weighted_redraw_retires_empty_and_nonfused_children_without_prefetch() {
    let counts: Vec<_> = (0..4).map(|_| Rc::new(Cell::new(0))).collect();
    let children = [vec![], vec![20, 21], vec![30], vec![40]]
        .into_iter()
        .zip([1.0, 1.0, 3.0, 0.0])
        .enumerate()
        .map(|(i, (items, w))| {
            (
                Polls {
                    items: items.into_iter(),
                    count: counts[i].clone(),
                    resurrect: true,
                },
                w,
            )
        })
        .collect();
    let draw_count = Cell::new(0);
    let mut draws = [0.0, 0.25, 0.5, 0.0, 0.9, 0.0].into_iter();
    let mut cursor = LazyWeightedInterleave::new(children, || {
        draw_count.set(draw_count.get() + 1);
        draws.next().unwrap()
    })
    .unwrap();
    assert_eq!(draw_count.get(), 0);
    assert!(counts.iter().all(|c| c.get() == 0));
    assert_eq!(
        cursor.by_ref().collect::<Result<Vec<_>, _>>().unwrap(),
        [30, 20, 21]
    );
    assert_eq!(
        counts.iter().map(|c| c.get()).collect::<Vec<_>>(),
        [1, 3, 2, 0]
    );
    assert_eq!(draw_count.get(), 6);
    assert!(cursor.next().is_none());
    assert_eq!(draw_count.get(), 6);
}
#[test]
fn seeded_finite_occurrences_match_an_independent_linear_reference() {
    for counts in [[0, 0, 0], [1, 2, 3], [5, 0, 2], [3, 3, 3]] {
        for seed in 0..32 {
            let weights = [1.0, 2.0, 4.0];
            let children = counts
                .into_iter()
                .enumerate()
                .map(|(i, n)| ((0..n).map(move |j| (i, j)), weights[i]))
                .collect();
            let mut random = SampleRandom::new(seed);
            let actual = LazyWeightedInterleave::new(children, move || {
                random.index(std::num::NonZeroUsize::new(1024).unwrap()) as f64 / 1024.0
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
            let mut random = SampleRandom::new(seed);
            let mut active = weights;
            let mut positions = [0; 3];
            let mut expected = vec![];
            while active.iter().sum::<f64>() > 0.0 {
                let total = active.iter().sum::<f64>();
                let draw = random.index(std::num::NonZeroUsize::new(1024).unwrap()) as f64 / 1024.0
                    * total;
                let mut sum = 0.0;
                let index = (0..3)
                    .find(|&i| {
                        sum += active[i];
                        draw < sum
                    })
                    .unwrap();
                if positions[index] == counts[index] {
                    active[index] = 0.0;
                } else {
                    expected.push((index, positions[index]));
                    positions[index] += 1;
                }
            }
            assert_eq!(actual, expected, "seed={seed} counts={counts:?}");
        }
    }
}
#[test]
fn errors_and_callback_panics_fuse_without_touching_other_children() {
    let polls = Cell::new(0);
    let children = vec![(
        std::iter::from_fn(|| {
            polls.set(polls.get() + 1);
            Some(1)
        }),
        1.0,
    )];
    let mut cursor = LazyWeightedInterleave::new(children, || f64::NAN).unwrap();
    assert!(cursor.next().unwrap().is_err());
    assert!(cursor.next().is_none());
    assert_eq!(polls.get(), 0);
    for child_panic in [false, true] {
        let count = Cell::new(0);
        let children = vec![(
            std::iter::from_fn(|| {
                count.set(count.get() + 1);
                assert!(!child_panic, "child panic");
                Some(1)
            }),
            1.0,
        )];
        let mut cursor = LazyWeightedInterleave::new(children, || {
            assert!(child_panic, "draw panic");
            0.0
        })
        .unwrap();
        assert!(catch_unwind(AssertUnwindSafe(|| cursor.next())).is_err());
        assert!(cursor.next().is_none());
        assert_eq!(count.get(), usize::from(child_panic));
    }
    let mut empty =
        LazyWeightedInterleave::new(Vec::<(std::iter::Empty<usize>, f64)>::new(), || {
            panic!("empty draw")
        })
        .unwrap();
    assert!(empty.next().is_none());
    assert!(
        LazyWeightedInterleave::new(vec![(std::iter::empty::<usize>(), -1.0)], || 0.0).is_err()
    );
}
#[test]
fn borrowed_nonclone_payloads_and_seeded_source_reopening_are_supported() {
    struct Item(u8);
    let items = [Item(3), Item(5)];
    let mut cursor = LazyWeightedInterleave::new(vec![(items.iter(), 1.0)], || 0.0).unwrap();
    assert_eq!(
        cursor.by_ref().map(|x| x.unwrap().0).collect::<Vec<_>>(),
        [3, 5]
    );
    let log = Rc::new(RefCell::new(vec![]));
    let mut union = SelectionUnion::new(
        (0..3)
            .map(|id| {
                (
                    Source {
                        id,
                        log: log.clone(),
                    },
                    1.0,
                )
            })
            .collect(),
    )
    .unwrap();
    let first = union
        .random_lazy(&(), 42)
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let second = union
        .random_lazy(&(), 42)
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(first, second);
    assert_eq!(first.len(), 3);
}
