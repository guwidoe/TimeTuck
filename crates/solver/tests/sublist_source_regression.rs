//! Original composition, application/undo and lifecycle checks.
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use timetuck::*;
struct Source<T> {
    rows: Vec<T>,
    events: Rc<RefCell<Vec<SelectionLifecycleEvent>>>,
    opens: Rc<Cell<usize>>,
}
impl<T> Source<T> {
    fn new(rows: Vec<T>) -> Self {
        Self {
            rows,
            events: Default::default(),
            opens: Default::default(),
        }
    }
}
impl<C, T: Clone> SelectionSource<C, T> for Source<T> {
    fn event(&mut self, e: SelectionLifecycleEvent, _: &C) {
        self.events.borrow_mut().push(e);
    }
    fn size(&mut self, _: &C) -> Option<usize> {
        Some(self.rows.len())
    }
    fn open<'a>(&'a mut self, _: &'a C) -> Box<dyn Iterator<Item = T> + 'a>
    where
        T: 'a,
    {
        self.opens.set(self.opens.get() + 1);
        Box::new(self.rows.iter().cloned())
    }
}
fn span(e: usize, s: usize, t: usize) -> ListRange {
    ListRange {
        entity: e,
        start: s,
        end: t,
    }
}
fn pos(e: usize, i: usize) -> Option<ListPosition> {
    Some(ListPosition {
        entity: e,
        index: i,
    })
}
fn model() -> ListProblem<i32> {
    ListProblem::new(
        vec![1, 2, 3, 4, 5],
        vec![
            ListEntity::new(vec![1, 2, 3, 4]),
            ListEntity::new(vec![5]),
            ListEntity::new(vec![]),
        ],
    )
    .unwrap()
    .allowing_unassigned()
}
#[test]
fn change_pairs_lazily_routes_events_and_skips_unassignment_randomness() {
    let left = Source::new(vec![span(0, 0, 2), span(0, 1, 3), span(0, 2, 4)]);
    let right = Source::new(vec![None, pos(1, 1)]);
    let le = left.events.clone();
    let re = right.events.clone();
    let lo = left.opens.clone();
    let ro = right.opens.clone();
    let draws = Rc::new(Cell::new(0));
    let d = draws.clone();
    let mut s = SubListChangeSource::with_draw(left, right, true, move |n| {
        assert_eq!(n, 2);
        d.set(d.get() + 1);
        1
    });
    use SelectionLifecycleEvent::*;
    let events = [
        SolveStarted,
        PhaseStarted,
        StepStarted,
        StepEnded,
        StepStarted,
        StepEnded,
        PhaseEnded,
        SolveEnded,
    ];
    for e in events {
        s.event(e, &());
    }
    assert_eq!(*le.borrow(), events);
    assert_eq!(*re.borrow(), events);
    {
        let _unused = s.open(&());
    }
    assert_eq!((lo.get(), ro.get(), draws.get()), (0, 0, 0));
    let mut i = s.open(&());
    assert_eq!(
        i.next(),
        Some(SubListChangeProposal {
            source: span(0, 0, 2),
            destination: None,
            reverse: false
        })
    );
    assert_eq!(
        i.next(),
        Some(SubListChangeProposal {
            source: span(0, 1, 3),
            destination: pos(1, 1),
            reverse: true
        })
    );
    assert_eq!(i.next(), None);
    assert_eq!(i.next(), None);
    assert_eq!((lo.get(), ro.get(), draws.get()), (1, 1, 1));
}
#[test]
fn pre_removal_conversion_preserves_relocation_unassignment_and_undo() {
    let p = model();
    let a = p.initial_assignment();
    let mut trial = a.clone();
    for (destination, reverse, expected) in [
        (pos(0, 4), false, vec![vec![1, 4, 2, 3], vec![5], vec![]]),
        (pos(0, 0), true, vec![vec![3, 2, 1, 4], vec![5], vec![]]),
        (pos(0, 3), true, vec![vec![1, 3, 2, 4], vec![5], vec![]]),
        (pos(1, 1), false, vec![vec![1, 4], vec![5, 2, 3], vec![]]),
        (None, true, vec![vec![1, 4], vec![5], vec![]]),
    ] {
        let m = SubListChangeProposal {
            source: span(0, 1, 3),
            destination,
            reverse,
        }
        .into_move()
        .unwrap();
        {
            let tx = p.apply(&mut trial, &m).unwrap();
            assert_eq!(tx.lists(), expected);
        }
        assert_eq!(trial, a);
    }
    assert!(
        SubListChangeProposal {
            source: span(0, 1, 3),
            destination: pos(0, 2),
            reverse: false
        }
        .into_move::<i32>()
        .is_none()
    );
    assert!(
        SubListChangeProposal {
            source: span(0, 3, 1),
            destination: None,
            reverse: false
        }
        .into_move::<i32>()
        .is_none()
    );
}
#[test]
fn swaps_normalize_only_same_entity_order_and_retain_overlap_for_validation() {
    let mut s = SubListSwapSource::with_draw(
        Source::new(vec![span(0, 2, 4), span(1, 0, 1), span(0, 0, 3)]),
        Source::new(vec![span(0, 0, 1), span(0, 1, 2), span(0, 1, 3)]),
        true,
        |n| {
            assert_eq!(n, 2);
            1
        },
    );
    let p = model();
    let a = p.initial_assignment();
    let results = s.open(&()).collect::<Vec<_>>();
    assert_eq!(results[0].left, span(0, 0, 1));
    assert_eq!(results[0].right, span(0, 2, 4));
    assert_eq!(results[1].left, span(1, 0, 1));
    assert_eq!(results[1].right, span(0, 1, 2));
    assert!(results.iter().all(|m| m.reverse));
    let mut trial = a.clone();
    for m in &results[..2] {
        {
            let _tx = p.apply(&mut trial, &m.into_move()).unwrap();
        }
        assert_eq!(trial, a);
    }
    assert!(p.apply(&mut trial, &results[2].into_move()).is_err());
    assert_eq!(trial, a);
}
#[test]
fn entity_source_bounds_rejected_reads_and_refreshes_after_commit() {
    let p = ListProblem::new(
        vec![1, 2, 3, 4, 5],
        vec![
            ListEntity::new(vec![1, 2, 3]).with_pinned_prefix(1),
            ListEntity::new(vec![4]).with_pinned(true),
            ListEntity::new(vec![5]),
            ListEntity::new(vec![]),
        ],
    )
    .unwrap();
    let mut a = p.initial_assignment();
    let bounds = Rc::new(RefCell::new(vec![]));
    let b = bounds.clone();
    let mut s = EntitySubListSource::with_draw(
        Source::new(vec![1, usize::MAX, 3, 2, 0, 0]),
        SubListSize::new(2, 3).unwrap(),
        5,
        move |n| {
            b.borrow_mut().push(n);
            0
        },
    );
    {
        let c = ListSelectionContext::new(&p, &a).unwrap();
        assert_eq!(s.span_count(&c), 1);
        assert_eq!(s.open(&c).collect::<Vec<_>>(), [span(0, 1, 3)]);
    }
    assert_eq!(*bounds.borrow(), [1]);
    p.apply(
        &mut a,
        &ListMove::SubListChange {
            entity: 0,
            start: 1,
            end: 3,
            destination: ListPosition {
                entity: 3,
                index: 0,
            },
            reverse: false,
        },
    )
    .unwrap()
    .commit();
    let c = ListSelectionContext::new(&p, &a).unwrap();
    assert_eq!(s.open(&c).collect::<Vec<_>>(), [span(3, 0, 2)]);
    let opens = s.entities().opens.clone();
    let before = opens.get();
    // Separate empty source avoids opening a child even when it advertises work.
    let mut empty = EntitySubListSource::new(
        Source::new(vec![0]),
        SubListSize::new(10, 20).unwrap(),
        0,
        0,
    );
    assert_eq!(empty.size(&c), Some(0));
    assert_eq!(empty.open(&c).count(), 0);
    assert_eq!(empty.entities().opens.get(), 0);
    assert_eq!(opens.get(), before);
}
#[test]
fn bounded_sublist_sources_compose_with_recording_and_replay() {
    let p = model();
    let a = p.initial_assignment();
    let c = ListSelectionContext::new(&p, &a).unwrap();
    let spans = EntitySubListSource::new(
        Source::new(vec![0, 1, 0]),
        SubListSize::new(2, 2).unwrap(),
        3,
        17,
    );
    let changes =
        SubListChangeSource::new(spans, Source::new(vec![pos(2, 0), pos(1, 1)]), false, 18);
    let mut registry = SelectionRegistry::new();
    let mut recorder = registry.register("moves", changes).unwrap();
    let replay = registry.replay("moves").unwrap();
    use SelectionLifecycleEvent::*;
    for e in [SolveStarted, PhaseStarted, StepStarted] {
        recorder.event(e, &c);
    }
    let mut observed = vec![];
    {
        let mut i = recorder.open(&c);
        for _ in 0..2 {
            observed.push(i.next().unwrap());
        }
    }
    assert_eq!(observed.len(), 2);
    // Recording stores only the latest publication, not a history.
    assert_eq!(
        replay.cursor().next_replayed().unwrap().as_deref(),
        observed.last().map(|x| &**x)
    );
    for m in observed {
        let movement = (*m).into_move::<i32>().unwrap();
        let mut trial = a.clone();
        {
            let _tx = p.apply(&mut trial, &movement).unwrap();
        }
        assert_eq!(trial, a);
    }
    for e in [StepEnded, PhaseEnded, SolveEnded] {
        recorder.event(e, &c);
    }
    assert_eq!(
        replay.cursor().next_replayed(),
        Err(SelectionReplayError::NotRecorded)
    );
}
fn source_moves(p: &ListProblem<i32>, a: &ListAssignment<i32>, _: u64) -> Vec<ListMove<i32>> {
    let c = ListSelectionContext::new(p, a).unwrap();
    let spans = EntitySubListSource::with_draw(
        Source::new(vec![1, 0]),
        SubListSize::new(2, 2).unwrap(),
        2,
        |_| 0,
    );
    let mut source = SubListChangeSource::new(spans, Source::new(vec![pos(1, 0)]), true, 17);
    source
        .open(&c)
        .filter_map(SubListChangeProposal::into_move)
        .collect()
}
#[test]
fn composed_sources_feed_list_and_mixed_solver_phases() {
    let p = ListProblem::new(
        vec![1, 2, 3, 4],
        vec![ListEntity::new(vec![1, 2, 3, 4]), ListEntity::new(vec![])],
    )
    .unwrap();
    let mut n = ListConstraintNetwork::new(2, 4);
    n.add_constraint(
        "balance",
        vec![ListScope::Entity(0), ListScope::Entity(1)],
        |rows| {
            let [ListFact::Entity(l), ListFact::Entity(r)] = rows else {
                panic!("entities")
            };
            SimpleScore::new(-(l.len() as i64 - r.len() as i64).abs())
        },
    )
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
    assert_eq!(r.best.unwrap().score, SimpleScore::ZERO);
    let m = MixedProblem::new(
        FiniteProblem::new(vec![
            Variable::new(vec![7]).with_initial(7).with_pinned(true),
        ])
        .unwrap(),
        p,
    );
    let mut n = MixedConstraintNetwork::new(1, 2, 4);
    n.add_constraint(
        "balance",
        vec![MixedScope::ListEntity(0), MixedScope::ListEntity(1)],
        |rows| {
            let [MixedFact::ListEntity(l), MixedFact::ListEntity(r)] = rows else {
                panic!("entities")
            };
            SimpleScore::new(-(l.len() as i64 - r.len() as i64).abs())
        },
    )
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
    assert_eq!(r.score, Some(SimpleScore::ZERO));
}
#[test]
fn endless_ineligible_owners_stop_at_budget_and_bad_draws_fail() {
    struct Endless(Rc<Cell<usize>>);
    impl<C> SelectionSource<C, usize> for Endless {
        fn is_never_ending(&self) -> bool {
            true
        }
        fn size(&mut self, _: &C) -> Option<usize> {
            Some(1)
        }
        fn open<'a>(&'a mut self, _: &'a C) -> Box<dyn Iterator<Item = usize> + 'a>
        where
            usize: 'a,
        {
            Box::new(std::iter::repeat_with(|| {
                self.0.set(self.0.get() + 1);
                1
            }))
        }
    }
    let p = model();
    let a = p.initial_assignment();
    let c = ListSelectionContext::new(&p, &a).unwrap();
    let reads = Rc::new(Cell::new(0));
    let mut s = EntitySubListSource::with_draw(
        Endless(reads.clone()),
        SubListSize::new(2, 2).unwrap(),
        7,
        |_| panic!("too short must not draw"),
    );
    assert_eq!(s.open(&c).count(), 0);
    assert_eq!(reads.get(), 7);
    use std::panic::{AssertUnwindSafe, catch_unwind};
    let mut s = EntitySubListSource::with_draw(
        Source::new(vec![0]),
        SubListSize::new(2, 2).unwrap(),
        1,
        |n| n,
    );
    assert!(catch_unwind(AssertUnwindSafe(|| s.open(&c).next())).is_err());
    let mut s = SubListChangeSource::with_draw(
        Source::new(vec![span(0, 0, 2)]),
        Source::new(vec![pos(1, 0)]),
        true,
        |n| n,
    );
    assert!(catch_unwind(AssertUnwindSafe(|| s.open(&()).next())).is_err());
}
