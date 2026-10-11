//! Original owned-context swap-provider and panic-boundary regressions.
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
type World = (Arc<ListProblem<i32>>, ListAssignment<i32>);
fn source(left: Script<i32>, right: Script<i32>) -> ListValueSwapSource<Script<i32>, Script<i32>> {
    ListValueSwapSource::new(
        left,
        right,
        ListSwapSourceLimits {
            reads_per_side: NonZeroUsize::new(4).unwrap(),
            pairs_per_item: NonZeroUsize::new(4).unwrap(),
        },
        false,
    )
}
fn network() -> ListConstraintNetwork<'static, i32, SimpleScore> {
    let mut n = ListConstraintNetwork::new(2, 5);
    n.add_constraint(
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
    n
}
#[test]
fn configured_provider_reopens_positions_for_swaps_and_optional_replacements() {
    for optional in [false, true] {
        let p = if optional {
            ListProblem::new(
                vec![1, 2, 3, 4, 5],
                vec![ListEntity::new(vec![1, 2]), ListEntity::new(vec![3])],
            )
            .unwrap()
            .allowing_unassigned()
        } else {
            ListProblem::new(
                vec![1, 2, 3, 4, 5],
                vec![ListEntity::new(vec![3, 4]), ListEntity::new(vec![1, 2, 5])],
            )
            .unwrap()
        };
        let shared = Arc::new(p.clone());
        let left_ids = if optional { vec![5, 4] } else { vec![3, 4] };
        let right_ids = vec![1, 2];
        let left = Script::new(left_ids.clone(), true);
        let right = Script::new(right_ids.clone(), true);
        let l = left.counts.clone();
        let r = right.counts.clone();
        let captures = Rc::new(RefCell::new(vec![]));
        let c = captures.clone();
        let observed = Rc::new(RefCell::new(vec![]));
        let o = observed.clone();
        let configured = SelectionBuilder::new(SelectionConfig {
            order: Some(SelectionOrder::Original),
            ..Default::default()
        })
        .build(ConfiguredSelectionPolicy::default(), |_| {
            source(left, right)
        })
        .unwrap();
        let mut provider = ConfiguredMoveProvider::new(
            configured,
            ConfiguredProviderConfig {
                maximum_draws: 2,
                seed: 17,
            },
            move |_: &ListProblem<i32>, a: &ListAssignment<i32>, step: u64| {
                c.borrow_mut().push((step, a.clone()));
                (shared.clone(), a.clone())
            },
            move |_: &ListProblem<i32>, a: &ListAssignment<i32>, step: u64, m: &ListMove<i32>| {
                o.borrow_mut().push((step, a.clone(), m.clone()));
                Some(m.clone())
            },
        );
        let n = network();
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
        assert_eq!(
            best.assignment.lists(),
            if optional {
                vec![vec![5, 4], vec![3]]
            } else {
                vec![vec![1, 2], vec![3, 4, 5]]
            }
        );
        assert_eq!(
            best.score,
            SimpleScore::new(if optional { 39 } else { 123 })
        );
        assert_eq!(
            (l.opens.get(), r.opens.get(), l.reads.get(), r.reads.get()),
            (2, 2, 4, 4)
        );
        let captures = captures.borrow();
        assert_eq!(captures.len(), 2);
        assert_ne!(captures[0].1, captures[1].1);
        let observed = observed.borrow();
        assert_eq!(observed.len(), 4);
        for (step, a) in captures.iter() {
            let expected =
                list_value_swap_moves(&p, a, &left_ids, &right_ids, ListSwapPairing::Paired, false)
                    .unwrap()
                    .collect::<Vec<_>>();
            let actual = observed
                .iter()
                .filter(|(s, _, _)| s == step)
                .map(|(_, snapshot, m)| {
                    assert_eq!(snapshot, a);
                    m.clone()
                })
                .collect::<Vec<_>>();
            assert_eq!(actual, expected);
            let mut director = ListScoreDirector::new(&p, &n, a.clone()).unwrap();
            let score = director.score();
            for m in actual {
                let trial = director.apply(&m).unwrap();
                drop(trial);
                assert_eq!(director.assignment(), a);
                assert_eq!(director.score(), score);
                director.assert_consistent().unwrap();
            }
        }
        assert_eq!(*l.events.borrow(), *r.events.borrow());
        assert!(
            l.events
                .borrow()
                .contains(&SelectionLifecycleEvent::SolveEnded)
        );
    }
}
#[test]
fn first_acceptance_stops_live_consumption_and_zero_draws_do_not_open_children() {
    for maximum_draws in [0, 1000] {
        let p = ListProblem::new(
            vec![1, 2, 3, 4, 5],
            vec![ListEntity::new(vec![3, 4]), ListEntity::new(vec![1, 2, 5])],
        )
        .unwrap();
        let shared = Arc::new(p.clone());
        let left = Script::new(vec![3, 4], true);
        let right = Script::new(vec![1, 2], true);
        let l = left.counts.clone();
        let r = right.counts.clone();
        let selection = SelectionBuilder::new(SelectionConfig {
            order: Some(SelectionOrder::Original),
            ..Default::default()
        })
        .build(ConfiguredSelectionPolicy::default(), |_| {
            source(left, right)
        })
        .unwrap();
        let mut provider = ConfiguredMoveProvider::new(
            selection,
            ConfiguredProviderConfig {
                maximum_draws,
                seed: 17,
            },
            move |_: &ListProblem<i32>, a: &ListAssignment<i32>, _: u64| {
                (shared.clone(), a.clone())
            },
            |_: &ListProblem<i32>, _: &ListAssignment<i32>, _: u64, m: &ListMove<i32>| {
                Some(m.clone())
            },
        );
        let result = list_local_search_from_with_provider(
            &p,
            &network(),
            p.initial_assignment(),
            LocalSearchConfig {
                max_steps: 1,
                selection: MoveSelection::FirstAccepted,
                assert_scores: true,
                ..Default::default()
            },
            &mut provider,
        )
        .unwrap();
        assert_eq!(
            result.best.unwrap().score,
            SimpleScore::new(if maximum_draws == 0 { 87 } else { 105 })
        );
        let expected = usize::from(maximum_draws != 0);
        assert_eq!(
            (l.opens.get(), r.opens.get(), l.reads.get(), r.reads.get()),
            (expected, expected, expected, expected)
        );
    }
}
#[test]
fn caught_child_panics_after_output_fuse_instead_of_resuming_half_consumed_pairs() {
    let p = Arc::new(
        ListProblem::new(
            vec![1, 2, 3],
            vec![ListEntity::new(vec![1, 2]), ListEntity::new(vec![3])],
        )
        .unwrap(),
    );
    let c: World = (p.clone(), p.initial_assignment());
    for right_failure in [false, true] {
        let left = Script::new(vec![1, 2], true);
        let right = Script::new(vec![3], true);
        let l = left.counts.clone();
        let r = right.counts.clone();
        let failure = if right_failure { r.clone() } else { l.clone() };
        let mut source = source(left, right);
        let mut cursor = source.open(&c);
        let expected = cursor.next().unwrap();
        failure.fail_read.set(true);
        assert!(catch_unwind(AssertUnwindSafe(|| cursor.next())).is_err());
        failure.fail_read.set(false);
        let before = (l.reads.get(), r.reads.get());
        assert!(cursor.next().is_none());
        assert_eq!((l.reads.get(), r.reads.get()), before);
        drop(cursor);
        assert_eq!(source.open(&c).next(), Some(expected));
        assert_eq!(c.1, p.initial_assignment());
    }
}
#[test]
fn owned_context_validates_before_opening_children_and_open_failures_fuse() {
    let p = Arc::new(
        ListProblem::new(
            vec![1, 2],
            vec![ListEntity::new(vec![1]), ListEntity::new(vec![2])],
        )
        .unwrap(),
    );
    for failure in 0..4 {
        let c: World = (
            p.clone(),
            if failure == 0 {
                ListAssignment::new(vec![vec![1, 1], vec![2]])
            } else {
                p.initial_assignment()
            },
        );
        let left = Script::new(vec![if failure == 3 { 99 } else { 1 }], true);
        let right = Script::new(vec![2], true);
        let l = left.counts.clone();
        let r = right.counts.clone();
        l.fail_open.set(failure == 1);
        r.fail_open.set(failure == 2);
        let mut source = source(left, right);
        let mut cursor = source.open(&c);
        assert!(catch_unwind(AssertUnwindSafe(|| cursor.next())).is_err());
        let before = (l.opens.get(), r.opens.get(), l.reads.get(), r.reads.get());
        assert!(cursor.next().is_none());
        assert_eq!(
            (l.opens.get(), r.opens.get(), l.reads.get(), r.reads.get()),
            before
        );
        if failure == 0 {
            assert_eq!(before, (0, 0, 0, 0));
        }
        drop(cursor);
        l.fail_open.set(false);
        r.fail_open.set(false);
        source.left_mut().items = vec![1];
        assert!(
            source
                .open(&(p.clone(), p.initial_assignment()))
                .next()
                .is_some()
        );
    }
}
