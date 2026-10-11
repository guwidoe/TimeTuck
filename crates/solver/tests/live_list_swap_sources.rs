// Original native regressions; no upstream source credit.
use std::{
    cell::{Cell, RefCell},
    num::NonZeroUsize,
    rc::Rc,
};
use timetuck::*;
#[derive(Clone, Default)]
struct Counts {
    opens: Rc<Cell<usize>>,
    reads: Rc<Cell<usize>>,
    events: Rc<RefCell<Vec<(u8, SelectionLifecycleEvent)>>>,
}
struct Values {
    values: Vec<i32>,
    repeat: bool,
    tag: u8,
    counts: Counts,
}
impl Values {
    fn new(values: Vec<i32>, repeat: bool) -> Self {
        Self {
            values,
            repeat,
            tag: 0,
            counts: Counts::default(),
        }
    }
}
impl<C> SelectionSource<C, i32> for Values {
    fn is_never_ending(&self) -> bool {
        self.repeat
    }
    fn size(&mut self, _: &C) -> Option<usize> {
        (!self.repeat).then_some(self.values.len())
    }
    fn event(&mut self, e: SelectionLifecycleEvent, _: &C) {
        self.counts.events.borrow_mut().push((self.tag, e));
    }
    fn open<'a>(&'a mut self, _: &'a C) -> Box<dyn Iterator<Item = i32> + 'a>
    where
        i32: 'a,
    {
        self.counts.opens.set(self.counts.opens.get() + 1);
        let mut i = 0;
        Box::new(std::iter::from_fn(move || {
            self.counts.reads.set(self.counts.reads.get() + 1);
            if self.values.is_empty() {
                return None;
            }
            let v = self
                .values
                .get(if self.repeat {
                    i % self.values.len()
                } else {
                    i
                })
                .copied();
            i += 1;
            v
        }))
    }
}
fn limits(reads: usize, pairs: usize) -> ListSwapSourceLimits {
    ListSwapSourceLimits {
        reads_per_side: NonZeroUsize::new(reads).unwrap(),
        pairs_per_item: NonZeroUsize::new(pairs).unwrap(),
    }
}
fn problem() -> ListProblem<i32> {
    ListProblem::new(
        vec![1, 2, 3, 4],
        vec![
            ListEntity::new(vec![1, 2]).with_pinned_prefix(1),
            ListEntity::new(vec![3]).with_pinned(true),
            ListEntity::new(vec![]),
        ],
    )
    .unwrap()
    .allowing_unassigned()
}
fn repeats<C, T>(s: &impl SelectionSource<C, T>, _: &C) -> bool {
    s.is_never_ending()
}
#[test]
fn unopened_and_partial_cursors_are_lazy_and_productive_streams_remain_bounded_per_item() {
    let p = problem();
    let a = p.initial_assignment();
    let c = ListSelectionContext::new(&p, &a).unwrap();
    let l = Values::new(vec![1, 2], true);
    let lc = l.counts.clone();
    let r = Values::new(vec![3, 4], true);
    let rc = r.counts.clone();
    let mut s = ListValueSwapSource::new(l, r, limits(2, 1), false);
    assert!(repeats(&s, &c));
    assert_eq!(s.size(&c), None);
    drop(s.open(&c));
    assert_eq!((lc.opens.get(), rc.opens.get()), (0, 0));
    assert_eq!(s.open(&c).take(10_000).count(), 10_000);
    assert_eq!((lc.reads.get(), rc.reads.get()), (20_000, 20_000));
    assert_eq!((lc.opens.get(), rc.opens.get()), (1, 1));
    assert!(s.open(&c).next().is_some());
    assert_eq!((lc.reads.get(), rc.reads.get()), (20_002, 20_002));
}
#[test]
fn side_and_pair_rejection_limits_fuse_without_consuming_more_children() {
    let p = problem();
    let a = p.initial_assignment();
    let c = ListSelectionContext::new(&p, &a).unwrap();
    let l = Values::new(vec![1], true);
    let lc = l.counts.clone();
    let r = Values::new(vec![4], true);
    let rc = r.counts.clone();
    let mut s = ListValueSwapSource::new(l, r, limits(3, 7), true);
    let mut i = s.open(&c);
    assert_eq!(i.next(), None);
    assert_eq!(i.next(), None);
    assert_eq!((lc.reads.get(), rc.reads.get()), (3, 0));
    drop(i);
    assert_eq!(s.open(&c).next(), None);
    assert_eq!(lc.reads.get(), 6);
    let p = ListProblem::from_entity_ranges(vec![
        ListEntity::new(vec![1]).with_value_range(vec![1]),
        ListEntity::new(vec![2]).with_value_range(vec![2]),
    ])
    .unwrap();
    let a = p.initial_assignment();
    let c = ListSelectionContext::new(&p, &a).unwrap();
    let l = Values::new(vec![1], true);
    let lc = l.counts.clone();
    let r = Values::new(vec![2], true);
    let rc = r.counts.clone();
    let mut s = ListValueSwapSource::new(l, r, limits(2, 4), true);
    let mut i = s.open(&c);
    assert_eq!(i.next(), None);
    assert_eq!(i.next(), None);
    assert_eq!((lc.reads.get(), rc.reads.get()), (4, 4));
}
#[test]
fn finite_children_metadata_lifecycle_unknown_values_and_panics() {
    let p = problem();
    let a = p.initial_assignment();
    let c = ListSelectionContext::new(&p, &a).unwrap();
    let l = Values::new(vec![2, 2], false);
    let r = Values::new(vec![4], true);
    let counts = l.counts.clone();
    let mut s = ListValueSwapSource::new(l, r, limits(2, 2), false);
    assert!(!repeats(&s, &c));
    assert_eq!(s.size(&c), None);
    s.left_mut().tag = 1;
    s.right_mut().tag = 2;
    s.right_mut().counts.events = counts.events.clone();
    use SelectionLifecycleEvent::*;
    let events = [
        SolveStarted,
        PhaseStarted,
        StepStarted,
        StepEnded,
        PhaseEnded,
        SolveEnded,
    ];
    for e in events {
        s.event(e, &c);
    }
    assert_eq!(
        *counts.events.borrow(),
        events
            .into_iter()
            .flat_map(|e| [(1, e), (2, e)])
            .collect::<Vec<_>>()
    );
    let mut i = s.open(&c);
    assert!(i.next().is_some());
    assert!(i.next().is_some());
    assert_eq!(i.next(), None);
    assert_eq!(i.next(), None);
    drop(i);
    assert_eq!(counts.reads.get(), 3);
    s.left_mut().values = vec![];
    assert_eq!(s.size(&c), Some(0));
    s.left_mut().values = vec![99];
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| s.open(&c).next())).is_err());
    assert_eq!(a, p.initial_assignment());
    s.left_mut().values = vec![2];
    assert!(s.open(&c).next().is_some());
}
#[test]
fn paired_reference_covers_pins_optional_values_ranges_and_noops() {
    for optional in [false, true] {
        for noops in [false, true] {
            for pinned in [false, true] {
                let mut p = ListProblem::new(
                    vec![1, 2, 3, 4],
                    vec![
                        ListEntity::new(vec![1, 2])
                            .with_value_range(vec![1, 2, 4])
                            .with_pinned_prefix(usize::from(pinned)),
                        ListEntity::new(vec![3]).with_value_range(vec![2, 3, 4]),
                    ],
                )
                .unwrap();
                if optional {
                    p = p.allowing_unassigned();
                }
                let a = p.initial_assignment();
                let c = ListSelectionContext::new(&p, &a).unwrap();
                for left in [[1, 2, 3, 4], [4, 3, 2, 1], [2, 2, 2, 2]] {
                    for right in [[1, 2, 3, 4], [4, 3, 2, 1], [3, 3, 4, 4]] {
                        let expected: Vec<_> = list_value_swap_moves(
                            &p,
                            &a,
                            &left,
                            &right,
                            ListSwapPairing::Paired,
                            noops,
                        )
                        .unwrap()
                        .collect();
                        let mut source = ListValueSwapSource::new(
                            Values::new(left.to_vec(), false),
                            Values::new(right.to_vec(), false),
                            limits(8, 8),
                            noops,
                        );
                        let actual: Vec<_> = source.open(&c).collect();
                        assert_eq!(actual, expected);
                        for m in actual {
                            let mut trial = a.clone();
                            drop(p.apply(&mut trial, &m).unwrap());
                            assert_eq!(trial, a);
                        }
                    }
                }
            }
        }
    }
}
#[test]
fn dependencies_reject_pairs_and_reopening_refreshes_positions() {
    let p = ListProblem::new(vec![1, 2, 3], vec![ListEntity::new(vec![1, 2, 3])])
        .unwrap()
        .with_dependencies(vec![vec![], vec![0], vec![1]])
        .unwrap();
    let a = p.initial_assignment();
    let c = ListSelectionContext::new(&p, &a).unwrap();
    let mut s = ListValueSwapSource::new(
        Values::new(vec![1, 2], false),
        Values::new(vec![3, 2], false),
        limits(2, 2),
        true,
    );
    assert_eq!(
        s.open(&c).collect::<Vec<_>>(),
        vec![ListMove::Composite(vec![])]
    );
    let p = ListProblem::new(
        vec![1, 2, 3],
        vec![ListEntity::new(vec![1, 2]), ListEntity::new(vec![3])],
    )
    .unwrap();
    let mut a = p.initial_assignment();
    let mut s = ListValueSwapSource::new(
        Values::new(vec![1], false),
        Values::new(vec![3], false),
        limits(1, 1),
        false,
    );
    let m = s
        .open(&ListSelectionContext::new(&p, &a).unwrap())
        .next()
        .unwrap();
    p.apply(&mut a, &m).unwrap().commit();
    let next = s
        .open(&ListSelectionContext::new(&p, &a).unwrap())
        .next()
        .unwrap();
    let ListMove::Swap { left, right } = m else {
        panic!()
    };
    assert_eq!(
        next,
        ListMove::Swap {
            left: right,
            right: left
        }
    );
}
fn source_moves(p: &ListProblem<i32>, a: &ListAssignment<i32>, _: u64) -> Vec<ListMove<i32>> {
    let c = ListSelectionContext::new(p, a).unwrap();
    let source = ListValueSwapSource::new(
        Values::new(vec![1], true),
        Values::new(vec![3], true),
        limits(2, 2),
        false,
    );
    LimitedSelectionSource::new(source, 2).open(&c).collect()
}
#[test]
fn bounded_sources_drive_list_and_mixed_solver_phases() {
    let p = ListProblem::new(
        vec![1, 2, 3],
        vec![ListEntity::new(vec![1, 2]), ListEntity::new(vec![3])],
    )
    .unwrap();
    let mut n = ListConstraintNetwork::new(2, 3);
    n.add_constraint("first", vec![ListScope::Entity(0)], |rows| {
        let ListFact::Entity(row) = rows[0] else {
            panic!()
        };
        SimpleScore::new(i64::from(row[0]))
    })
    .unwrap();
    let r = list_local_search_from_with_provider(
        &p,
        &n,
        p.initial_assignment(),
        LocalSearchConfig {
            max_steps: 1,
            assert_scores: true,
            ..Default::default()
        },
        &mut source_moves,
    )
    .unwrap();
    assert_eq!(r.best.unwrap().score, SimpleScore::new(3));
    let m = MixedProblem::new(
        FiniteProblem::new(vec![
            Variable::new(vec![7]).with_initial(7).with_pinned(true),
        ])
        .unwrap(),
        p,
    );
    let mut n = MixedConstraintNetwork::new(1, 2, 3);
    n.add_constraint("first", vec![MixedScope::ListEntity(0)], |rows| {
        let MixedFact::ListEntity(row) = rows[0] else {
            panic!()
        };
        SimpleScore::new(i64::from(row[0]))
    })
    .unwrap();
    let mut provider = |p: &MixedProblem<i32, i32>, a: &MixedAssignment<i32, i32>, step| {
        source_moves(p.lists(), a.lists(), step)
            .into_iter()
            .map(|lists| MixedMove {
                basic: Move::Composite(vec![]),
                lists,
            })
            .collect::<Vec<_>>()
    };
    let mut ext = SolverExtensions::default();
    ext.move_providers.bind(0, &mut provider);
    let r = solve_mixed_with_extensions(
        &m,
        &n,
        MixedSolverSequence {
            phases: vec![MixedSolverPhase::LocalSearch(MixedLocalSearchConfig {
                search: LocalSearchConfig {
                    max_steps: 1,
                    assert_scores: true,
                    ..Default::default()
                },
                ..Default::default()
            })],
            ..Default::default()
        },
        ext,
    )
    .unwrap();
    assert_eq!(r.score, Some(SimpleScore::new(3)));
    assert_eq!(r.assignment.basic().values(), &[Some(7)]);
}
