use SelectionLifecycleEvent::*;
use std::{
    cell::{Cell, RefCell},
    collections::BTreeSet,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;
struct Source<T> {
    items: Vec<T>,
    events: Vec<SelectionLifecycleEvent>,
    opens: usize,
}
impl<T> Source<T> {
    fn new(items: Vec<T>) -> Self {
        Self {
            items,
            events: vec![],
            opens: 0,
        }
    }
}
impl<C, T: Clone> SelectionSource<C, T> for Source<T> {
    fn event(&mut self, e: SelectionLifecycleEvent, _: &C) {
        self.events.push(e);
    }
    fn size(&mut self, _: &C) -> Option<usize> {
        Some(self.items.len())
    }
    fn open<'a>(&'a mut self, _: &'a C) -> Box<dyn Iterator<Item = T> + 'a>
    where
        T: 'a,
    {
        self.opens += 1;
        Box::new(self.items.iter().cloned())
    }
}
#[test]
fn stable_sort_captures_context_at_each_scope_and_preserves_snapshots() {
    for scope in [
        SelectionCacheScope::Solve,
        SelectionCacheScope::Phase,
        SelectionCacheScope::Step,
    ] {
        let seen = Rc::new(RefCell::new(vec![]));
        let calls = seen.clone();
        let mut c = OrderedSelectionCache::sorted_by(
            Source::new(vec![(2, 0), (1, 1), (2, 2), (1, 3)]),
            scope,
            4,
            move |context: &bool, a, b| {
                calls.borrow_mut().push(*context);
                if *context {
                    b.0.cmp(&a.0)
                } else {
                    a.0.cmp(&b.0)
                }
            },
        )
        .unwrap();
        assert!(c.snapshot().is_err());
        c.event(SolveStarted, &false).unwrap();
        c.event(PhaseStarted, &true).unwrap();
        c.event(StepStarted, &false).unwrap();
        let first = c.snapshot().unwrap();
        let initial = if scope == SelectionCacheScope::Phase {
            vec![(2, 0), (2, 2), (1, 1), (1, 3)]
        } else {
            vec![(1, 1), (1, 3), (2, 0), (2, 2)]
        };
        assert_eq!(first.iter().copied().collect::<Vec<_>>(), initial);
        assert_eq!(
            first.iter().rev().copied().collect::<Vec<_>>(),
            initial.iter().rev().copied().collect::<Vec<_>>()
        );
        assert_eq!(first.get(4), None);
        let before = seen.borrow().len();
        c.event(StepEnded, &false).unwrap();
        c.event(StepStarted, &true).unwrap();
        assert_eq!(
            seen.borrow().len() > before,
            scope == SelectionCacheScope::Step
        );
        assert_eq!(
            c.source().opens,
            if scope == SelectionCacheScope::Step {
                2
            } else {
                1
            }
        );
        assert_eq!(first.iter().copied().collect::<Vec<_>>(), initial);
        c.event(StepEnded, &true).unwrap();
        c.event(PhaseEnded, &true).unwrap();
        c.event(SolveEnded, &true).unwrap();
        assert!(c.snapshot().is_err());
        assert_eq!(first.len(), 4);
    }
}
fn permutations(n: usize) -> Vec<Vec<usize>> {
    fn visit(bound: usize, prefix: &mut Vec<usize>, out: &mut Vec<Vec<usize>>) {
        if bound < 2 {
            out.push(prefix.clone());
            return;
        }
        for i in 0..bound {
            prefix.push(i);
            visit(bound - 1, prefix, out);
            prefix.pop();
        }
    }
    let mut out = vec![];
    visit(n, &mut vec![], &mut out);
    out
}
#[test]
fn every_draw_combination_yields_a_distinct_permutation_through_six_items() {
    for n in 0..=6 {
        let sequences = permutations(n);
        let expected = sequences.len();
        let mut results = BTreeSet::new();
        for sequence in sequences {
            let mut draws = sequence.into_iter();
            let bounds = Rc::new(RefCell::new(vec![]));
            let seen = bounds.clone();
            let mut c = OrderedSelectionCache::shuffled_with(
                Source::new((0..n).collect()),
                SelectionCacheScope::Step,
                n,
                move |bound| {
                    seen.borrow_mut().push(bound);
                    draws.next().unwrap()
                },
            )
            .unwrap();
            c.event(SolveStarted, &()).unwrap();
            c.event(PhaseStarted, &()).unwrap();
            c.event(StepStarted, &()).unwrap();
            let snapshot = c.snapshot().unwrap();
            let order = snapshot.iter().copied().collect::<Vec<_>>();
            assert_eq!(
                order.iter().copied().collect::<BTreeSet<_>>(),
                (0..n).collect()
            );
            assert_eq!(*bounds.borrow(), (2..=n).rev().collect::<Vec<_>>());
            assert_eq!(snapshot.iter().len(), n);
            assert_eq!(snapshot.iter().copied().collect::<Vec<_>>(), order);
            results.insert(order);
        }
        assert_eq!(results.len(), expected);
    }
}
#[test]
fn failed_ordering_is_unavailable_invalid_events_preserve_it_and_next_scope_recovers() {
    let bad = Rc::new(Cell::new(false));
    let flag = bad.clone();
    let mut c = OrderedSelectionCache::shuffled_with(
        Source::new(vec![1, 2, 3]),
        SelectionCacheScope::Step,
        3,
        move |n| if flag.get() { n } else { 0 },
    )
    .unwrap();
    c.event(SolveStarted, &()).unwrap();
    c.event(PhaseStarted, &()).unwrap();
    c.event(StepStarted, &()).unwrap();
    let held = c.snapshot().unwrap();
    let prior = held.iter().copied().collect::<Vec<_>>();
    let events = c.source().events.len();
    assert!(c.event(StepStarted, &()).is_err());
    assert_eq!(c.source().events.len(), events);
    assert_eq!(
        c.snapshot().unwrap().iter().copied().collect::<Vec<_>>(),
        prior
    );
    c.event(StepEnded, &()).unwrap();
    bad.set(true);
    assert_eq!(
        c.event(StepStarted, &()),
        Err(OrderedSelectionError::InvalidDraw { bound: 3, index: 3 })
    );
    assert!(c.snapshot().is_err());
    assert_eq!(c.state(), SelectionCacheState::Step);
    assert_eq!(held.iter().copied().collect::<Vec<_>>(), prior);
    c.event(StepEnded, &()).unwrap();
    bad.set(false);
    c.event(StepStarted, &()).unwrap();
    assert_eq!(c.size().unwrap(), 3);
    let panic_flag = Rc::new(Cell::new(false));
    let captured = panic_flag.clone();
    let mut sorted = OrderedSelectionCache::sorted_by(
        Source::new(vec![3, 1, 2]),
        SelectionCacheScope::Phase,
        3,
        move |_: &(), a, b| {
            assert!(!captured.get(), "comparator failed");
            a.cmp(b)
        },
    )
    .unwrap();
    sorted.event(SolveStarted, &()).unwrap();
    sorted.event(PhaseStarted, &()).unwrap();
    let old = sorted.snapshot().unwrap();
    sorted.event(PhaseEnded, &()).unwrap();
    panic_flag.set(true);
    assert!(catch_unwind(AssertUnwindSafe(|| sorted.event(PhaseStarted, &()))).is_err());
    assert!(sorted.snapshot().is_err());
    assert_eq!(old.iter().copied().collect::<Vec<_>>(), [1, 2, 3]);
    sorted.event(PhaseEnded, &()).unwrap();
    panic_flag.set(false);
    sorted.event(PhaseStarted, &()).unwrap();
    assert_eq!(sorted.size().unwrap(), 3);
}
struct Item {
    id: usize,
    clones: Rc<Cell<usize>>,
}
impl Clone for Item {
    fn clone(&self) -> Self {
        self.clones.set(self.clones.get() + 1);
        Self {
            id: self.id,
            clones: self.clones.clone(),
        }
    }
}
struct Once<T>(Option<Vec<T>>);
impl<C, T> SelectionSource<C, T> for Once<T> {
    fn size(&mut self, _: &C) -> Option<usize> {
        self.0.as_ref().map(Vec::len)
    }
    fn open<'a>(&'a mut self, _: &'a C) -> Box<dyn Iterator<Item = T> + 'a>
    where
        T: 'a,
    {
        Box::new(self.0.take().unwrap().into_iter())
    }
}
#[test]
fn nonclone_items_and_duplicate_occurrences_survive_release_and_owned_iteration_is_lazy() {
    struct NonClone(usize);
    let mut c = OrderedSelectionCache::sorted_by(
        Once(Some(vec![NonClone(2), NonClone(1)])),
        SelectionCacheScope::Solve,
        2,
        |_: &(), a, b| a.0.cmp(&b.0),
    )
    .unwrap();
    c.event(SolveStarted, &()).unwrap();
    let held = c.snapshot().unwrap();
    c.event(SolveEnded, &()).unwrap();
    assert_eq!(held.iter().map(|x| x.0).collect::<Vec<_>>(), [1, 2]);
    let clones = Rc::new(Cell::new(0));
    let items = (0..4)
        .map(|id| Item {
            id: id % 2,
            clones: clones.clone(),
        })
        .collect();
    let mut c = OrderedSelectionCache::sorted_by(
        Once(Some(items)),
        SelectionCacheScope::Solve,
        4,
        |_: &(), a, b| a.id.cmp(&b.id),
    )
    .unwrap();
    c.event(SolveStarted, &()).unwrap();
    let held = c.snapshot().unwrap();
    assert_eq!(held.iter().map(|x| x.id).collect::<Vec<_>>(), [0, 0, 1, 1]);
    assert_eq!(clones.get(), 0);
    let mut it = held.into_items();
    assert_eq!(it.len(), 4);
    assert_eq!(it.next().unwrap().id, 0);
    assert_eq!(it.next_back().unwrap().id, 1);
    assert_eq!(it.len(), 2);
    drop(it);
    assert_eq!(clones.get(), 2);
}
#[test]
fn seeded_shuffles_reproduce_across_steps_and_bounds_reject_before_opening() {
    fn run(seed: u64) -> Vec<Vec<usize>> {
        let mut c = OrderedSelectionCache::shuffled(
            Source::new((0..8).collect()),
            SelectionCacheScope::Solve,
            8,
            seed,
        )
        .unwrap();
        c.event(SolveStarted, &()).unwrap();
        c.event(PhaseStarted, &()).unwrap();
        let mut out = vec![];
        for _ in 0..5 {
            c.event(StepStarted, &()).unwrap();
            out.push(c.snapshot().unwrap().into_items().collect());
            c.event(StepEnded, &()).unwrap();
        }
        assert_eq!(c.source().opens, 1);
        out
    }
    assert_eq!(run(7), run(7));
    assert_ne!(run(7), run(8));
    let mut c =
        OrderedSelectionCache::shuffled(Source::new(vec![1, 2]), SelectionCacheScope::Solve, 1, 0)
            .unwrap();
    assert!(c.event(SolveStarted, &()).is_err());
    assert_eq!(c.source().opens, 0);
    assert!(c.snapshot().is_err());
}
struct Changes(FiniteProblem<i32>);
impl SelectionSource<Assignment<i32>, Move<i32>> for Changes {
    fn size(&mut self, _: &Assignment<i32>) -> Option<usize> {
        None
    }
    fn open<'a>(&'a mut self, a: &'a Assignment<i32>) -> Box<dyn Iterator<Item = Move<i32>> + 'a>
    where
        Move<i32>: 'a,
    {
        Box::new(change_moves(&self.0, a).unwrap())
    }
}
struct Provider(OrderedSelectionCache<Assignment<i32>, Move<i32>, Changes>);
struct Cursor<'a> {
    owner: &'a mut Provider,
    context: &'a Assignment<i32>,
    items: Box<dyn Iterator<Item = Move<i32>> + 'a>,
}
impl Iterator for Cursor<'_> {
    type Item = Move<i32>;
    fn next(&mut self) -> Option<Self::Item> {
        self.items.next()
    }
}
impl Drop for Cursor<'_> {
    fn drop(&mut self) {
        self.owner.0.event(StepEnded, self.context).unwrap();
    }
}
impl MoveProvider<FiniteProblem<i32>, Assignment<i32>, Move<i32>> for Provider {
    fn moves<'a>(
        &'a mut self,
        _: &'a FiniteProblem<i32>,
        a: &'a Assignment<i32>,
        _: u64,
    ) -> Box<dyn Iterator<Item = Move<i32>> + 'a>
    where
        Move<i32>: 'a,
    {
        if self.0.state() == SelectionCacheState::Idle {
            self.0.event(SolveStarted, a).unwrap();
            self.0.event(PhaseStarted, a).unwrap();
        }
        self.0.event(StepStarted, a).unwrap();
        let items = Box::new(self.0.snapshot().unwrap().into_items());
        Box::new(Cursor {
            owner: self,
            context: a,
            items,
        })
    }
}
#[test]
fn sorted_and_shuffled_move_providers_solve_in_both_basic_backends() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2, 3]).with_initial(0)]).unwrap();
    let mut n = ConstraintNetwork::new(1);
    n.add_constraint("value", vec![0], |v| {
        SimpleScore::new(i64::from(*v[0].unwrap()))
    })
    .unwrap();
    for shuffle in [false, true] {
        for stream in [false, true] {
            let cache = if shuffle {
                OrderedSelectionCache::shuffled(Changes(p.clone()), SelectionCacheScope::Step, 3, 9)
                    .unwrap()
            } else {
                OrderedSelectionCache::sorted_by(
                    Changes(p.clone()),
                    SelectionCacheScope::Step,
                    3,
                    |_, a, b| {
                        let value = |m: &Move<i32>| match m {
                            Move::Change { value, .. } => *value,
                            _ => None,
                        };
                        value(b).cmp(&value(a))
                    },
                )
                .unwrap()
            };
            let mut provider = Provider(cache);
            let config = LocalSearchConfig {
                max_steps: 3,
                max_move_evaluations: Some(if shuffle { 20 } else { 1 }),
                assert_scores: true,
                ..Default::default()
            };
            let result = if stream {
                let mut b = StreamGraphBuilder::new();
                let source = b.source::<usize, (usize, Option<i32>)>();
                b.score("value", source.port(), |r| {
                    SimpleScore::new(i64::from(r.1.unwrap()))
                })
                .unwrap();
                stream_local_search_from_with_provider(
                    &p,
                    b.build(),
                    source,
                    p.initial_assignment(),
                    config,
                    &mut provider,
                )
                .unwrap()
            } else {
                local_search_from_with_provider(
                    &p,
                    &n,
                    p.initial_assignment(),
                    config,
                    &mut provider,
                )
                .unwrap()
            };
            let best = result.best.unwrap();
            assert_eq!(best.score, SimpleScore::new(3));
            assert_eq!(provider.0.state(), SelectionCacheState::Phase);
            assert!(provider.0.snapshot().is_err());
            provider.0.event(PhaseEnded, &best.assignment).unwrap();
            provider.0.event(SolveEnded, &best.assignment).unwrap();
        }
    }
}
