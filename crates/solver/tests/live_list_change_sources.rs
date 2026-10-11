//! Original lazy list-change contracts and actual provider/score execution.
use std::{
    cell::{Cell, RefCell},
    num::NonZeroUsize,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
    sync::Arc,
};
use timetuck::*;
#[derive(Clone, Default)]
struct Counts {
    opens: Rc<Cell<usize>>,
    reads: Rc<Cell<usize>>,
    events: Rc<RefCell<Vec<SelectionLifecycleEvent>>>,
    fail_open: Rc<Cell<bool>>,
    fail_read: Rc<Cell<bool>>,
}
struct Script<T> {
    items: Vec<T>,
    repeat: bool,
    counts: Counts,
}
impl<T> Script<T> {
    fn new(items: Vec<T>, repeat: bool) -> Self {
        Self {
            items,
            repeat,
            counts: Counts::default(),
        }
    }
}
impl<C, T: Clone> SelectionSource<C, T> for Script<T> {
    fn size(&mut self, _: &C) -> Option<usize> {
        (!self.repeat || self.items.is_empty()).then_some(self.items.len())
    }
    fn is_never_ending(&self) -> bool {
        self.repeat
    }
    fn event(&mut self, e: SelectionLifecycleEvent, _: &C) {
        self.counts.events.borrow_mut().push(e);
    }
    fn open<'a>(&'a mut self, _: &'a C) -> Box<dyn Iterator<Item = T> + 'a>
    where
        T: 'a,
    {
        self.counts.opens.set(self.counts.opens.get() + 1);
        assert!(!self.counts.fail_open.get(), "open");
        let mut index = 0;
        Box::new(std::iter::from_fn(move || {
            self.counts.reads.set(self.counts.reads.get() + 1);
            assert!(!self.counts.fail_read.get(), "read");
            if self.items.is_empty() {
                return None;
            }
            let at = if self.repeat {
                index % self.items.len()
            } else {
                index
            };
            index += 1;
            self.items.get(at).cloned()
        }))
    }
}
fn limits(reads: usize, pairs: usize) -> ListChangeSourceLimits {
    ListChangeSourceLimits {
        reads_per_side: NonZeroUsize::new(reads).unwrap(),
        pairs_per_item: NonZeroUsize::new(pairs).unwrap(),
    }
}
fn at(entity: usize, index: usize) -> Option<ListPosition> {
    Some(ListPosition { entity, index })
}
fn pinned() -> ListProblem<i32> {
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
#[test]
fn first_pull_is_lazy_and_each_side_skips_pins_before_pairing() {
    let p = pinned();
    let a = p.initial_assignment();
    let c = ListSelectionContext::new(&p, &a).unwrap();
    let left = Script::new(vec![1, 2, 3, 4], true);
    let right = Script::new(vec![at(1, 0), at(0, 0), at(2, 0), None], true);
    let l = left.counts.clone();
    let r = right.counts.clone();
    let mut source = ListValueChangeSource::new(left, right, limits(3, 1), false);
    source.event(SelectionLifecycleEvent::SolveStarted, &c);
    source.event(SelectionLifecycleEvent::PhaseStarted, &c);
    assert_eq!(*l.events.borrow(), *r.events.borrow());
    assert_eq!(l.events.borrow().len(), 2);
    drop(source.open(&c));
    assert_eq!((l.opens.get(), r.opens.get()), (0, 0));
    let mut cursor = source.open(&c);
    let first = cursor.next().unwrap();
    assert_eq!(*first.value(), 2);
    assert_eq!(first.destination(), at(2, 0));
    assert_eq!((l.reads.get(), r.reads.get()), (2, 3));
    // Missing 4 paired with unassignment is a no-op and rejected at the one-pair limit.
    assert!(cursor.next().is_none());
    assert_eq!((l.reads.get(), r.reads.get()), (4, 4));
    assert!(cursor.next().is_none());
    assert_eq!((l.reads.get(), r.reads.get()), (4, 4));
    drop(cursor);
    assert_eq!(*source.open(&c).next().unwrap().value(), 2);
}
#[test]
fn infinite_rejections_and_exhaustion_have_exact_read_bounds() {
    let p = pinned();
    let a = p.initial_assignment();
    let c = ListSelectionContext::new(&p, &a).unwrap();
    for (values, destinations, expected) in [
        (vec![1], vec![at(2, 0)], (3, 0)),
        (vec![2], vec![at(1, 0)], (1, 3)),
        (vec![2], vec![at(0, 2)], (2, 2)),
        (vec![], vec![at(2, 0)], (1, 0)),
        (vec![2], vec![], (1, 1)),
    ] {
        let left = Script::new(values, true);
        let right = Script::new(destinations, true);
        let l = left.counts.clone();
        let r = right.counts.clone();
        let mut source = ListValueChangeSource::new(left, right, limits(3, 2), false);
        let mut cursor = source.open(&c);
        assert!(cursor.next().is_none());
        assert!(cursor.next().is_none());
        assert_eq!((l.reads.get(), r.reads.get()), expected);
    }
}
#[test]
fn finite_pairs_match_existing_enumerator_across_ranges_pins_dependencies_and_noops() {
    let models = [
        pinned(),
        ListProblem::new(
            vec![1, 2, 3, 4],
            vec![
                ListEntity::new(vec![1, 2]).with_value_range(vec![1, 2]),
                ListEntity::new(vec![3]).with_value_range(vec![2, 3, 4]),
            ],
        )
        .unwrap()
        .allowing_unassigned(),
        ListProblem::new(vec![1, 2, 3], vec![ListEntity::new(vec![1, 2, 3])])
            .unwrap()
            .with_dependencies(vec![vec![], vec![0], vec![1]])
            .unwrap(),
    ];
    for p in models {
        let a = p.initial_assignment();
        let c = ListSelectionContext::new(&p, &a).unwrap();
        let values: Vec<_> = p.values().iter().rev().chain(p.values()).copied().collect();
        let mut dest: Vec<_> = a
            .lists()
            .iter()
            .enumerate()
            .flat_map(|(entity, row)| (0..=row.len()).map(move |index| at(entity, index)))
            .collect();
        if p.allows_unassigned() {
            dest.push(None)
        }
        dest.extend(dest.clone());
        for noops in [false, true] {
            let expected =
                list_value_changes(&p, &a, &values, &dest, ListChangePairing::Paired, noops)
                    .unwrap()
                    .collect::<Vec<_>>();
            let mut source = ListValueChangeSource::new(
                Script::new(values.clone(), false),
                Script::new(dest.clone(), false),
                limits(32, 32),
                noops,
            );
            assert_eq!(source.open(&c).collect::<Vec<_>>(), expected);
            for candidate in expected {
                let mut trial = a.clone();
                drop(p.apply(&mut trial, candidate.movement()).unwrap());
                assert_eq!(trial, a);
            }
        }
    }
}
#[test]
fn malformed_metadata_and_callback_panics_fuse_then_reopen_safely() {
    let p = pinned();
    let a = p.initial_assignment();
    let c = ListSelectionContext::new(&p, &a).unwrap();
    for failure in 0..4 {
        let left = Script::new(vec![if failure == 0 { 999 } else { 2 }], true);
        let right = Script::new(vec![if failure == 1 { at(99, 0) } else { at(2, 0) }], true);
        let l = left.counts.clone();
        let r = right.counts.clone();
        r.fail_open.set(failure == 2);
        r.fail_read.set(failure == 3);
        let mut source = ListValueChangeSource::new(left, right, limits(2, 2), false);
        let mut cursor = source.open(&c);
        assert!(catch_unwind(AssertUnwindSafe(|| cursor.next())).is_err());
        let before = (l.reads.get(), r.reads.get());
        assert!(cursor.next().is_none());
        assert_eq!((l.reads.get(), r.reads.get()), before);
        drop(cursor);
        source.values_mut().items = vec![2];
        source.destinations_mut().items = vec![at(2, 0)];
        r.fail_open.set(false);
        r.fail_read.set(false);
        assert_eq!(*source.open(&c).next().unwrap().value(), 2);
        assert_eq!(a, p.initial_assignment());
    }
}
fn network() -> ListConstraintNetwork<'static, i32, SimpleScore> {
    let mut n = ListConstraintNetwork::new(2, 3);
    n.add_constraint(
        "rows",
        vec![ListScope::Entity(0), ListScope::Entity(1)],
        |facts| {
            let [ListFact::Entity(a), ListFact::Entity(b)] = facts else {
                panic!()
            };
            SimpleScore::new(
                a.iter().map(|v| i64::from(*v)).sum::<i64>()
                    + 10 * b.iter().map(|v| i64::from(*v)).sum::<i64>(),
            )
        },
    )
    .unwrap();
    n
}
#[test]
fn change_assign_unassign_and_noop_metadata_apply_score_and_undo() {
    let p = ListProblem::new(
        vec![1, 2, 3],
        vec![ListEntity::new(vec![1, 2]), ListEntity::new(vec![])],
    )
    .unwrap()
    .allowing_unassigned();
    let a = p.initial_assignment();
    let c = ListSelectionContext::new(&p, &a).unwrap();
    let n = network();
    let mut source = ListValueChangeSource::new(
        Script::new(vec![1, 2, 3, 3, 1], false),
        Script::new(vec![at(0, 2), None, at(1, 0), None, at(0, 1)], false),
        limits(1, 1),
        true,
    );
    let selected = source.open(&c).collect::<Vec<_>>();
    assert_eq!(selected.len(), 5);
    assert_eq!(selected[0].destination(), at(0, 2));
    assert_eq!(
        *selected[0].movement(),
        ListMove::Change {
            source: at(0, 0).unwrap(),
            destination: at(0, 1).unwrap()
        }
    );
    let mut director = ListScoreDirector::new(&p, &n, a.clone()).unwrap();
    for (candidate, score) in selected.iter().zip([3, 1, 33, 3, 3]) {
        let trial = director.apply(candidate.movement()).unwrap();
        assert_eq!(trial.score(), SimpleScore::new(score));
        drop(trial);
        assert_eq!(director.assignment(), &a);
        assert_eq!(director.score(), SimpleScore::new(3));
    }
    let movement = selected[0].movement().clone();
    let mut changed = a.clone();
    p.apply(&mut changed, &movement).unwrap().commit();
    let next_context = ListSelectionContext::new(&p, &changed).unwrap();
    assert_eq!(
        source.open(&next_context).next().unwrap().source(),
        at(0, 1)
    );
    director.assert_consistent().unwrap();
}
#[test]
fn configured_provider_reopens_with_current_assignment_and_bounds_repeating_children() {
    let p = ListProblem::new(
        vec![1, 2, 3],
        vec![ListEntity::new(vec![1, 2]), ListEntity::new(vec![3])],
    )
    .unwrap();
    let shared = Arc::new(p.clone());
    let n = network();
    let captures = Rc::new(Cell::new(0));
    let observed = captures.clone();
    let left = Script::new(vec![1, 2], true);
    let right = Script::new(vec![at(1, 0)], true);
    let lc = left.counts.clone();
    let rc = right.counts.clone();
    let source = ListValueChangeSource::new(left, right, limits(1, 2), false);
    let selection = SelectionBuilder::new(SelectionConfig {
        order: Some(SelectionOrder::Original),
        ..Default::default()
    })
    .build(ConfiguredSelectionPolicy::default(), |_| source)
    .unwrap();
    let metadata = Rc::new(RefCell::new(vec![]));
    let recorded = metadata.clone();
    let mut provider = ConfiguredMoveProvider::new(
        selection,
        ConfiguredProviderConfig {
            maximum_draws: 2,
            seed: 0,
        },
        move |_: &ListProblem<i32>, a: &ListAssignment<i32>, _: u64| {
            observed.set(observed.get() + 1);
            (shared.clone(), a.clone())
        },
        move |_: &ListProblem<i32>,
              _: &ListAssignment<i32>,
              _: u64,
              candidate: &ListValueChange<i32>| {
            recorded
                .borrow_mut()
                .push((*candidate.value(), candidate.source()));
            Some(candidate.movement().clone())
        },
    );
    let result = list_local_search_from_with_provider(
        &p,
        &n,
        p.initial_assignment(),
        LocalSearchConfig {
            max_steps: 2,
            selection: MoveSelection::BestAccepted,
            assert_scores: true,
            ..Default::default()
        },
        &mut provider,
    )
    .unwrap();
    let best = result.best.unwrap();
    assert_eq!(best.score, SimpleScore::new(60));
    assert_eq!(best.assignment.lists(), &[vec![], vec![1, 2, 3]]);
    assert_eq!(captures.get(), 2);
    assert_eq!(lc.opens.get(), 2);
    assert_eq!(rc.opens.get(), 2);
    assert_eq!(
        *metadata.borrow(),
        [(1, at(0, 0)), (2, at(0, 1)), (1, at(0, 0)), (1, at(0, 0))]
    );
    assert_eq!(lc.reads.get(), 5);
    assert_eq!(rc.reads.get(), 5);
    assert_eq!(*lc.events.borrow(), *rc.events.borrow());
    assert!(
        lc.events
            .borrow()
            .contains(&SelectionLifecycleEvent::SolveEnded)
    );
}
