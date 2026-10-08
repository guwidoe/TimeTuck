use std::{cell::Cell, rc::Rc};
use timetuck::*;
struct Source<T> {
    values: Vec<T>,
    repeat: bool,
    known: bool,
    opens: usize,
    events: Vec<SelectionLifecycleEvent>,
}
impl<T: Clone> SelectionSource<(), T> for Source<T> {
    fn is_never_ending(&self) -> bool {
        self.repeat
    }
    fn size(&mut self, _: &()) -> Option<usize> {
        self.known.then_some(self.values.len())
    }
    fn event(&mut self, e: SelectionLifecycleEvent, _: &()) {
        self.events.push(e);
    }
    fn open<'a>(&'a mut self, _: &'a ()) -> Box<dyn Iterator<Item = T> + 'a>
    where
        T: 'a,
    {
        self.opens += 1;
        if self.repeat {
            Box::new(self.values.iter().cloned().cycle())
        } else {
            Box::new(self.values.iter().cloned())
        }
    }
}
fn source<T>(values: Vec<T>, repeat: bool) -> Source<T> {
    Source {
        values,
        repeat,
        known: true,
        opens: 0,
        events: vec![],
    }
}
#[test]
fn cardinalities_empty_sides_laziness_and_reopen() {
    for (l, r, expected) in [
        (false, false, Some(2)),
        (true, false, None),
        (false, true, None),
        (true, true, None),
    ] {
        let mut s =
            ZippedSelectionSource::new(source(vec![1, 2], l), source(vec!['a', 'b', 'c'], r));
        assert_eq!(s.size(&()), expected);
        assert_eq!(s.is_never_ending(), l && r);
        drop(s.open(&()));
        assert_eq!((s.left().opens, s.right().opens), (0, 0));
        assert_eq!(
            s.open(&()).take(2).collect::<Vec<_>>(),
            [(1, 'a'), (2, 'b')]
        );
        assert_eq!(s.open(&()).next(), Some((1, 'a')));
        s.left_mut().known = false;
        assert_eq!(s.size(&()), None);
        s.right_mut().values.clear();
        assert_eq!(s.size(&()), Some(0));
        let mut i = s.open(&());
        assert_eq!(i.next(), None);
        assert_eq!(i.next(), None);
    }
}
struct Payload(usize);
struct Owned(Rc<Cell<usize>>);
impl SelectionSource<(), Payload> for Owned {
    fn size(&mut self, _: &()) -> Option<usize> {
        None
    }
    fn open<'a>(&'a mut self, _: &'a ()) -> Box<dyn Iterator<Item = Payload> + 'a>
    where
        Payload: 'a,
    {
        Box::new(std::iter::from_fn(move || {
            let n = self.0.get();
            self.0.set(n + 1);
            Some(Payload(n))
        }))
    }
}
#[test]
fn nonclone_payloads_are_lazy_and_right_exhaustion_consumes_at_most_one_left() {
    let reads = Rc::new(Cell::new(0));
    let mut s = ZippedSelectionSource::new(Owned(reads.clone()), source(vec!["x"], false));
    let mut i = s.open(&());
    assert_eq!(reads.get(), 0);
    let p = i.next().unwrap();
    assert_eq!((p.0.0, p.1), (0, "x"));
    assert!(i.next().is_none());
    assert!(i.next().is_none());
    assert_eq!(reads.get(), 2);
    drop(i);
    assert_eq!(p.0.0, 0);
}
#[test]
fn lifecycle_and_independent_filters_precede_pairing() {
    let left =
        FilteredSelectionSource::new(source(vec![1, 2, 3, 4], false), |_: &(), v: &i32| *v != 1);
    let right =
        FilteredSelectionSource::new(source(vec!['a', 'b', 'c'], false), |_: &(), v: &char| {
            *v != 'b'
        });
    let mut s = ZippedSelectionSource::new(left, right);
    let events = [
        SelectionLifecycleEvent::SolveStarted,
        SelectionLifecycleEvent::PhaseStarted,
        SelectionLifecycleEvent::StepStarted,
        SelectionLifecycleEvent::StepEnded,
        SelectionLifecycleEvent::PhaseEnded,
        SelectionLifecycleEvent::SolveEnded,
    ];
    for e in events {
        s.event(e, &());
    }
    assert_eq!(s.open(&()).collect::<Vec<_>>(), [(2, 'a'), (3, 'c')]);
    assert_eq!(s.left().source().events, events);
    assert_eq!(s.right().source().events, events);
}
#[test]
fn bounded_repeating_pairs_drive_actual_list_search() {
    let p = ListProblem::new(
        vec![1, 2],
        vec![ListEntity::new(vec![1, 2]), ListEntity::new(vec![])],
    )
    .unwrap();
    let mut n = ListConstraintNetwork::new(2, 2);
    n.add_constraint(
        "balance",
        vec![ListScope::Entity(0), ListScope::Entity(1)],
        |rows| {
            let [ListFact::Entity(left), ListFact::Entity(right)] = rows else {
                panic!("entity scope")
            };
            SimpleScore::new(-(left.len() as i64 - right.len() as i64).abs())
        },
    )
    .unwrap();
    let zipped = ZippedSelectionSource::new(
        source(vec![1, 2], true),
        source(
            vec![Some(ListPosition {
                entity: 1,
                index: 0,
            })],
            true,
        ),
    );
    let mut provider = CachedMoveProvider::uniform(
        LimitedSelectionSource::new(zipped, 2),
        CachedProviderConfig::default(),
        |_: &ListProblem<i32>, _: &ListAssignment<i32>, _: u64| (),
        |p: &ListProblem<i32>,
         a: &ListAssignment<i32>,
         _: u64,
         pair: &(i32, Option<ListPosition>)| {
            list_value_changes(p, a, &[pair.0], &[pair.1], ListChangePairing::Paired, false)
                .unwrap()
                .next()
                .map(ListValueChange::into_move)
        },
    )
    .unwrap();
    let result = list_local_search_from_with_provider(
        &p,
        &n,
        p.initial_assignment(),
        LocalSearchConfig {
            max_steps: 1,
            assert_scores: true,
            ..Default::default()
        },
        &mut provider,
    )
    .unwrap();
    assert_eq!(result.best.unwrap().score, SimpleScore::new(0));
}
