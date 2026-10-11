//! Original retained-partner cursor, bounded rejection and configured solve cases.
use std::{
    cell::{Cell, RefCell},
    num::NonZeroUsize,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
    sync::Arc,
};
use timetuck::*;
type World = (Arc<ListProblem<i32>>, ListAssignment<i32>);
struct Values {
    items: Vec<i32>,
    repeat: bool,
    reads: Rc<Cell<usize>>,
    events: Rc<RefCell<Vec<&'static str>>>,
}
impl Values {
    fn new(items: Vec<i32>, repeat: bool) -> Self {
        Self {
            items,
            repeat,
            reads: Rc::new(Cell::new(0)),
            events: Rc::new(RefCell::new(vec![])),
        }
    }
}
impl<C> SelectionSource<C, i32> for Values {
    fn event(&mut self, _: SelectionLifecycleEvent, _: &C) {
        self.events.borrow_mut().push("left");
    }
    fn size(&mut self, _: &C) -> Option<usize> {
        (!self.repeat || self.items.is_empty()).then_some(self.items.len())
    }
    fn is_never_ending(&self) -> bool {
        self.repeat
    }
    fn open<'a>(&'a mut self, _: &'a C) -> Box<dyn Iterator<Item = i32> + 'a>
    where
        i32: 'a,
    {
        let mut at = 0;
        Box::new(std::iter::from_fn(move || {
            self.reads.set(self.reads.get() + 1);
            if self.items.is_empty() {
                return None;
            }
            let i = if self.repeat {
                at % self.items.len()
            } else {
                at
            };
            at += 1;
            self.items.get(i).copied()
        }))
    }
}
fn limits(reads: usize, branches: usize) -> ListSwapSourceLimits {
    ListSwapSourceLimits {
        reads_per_side: NonZeroUsize::new(reads).unwrap(),
        pairs_per_item: NonZeroUsize::new(branches).unwrap(),
    }
}
fn world() -> World {
    let p = Arc::new(
        ListProblem::new(
            vec![0, 1, 2, 3, 4, 5],
            vec![
                ListEntity::new(vec![0, 1])
                    .with_pinned_prefix(1)
                    .with_value_range(vec![0, 1, 2, 4]),
                ListEntity::new(vec![2, 3]).with_value_range(vec![1, 2, 3, 4]),
            ],
        )
        .unwrap()
        .allowing_unassigned(),
    );
    let a = p.initial_assignment();
    (p, a)
}
fn partners(left: i32) -> Vec<i32> {
    match left {
        1 => vec![0, 2, 3, 4],
        2 => vec![1, 2],
        4 => vec![1, 5],
        _ => vec![],
    }
}
#[test]
fn every_finite_partner_is_retained_and_matches_ordered_reference() {
    let c = world();
    let opened = RefCell::new(vec![]);
    let left = Values::new(vec![0, 1, 2, 4, 4], false);
    let reads = left.reads.clone();
    let mut source = ListValueSwapSource::correlated(
        left,
        |_: &World, l: &i32, p: Option<ListPosition>| {
            opened.borrow_mut().push((*l, p));
            partners(*l).into_iter()
        },
        limits(8, 8),
        false,
    );
    let mut cursor = source.open(&c);
    assert_eq!(reads.get(), 0);
    assert!(opened.borrow().is_empty());
    let mut actual = vec![cursor.next().unwrap()];
    assert_eq!(reads.get(), 2);
    assert_eq!(opened.borrow().len(), 1);
    actual.push(cursor.next().unwrap());
    assert_eq!(reads.get(), 2);
    assert_eq!(opened.borrow().len(), 1);
    actual.extend(cursor);
    let mut expected = vec![];
    for l in [0, 1, 2, 4, 4] {
        expected.extend(
            list_value_swap_moves(
                &c.0,
                &c.1,
                &[l],
                &partners(l),
                ListSwapPairing::Cartesian,
                false,
            )
            .unwrap(),
        );
    }
    assert_eq!(actual, expected);
    assert_eq!(actual.len(), 5);
    assert_eq!(
        opened.borrow().iter().map(|p| p.0).collect::<Vec<_>>(),
        [1, 2, 4, 4]
    );
    assert_eq!(
        opened.borrow()[0].1,
        Some(ListPosition {
            entity: 0,
            index: 1
        })
    );
    assert_eq!(opened.borrow()[2].1, None);
}
#[test]
fn rejected_branches_advance_and_total_failed_work_is_bounded() {
    let c = world();
    let probes = Rc::new(Cell::new(0));
    let p = probes.clone();
    let mut source = ListValueSwapSource::correlated(
        Values::new(vec![1, 2], false),
        move |_: &World, l: &i32, _: Option<ListPosition>| {
            let v = if *l == 1 { 1 } else { 4 };
            let p = p.clone();
            std::iter::repeat_with(move || {
                p.set(p.get() + 1);
                v
            })
        },
        limits(3, 2),
        false,
    );
    let mut cursor = source.open(&c);
    assert!(cursor.next().is_some());
    assert_eq!(probes.get(), 4);
    assert!(cursor.next().is_some());
    assert_eq!(probes.get(), 5);
    drop(cursor);
    let probes = Rc::new(Cell::new(0));
    let p = probes.clone();
    let left = Values::new(vec![1, 2], true);
    let reads = left.reads.clone();
    let mut rejected = ListValueSwapSource::correlated(
        left,
        move |_: &World, l: &i32, _: Option<ListPosition>| {
            let l = *l;
            let p = p.clone();
            std::iter::repeat_with(move || {
                p.set(p.get() + 1);
                l
            })
        },
        limits(3, 2),
        false,
    );
    let mut cursor = rejected.open(&c);
    assert!(cursor.next().is_none());
    assert!(cursor.next().is_none());
    assert_eq!((reads.get(), probes.get()), (2, 6));
    let called = Cell::new(0);
    let mut pinned = ListValueSwapSource::correlated(
        Values::new(vec![0], true),
        |_: &World, _: &i32, _: Option<ListPosition>| {
            called.set(called.get() + 1);
            std::iter::empty()
        },
        limits(3, 2),
        false,
    );
    assert!(pinned.open(&c).next().is_none());
    assert_eq!(called.get(), 0);
}
struct Borrowed(Rc<RefCell<Vec<&'static str>>>);
impl ListSwapPartnerFactory<World, i32> for Borrowed {
    fn event(&mut self, _: SelectionLifecycleEvent, _: &World) {
        self.0.borrow_mut().push("factory");
    }
    fn open<'a>(
        &mut self,
        c: &'a World,
        left: &'a i32,
        _: Option<ListPosition>,
    ) -> Box<dyn Iterator<Item = i32> + 'a> {
        Box::new(c.0.values().iter().copied().filter(move |v| v != left))
    }
}
#[test]
fn partner_cursor_borrows_context_and_left_across_outputs() {
    let c = world();
    let trace = Rc::new(RefCell::new(vec![]));
    let mut left = Values::new(vec![1], false);
    left.events = trace.clone();
    let mut source =
        ListValueSwapSource::correlated(left, Borrowed(trace.clone()), limits(8, 2), false);
    source.event(SelectionLifecycleEvent::SolveStarted, &c);
    source.event(SelectionLifecycleEvent::PhaseStarted, &c);
    assert_eq!(*trace.borrow(), ["left", "factory", "left", "factory"]);
    let actual = source.open(&c).collect::<Vec<_>>();
    let expected = list_value_swap_moves(
        &c.0,
        &c.1,
        &[1],
        c.0.values(),
        ListSwapPairing::Cartesian,
        false,
    )
    .unwrap()
    .collect::<Vec<_>>();
    assert_eq!(actual, expected);
    assert_eq!(actual.len(), 2);
}
struct Bomb {
    at: usize,
    kind: usize,
    armed: Rc<Cell<bool>>,
}
impl Iterator for Bomb {
    type Item = i32;
    fn next(&mut self) -> Option<i32> {
        self.at += 1;
        if self.kind == 1 && self.at == 2 && self.armed.replace(false) {
            panic!("partner next")
        }
        if self.kind == 3 && self.at == 2 && self.armed.replace(false) {
            return Some(99);
        }
        match self.at {
            1 => Some(2),
            2 => Some(4),
            _ => None,
        }
    }
}
impl Drop for Bomb {
    fn drop(&mut self) {
        if self.kind == 2 && self.armed.replace(false) {
            panic!("partner drop")
        }
    }
}
#[test]
fn factory_next_unknown_value_and_exhaustion_drop_panics_fuse_then_reopen() {
    for kind in 0..4 {
        let c = world();
        let armed = Rc::new(Cell::new(true));
        let a = armed.clone();
        let opened = Cell::new(0);
        let mut source = ListValueSwapSource::correlated(
            Values::new(vec![1, 2], false),
            |_: &World, _: &i32, _: Option<ListPosition>| {
                opened.set(opened.get() + 1);
                if kind == 0 && a.replace(false) {
                    panic!("factory")
                };
                Bomb {
                    at: 0,
                    kind,
                    armed: a.clone(),
                }
            },
            limits(4, 2),
            false,
        );
        let mut cursor = source.open(&c);
        if kind != 0 {
            assert!(cursor.next().is_some());
        }
        if kind == 2 {
            assert!(cursor.next().is_some());
        }
        assert!(catch_unwind(AssertUnwindSafe(|| cursor.next())).is_err());
        let before = opened.get();
        assert!(cursor.next().is_none());
        assert_eq!(opened.get(), before);
        drop(cursor);
        armed.set(false);
        assert!(source.open(&c).next().is_some());
    }
}
#[test]
fn configured_provider_solves_with_distinct_partners_and_reopens_current_positions() {
    let p = ListProblem::new(
        vec![1, 2, 3, 4],
        vec![ListEntity::new(vec![3, 4]), ListEntity::new(vec![1, 2])],
    )
    .unwrap();
    let shared = Arc::new(p.clone());
    let seen = Rc::new(RefCell::new(vec![]));
    let observe = seen.clone();
    let source = ListValueSwapSource::correlated(
        Values::new(vec![3, 4], false),
        move |_: &World, l: &i32, pos: Option<ListPosition>| {
            observe.borrow_mut().push((*l, pos));
            if pos.is_some_and(|p| p.entity == 0) {
                if *l == 3 { vec![2, 1] } else { vec![1, 2] }
            } else {
                vec![]
            }
            .into_iter()
        },
        limits(4, 4),
        false,
    );
    let configured = SelectionBuilder::new(SelectionConfig {
        order: Some(SelectionOrder::Original),
        ..Default::default()
    })
    .build(ConfiguredSelectionPolicy::default(), |_| source)
    .unwrap();
    let mut provider = ConfiguredMoveProvider::new(
        configured,
        ConfiguredProviderConfig {
            maximum_draws: 16,
            seed: 17,
        },
        move |_: &ListProblem<i32>, a: &ListAssignment<i32>, _: u64| (shared.clone(), a.clone()),
        |_: &ListProblem<i32>, _: &ListAssignment<i32>, _: u64, m: &ListMove<i32>| Some(m.clone()),
    );
    let mut network = ListConstraintNetwork::new(2, 4);
    network
        .add_constraint(
            "weighted rows",
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
    let result = list_local_search_from_with_provider(
        &p,
        &network,
        p.initial_assignment(),
        LocalSearchConfig {
            max_steps: 4,
            selection: MoveSelection::BestAccepted,
            acceptance: Acceptance::HillClimbing,
            assert_scores: true,
            ..Default::default()
        },
        &mut provider,
    )
    .unwrap();
    let best = result.best.unwrap();
    assert_eq!(best.score, SimpleScore::new(73));
    assert_eq!(best.assignment.lists(), &[vec![2, 1], vec![4, 3]]);
    assert_eq!(seen.borrow().len(), 6);
    assert_eq!(
        seen.borrow()[1],
        (
            4,
            Some(ListPosition {
                entity: 0,
                index: 1
            })
        )
    );
    assert_eq!(
        seen.borrow()[3],
        (
            4,
            Some(ListPosition {
                entity: 1,
                index: 0
            })
        )
    );
}

#[derive(Clone)]
struct DroppingValue {
    id: i32,
    panic_on_drop: bool,
    armed: Rc<Cell<bool>>,
}
impl PartialEq for DroppingValue {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for DroppingValue {}
impl Drop for DroppingValue {
    fn drop(&mut self) {
        if self.panic_on_drop && self.armed.replace(false) {
            panic!("valid streamed partner drop")
        }
    }
}
type DropWorld = (
    Arc<ListProblem<DroppingValue>>,
    ListAssignment<DroppingValue>,
);
struct DropLeft;
impl SelectionSource<DropWorld, DroppingValue> for DropLeft {
    fn size(&mut self, _: &DropWorld) -> Option<usize> {
        Some(1)
    }
    fn open<'a>(&'a mut self, c: &'a DropWorld) -> Box<dyn Iterator<Item = DroppingValue> + 'a>
    where
        DroppingValue: 'a,
    {
        Box::new(std::iter::once(c.0.values()[0].clone()))
    }
}
#[test]
// Owned ListSelectionView uses Arc; the payload deliberately has local panic state.
#[allow(clippy::arc_with_non_send_sync)]
fn valid_partner_payload_drop_panics_before_publication_and_fuses() {
    let armed = Rc::new(Cell::new(true));
    let values = (1..=2)
        .map(|id| DroppingValue {
            id,
            panic_on_drop: false,
            armed: armed.clone(),
        })
        .collect::<Vec<_>>();
    let p = Arc::new(ListProblem::new(values.clone(), vec![ListEntity::new(values)]).unwrap());
    let c = (p.clone(), p.initial_assignment());
    let calls = Cell::new(0);
    let mut source = ListValueSwapSource::correlated(
        DropLeft,
        |_: &DropWorld, _: &DroppingValue, _: Option<ListPosition>| {
            calls.set(calls.get() + 1);
            std::iter::once(DroppingValue {
                id: 2,
                panic_on_drop: true,
                armed: armed.clone(),
            })
        },
        limits(2, 2),
        false,
    );
    let mut cursor = source.open(&c);
    assert!(catch_unwind(AssertUnwindSafe(|| cursor.next())).is_err());
    assert!(cursor.next().is_none());
    assert_eq!(calls.get(), 1);
    drop(cursor);
    assert!(source.open(&c).next().is_some());
    assert_eq!(calls.get(), 2);
}
