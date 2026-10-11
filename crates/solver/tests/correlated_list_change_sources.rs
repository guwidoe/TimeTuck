//! Original correlated destination factory contracts and solver execution.
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
fn limits(values: usize, destinations: usize) -> CorrelatedListChangeLimits {
    CorrelatedListChangeLimits {
        values_per_item: NonZeroUsize::new(values).unwrap(),
        destinations_per_value: NonZeroUsize::new(destinations).unwrap(),
    }
}
fn at(entity: usize, index: usize) -> Option<ListPosition> {
    Some(ListPosition { entity, index })
}
type World = (Arc<ListProblem<i32>>, ListAssignment<i32>);
fn world() -> World {
    let p = ListProblem::new(
        vec![1, 2, 3, 4],
        vec![
            ListEntity::new(vec![1, 2]).with_pinned_prefix(1),
            ListEntity::new(vec![3]).with_pinned(true),
            ListEntity::new(vec![]),
        ],
    )
    .unwrap()
    .allowing_unassigned();
    let a = p.initial_assignment();
    (Arc::new(p), a)
}
struct BorrowingFactory {
    rows: Vec<Vec<Option<ListPosition>>>,
    opened: Vec<i32>,
    indices: Vec<usize>,
    counts: Counts,
}
impl ListDestinationFactory<World, i32> for BorrowingFactory {
    fn event(&mut self, e: SelectionLifecycleEvent, _: &World) {
        self.counts.events.borrow_mut().push(e);
    }
    fn open<'a>(
        &'a mut self,
        _: &'a World,
        value: &'a i32,
        index: &'a ListDestinationIndex<'_, i32>,
    ) -> Box<dyn Iterator<Item = Option<ListPosition>> + 'a> {
        self.opened.push(*value);
        self.indices.push(std::ptr::from_ref(index) as usize);
        self.counts.opens.set(self.counts.opens.get() + 1);
        assert!(!self.counts.fail_open.get(), "factory");
        let row = &self.rows[*value as usize - 1];
        let counts = &self.counts;
        let mut cursor = row.iter();
        Box::new(std::iter::from_fn(move || {
            counts.reads.set(counts.reads.get() + 1);
            assert!(!counts.fail_read.get(), "destination");
            cursor.next().copied()
        }))
    }
}
fn factory(rows: Vec<Vec<Option<ListPosition>>>) -> BorrowingFactory {
    BorrowingFactory {
        rows,
        opened: vec![],
        indices: vec![],
        counts: Counts::default(),
    }
}
#[test]
fn borrowed_factories_are_lazy_pin_filtered_correlated_and_drop_unused_destinations() {
    let c = world();
    let left = Script::new(vec![1, 2, 3, 4, 2], false);
    let reads = left.counts.clone();
    let f = factory(vec![
        vec![],
        vec![at(1, 0), at(0, 0), at(2, 0), None],
        vec![],
        vec![None, at(2, 0)],
    ]);
    let counts = f.counts.clone();
    let mut source = CorrelatedListValueChangeSource::new(left, f, limits(4, 4), false);
    source.event(SelectionLifecycleEvent::SolveStarted, &c);
    source.event(SelectionLifecycleEvent::PhaseStarted, &c);
    assert_eq!(*reads.events.borrow(), *counts.events.borrow());
    assert_eq!(counts.events.borrow().len(), 2);
    assert_eq!(source.size(&c), None);
    assert!(!source.is_never_ending());
    drop(source.open(&c));
    assert_eq!((reads.opens.get(), counts.opens.get()), (0, 0));
    let selected = source.open(&c).collect::<Vec<_>>();
    assert_eq!(
        selected
            .iter()
            .map(|v| (*v.value(), v.destination()))
            .collect::<Vec<_>>(),
        [(2, at(2, 0)), (4, at(2, 0)), (2, at(2, 0))]
    );
    assert_eq!(source.factory().opened, [2, 4, 2]);
    assert_eq!((reads.reads.get(), counts.reads.get()), (6, 8));
    assert!(source.factory().indices.windows(2).all(|p| p[0] == p[1]));
    for candidate in selected {
        let mut a = c.1.clone();
        drop(c.0.apply(&mut a, candidate.movement()).unwrap());
        assert_eq!(a, c.1);
    }
}
#[test]
fn failed_branches_advance_values_and_raw_work_limits_fuse_repeating_sources() {
    let c = world();
    for (values, row, expected) in [
        (vec![1], vec![at(2, 0)], (3, 0, 0)),
        (vec![2], vec![], (3, 3, 3)),
        (vec![2], vec![at(0, 2), at(1, 0), at(0, 2)], (3, 3, 6)),
        (vec![], vec![], (1, 0, 0)),
    ] {
        let left = Script::new(values, true);
        let counts = left.counts.clone();
        let f = factory(vec![vec![], row, vec![], vec![]]);
        let fc = f.counts.clone();
        let mut source = CorrelatedListValueChangeSource::new(left, f, limits(3, 2), false);
        assert!(source.is_never_ending());
        let mut cursor = source.open(&c);
        assert!(cursor.next().is_none());
        assert!(cursor.next().is_none());
        assert_eq!(
            (counts.reads.get(), fc.opens.get(), fc.reads.get()),
            expected
        );
    }
    let mut source = CorrelatedListValueChangeSource::new(
        Script::new(vec![2, 4], false),
        factory(vec![vec![], vec![at(0, 2)], vec![], vec![at(2, 0)]]),
        limits(2, 1),
        false,
    );
    assert_eq!(*source.open(&c).next().unwrap().value(), 4);
}
#[test]
fn range_index_factories_match_independent_public_enumeration_and_apply_undo() {
    let models = [
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
        let values = p
            .values()
            .iter()
            .chain(p.values().iter().rev())
            .copied()
            .collect::<Vec<_>>();
        for noops in [false, true] {
            let index = ListDestinationIndex::new(&p, &a, p.allows_unassigned()).unwrap();
            let mut expected = vec![];
            for value in &values {
                let set = index.for_value(value).unwrap();
                let destinations = (0..set.len())
                    .map(|i| set.get(i).unwrap())
                    .collect::<Vec<_>>();
                if let Some(first) = list_value_changes(
                    &p,
                    &a,
                    &[*value],
                    &destinations,
                    ListChangePairing::Cartesian,
                    noops,
                )
                .unwrap()
                .next()
                {
                    expected.push(first);
                }
            }
            let f = |_: &ListSelectionContext<'_, i32>,
                     value: &i32,
                     index: &ListDestinationIndex<'_, i32>| {
                let set = index.for_value(value).unwrap();
                (0..set.len()).map(move |i| set.get(i).unwrap())
            };
            let mut source = CorrelatedListValueChangeSource::new(
                Script::new(values.clone(), false),
                f,
                limits(32, 32),
                noops,
            );
            let actual = source.open(&c).collect::<Vec<_>>();
            assert_eq!(actual, expected);
            for candidate in actual {
                let mut a = a.clone();
                drop(p.apply(&mut a, candidate.movement()).unwrap());
                assert_eq!(a, p.initial_assignment());
            }
        }
    }
}
#[test]
fn unknown_values_bad_destinations_and_callback_panics_fuse_then_reopen() {
    let c = world();
    for failure in 0..5 {
        let left = Script::new(vec![if failure == 0 { 99 } else { 2 }], true);
        let f = factory(vec![
            vec![],
            vec![if failure == 1 { at(99, 0) } else { at(2, 0) }],
            vec![],
            vec![],
        ]);
        let counts = f.counts.clone();
        counts.fail_open.set(failure == 2);
        counts.fail_read.set(failure == 3);
        let lc = left.counts.clone();
        lc.fail_read.set(failure == 4);
        let mut source = CorrelatedListValueChangeSource::new(left, f, limits(2, 2), false);
        let mut cursor = source.open(&c);
        assert!(catch_unwind(AssertUnwindSafe(|| cursor.next())).is_err());
        let before = (lc.reads.get(), counts.reads.get(), counts.opens.get());
        assert!(cursor.next().is_none());
        assert_eq!(
            (lc.reads.get(), counts.reads.get(), counts.opens.get()),
            before
        );
        drop(cursor);
        source.values_mut().items = vec![2];
        lc.fail_read.set(false);
        source.factory_mut().rows[1] = vec![at(2, 0)];
        counts.fail_open.set(false);
        counts.fail_read.set(false);
        assert_eq!(*source.open(&c).next().unwrap().value(), 2);
        assert_eq!(c.1, c.0.initial_assignment());
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
fn configured_provider_rebuilds_correlations_for_current_assignment_and_bounds_draws() {
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
    let lc = left.counts.clone();
    let opened = Rc::new(RefCell::new(vec![]));
    let recorded_opens = opened.clone();
    let factory = move |c: &World, value: &i32, index: &ListDestinationIndex<'_, i32>| {
        recorded_opens
            .borrow_mut()
            .push((*value, c.1.lists()[0].clone()));
        let slots = index.for_value(value).unwrap();
        let target = if *value == 1 {
            at(1, 0)
        } else {
            at(1, c.1.lists()[1].len())
        };
        assert!((0..slots.len()).any(|i| slots.get(i) == Some(target)));
        std::iter::once(target)
    };
    let source = CorrelatedListValueChangeSource::new(left, factory, limits(2, 1), false);
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
    assert_eq!(best.assignment.lists(), &[vec![], vec![1, 3, 2]]);
    assert_eq!(captures.get(), 2);
    assert_eq!(lc.opens.get(), 2);
    assert_eq!(
        *metadata.borrow(),
        [(1, at(0, 0)), (2, at(0, 1)), (1, at(0, 0)), (1, at(0, 0))]
    );
    assert_eq!(lc.reads.get(), 5);
    assert_eq!(
        *opened.borrow(),
        [
            (1, vec![1, 2]),
            (2, vec![1, 2]),
            (1, vec![1]),
            (2, vec![1]),
            (1, vec![1])
        ]
    );
    assert!(
        lc.events
            .borrow()
            .contains(&SelectionLifecycleEvent::SolveEnded)
    );
}
#[derive(Clone, Debug)]
struct Tagged {
    id: i32,
    tag: &'static str,
}
impl PartialEq for Tagged {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for Tagged {}
struct DropCursor {
    fail: Rc<Cell<bool>>,
    drops: Rc<Cell<usize>>,
    next: Option<Option<ListPosition>>,
}
impl Iterator for DropCursor {
    type Item = Option<ListPosition>;
    fn next(&mut self) -> Option<Self::Item> {
        self.next.take()
    }
}
impl Drop for DropCursor {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
        assert!(!self.fail.get(), "cursor disposal");
    }
}
#[test]
fn canonical_value_identity_and_panicking_cursor_disposal_preserve_publication_boundary() {
    let p = ListProblem::new(
        vec![Tagged {
            id: 1,
            tag: "registered",
        }],
        vec![
            ListEntity::new(vec![Tagged {
                id: 1,
                tag: "assigned",
            }]),
            ListEntity::new(vec![]),
        ],
    )
    .unwrap();
    let a = p.initial_assignment();
    let c = ListSelectionContext::new(&p, &a).unwrap();
    let values = Script::new(
        vec![Tagged {
            id: 1,
            tag: "streamed",
        }],
        true,
    );
    let reads = values.counts.clone();
    let fail = Rc::new(Cell::new(true));
    let drops = Rc::new(Cell::new(0));
    let f = fail.clone();
    let d = drops.clone();
    let factory = move |_: &ListSelectionContext<'_, Tagged>,
                        value: &Tagged,
                        index: &ListDestinationIndex<'_, Tagged>| {
        assert_eq!(value.tag, "registered");
        assert_eq!(index.for_value(value).unwrap().len(), 3);
        DropCursor {
            fail: f.clone(),
            drops: d.clone(),
            next: Some(at(1, 0)),
        }
    };
    let mut source = CorrelatedListValueChangeSource::new(values, factory, limits(1, 1), false);
    let mut cursor = source.open(&c);
    assert!(catch_unwind(AssertUnwindSafe(|| cursor.next())).is_err());
    fail.set(false);
    assert!(cursor.next().is_none());
    assert_eq!((reads.reads.get(), drops.get()), (1, 1));
    drop(cursor);
    let candidate = source.open(&c).next().unwrap();
    assert_eq!(candidate.value().tag, "registered");
    assert_eq!((reads.reads.get(), drops.get()), (2, 2));
    assert_eq!(a, p.initial_assignment());
}
