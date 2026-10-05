use SelectionLifecycleEvent::*;
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
    sync::Arc,
};
use timetuck::*;
struct Source<T> {
    values: Vec<T>,
    opens: usize,
    polls: Rc<Cell<usize>>,
}
impl<T> Source<T> {
    fn new(values: Vec<T>) -> Self {
        Self {
            values,
            opens: 0,
            polls: Rc::new(Cell::new(0)),
        }
    }
}
impl<C, T: Clone> SelectionSource<C, T> for Source<T> {
    fn size(&mut self, _: &C) -> Option<usize> {
        Some(self.values.len())
    }
    fn open<'a>(&'a mut self, _: &'a C) -> Box<dyn Iterator<Item = T> + 'a>
    where
        T: 'a,
    {
        self.opens += 1;
        Box::new(
            self.values
                .iter()
                .cloned()
                .inspect(|_| self.polls.set(self.polls.get() + 1)),
        )
    }
}
#[test]
fn announcement_does_not_publish_and_repeated_probes_do_not_advance() {
    let mut owner = RecordingSelection::<(), _, _>::new(Source::new(vec![10, 20]));
    let replay = owner.replaying();
    assert_eq!(replay.size(), None);
    let polls = owner.source().polls.clone();
    let mut rec = owner.open(&());
    let mut reader = replay.cursor();
    assert!(!reader.has_next());
    assert_eq!(
        reader.next_replayed(),
        Err(SelectionReplayError::NotRecorded)
    );
    for _ in 0..5 {
        assert!(rec.has_next());
        assert!(reader.has_next());
    }
    assert_eq!(polls.get(), 1);
    assert_eq!(
        reader.next_replayed(),
        Err(SelectionReplayError::NotRecorded)
    );
    let first = rec.next().unwrap();
    assert_eq!(polls.get(), 1);
    let echoed = reader.next_replayed().unwrap().unwrap();
    assert!(Arc::ptr_eq(&first, &echoed));
    assert!(!reader.has_next());
    assert_eq!(reader.next_replayed().unwrap(), None);
    assert_eq!(*rec.next().unwrap(), 20);
    assert!(reader.has_next());
    assert_eq!(*reader.next_replayed().unwrap().unwrap(), 20);
    assert_eq!(rec.next(), None);
    assert_eq!(rec.next(), None);
    assert_eq!(polls.get(), 2);
}
#[test]
fn readers_are_independent_duplicates_are_occurrences_and_slow_readers_skip() {
    let mut owner = RecordingSelection::<(), _, _>::new(Source::new(vec![1, 1, 2]));
    let replay = owner.replaying();
    let mut rec = owner.open(&());
    let mut a = replay.cursor();
    let mut b = replay.clone().cursor();
    let first = rec.next().unwrap();
    assert!(Arc::ptr_eq(&a.next_replayed().unwrap().unwrap(), &first));
    assert_eq!(a.next_replayed().unwrap(), None);
    assert!(b.has_next());
    let second = rec.next().unwrap();
    assert!(!Arc::ptr_eq(&first, &second));
    assert!(Arc::ptr_eq(&a.next_replayed().unwrap().unwrap(), &second));
    assert!(Arc::ptr_eq(&b.next_replayed().unwrap().unwrap(), &second));
    let mut fresh = replay.cursor();
    assert!(Arc::ptr_eq(
        &fresh.next_replayed().unwrap().unwrap(),
        &second
    ));
    let third = rec.next().unwrap();
    assert_eq!(*third, 2);
    assert_eq!(*first, 1);
    assert_eq!(*second, 1);
    assert!(Arc::ptr_eq(&a.next_replayed().unwrap().unwrap(), &third));
}
struct Item(Rc<Cell<usize>>);
impl Drop for Item {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}
struct Once(Option<Item>);
impl SelectionSource<(), Item> for Once {
    fn size(&mut self, _: &()) -> Option<usize> {
        Some(usize::from(self.0.is_some()))
    }
    fn open<'a>(&'a mut self, _: &'a ()) -> Box<dyn Iterator<Item = Item> + 'a>
    where
        Item: 'a,
    {
        Box::new(self.0.take().into_iter())
    }
}
#[test]
fn nonclone_payloads_outlive_recorders_and_scope_changes_invalidate_old_cursors() {
    let drops = Rc::new(Cell::new(0));
    let mut owner = RecordingSelection::new(Once(Some(Item(drops.clone()))));
    let view = owner.replaying();
    let mut rec = owner.open(&());
    let mut reader = view.cursor();
    let held = rec.next().unwrap();
    drop(rec);
    let replayed = reader.next_replayed().unwrap().unwrap();
    assert!(Arc::ptr_eq(&held, &replayed));
    owner.event(StepEnded, &());
    assert!(!reader.has_next());
    assert!(matches!(
        reader.next_replayed(),
        Err(SelectionReplayError::StaleCursor)
    ));
    assert_eq!(view.size(), None);
    assert_eq!(drops.get(), 0);
    drop(owner);
    drop(view);
    drop(reader);
    drop(replayed);
    assert_eq!(drops.get(), 0);
    drop(held);
    assert_eq!(drops.get(), 1);
    let mut owner = RecordingSelection::<(), _, _>::new(Source::new(vec![3]));
    let view = owner.replaying();
    let mut rec = owner.open(&());
    let mut old = view.cursor();
    rec.next();
    drop(rec);
    let mut rec = owner.open(&());
    assert_eq!(old.next_replayed(), Err(SelectionReplayError::StaleCursor));
    let mut new = view.cursor();
    assert_eq!(*rec.next().unwrap(), 3);
    assert_eq!(*new.next_replayed().unwrap().unwrap(), 3);
}
#[test]
fn abandoning_unpublished_lookahead_clears_availability() {
    let mut owner = RecordingSelection::<(), _, _>::new(Source::new(vec![1, 2]));
    let view = owner.replaying();
    let mut rec = owner.open(&());
    let mut reader = view.cursor();
    rec.next();
    assert!(rec.has_next());
    assert!(reader.has_next());
    drop(rec);
    assert!(!reader.has_next());
    assert_eq!(reader.next_replayed().unwrap(), None);
}
struct Panics {
    bad: Rc<Cell<bool>>,
}
impl SelectionSource<(), usize> for Panics {
    fn size(&mut self, _: &()) -> Option<usize> {
        Some(10)
    }
    fn open<'a>(&'a mut self, _: &'a ()) -> Box<dyn Iterator<Item = usize> + 'a>
    where
        usize: 'a,
    {
        Box::new((0..10).inspect(|_| assert!(!self.bad.get(), "source failed")))
    }
}
#[test]
fn source_unwinding_cannot_leave_a_previous_choice_available() {
    let bad = Rc::new(Cell::new(false));
    let mut owner = RecordingSelection::new(Panics { bad: bad.clone() });
    let view = owner.replaying();
    let mut rec = owner.open(&());
    let mut reader = view.cursor();
    let held = rec.next().unwrap();
    bad.set(true);
    assert!(catch_unwind(AssertUnwindSafe(|| rec.has_next())).is_err());
    assert!(!reader.has_next());
    assert_eq!(
        reader.next_replayed(),
        Err(SelectionReplayError::NotRecorded)
    );
    assert_eq!(*held, 0);
    bad.set(false);
    let next = rec.next().unwrap();
    assert_eq!(*next, 2);
    assert_eq!(*reader.next_replayed().unwrap().unwrap(), 2);
}
#[test]
fn recorded_entities_coordinate_composite_moves_in_both_basic_backends() {
    let problem = FiniteProblem::new(vec![
        Variable::new(vec![0, 3]).with_initial(0),
        Variable::new(vec![0, 3]).with_initial(0),
    ])
    .unwrap();
    let mut network = ConstraintNetwork::new(2);
    network
        .add_constraint("sum", vec![0, 1], |v| {
            SimpleScore::new(i64::from(*v[0].unwrap() + *v[1].unwrap()))
        })
        .unwrap();
    for stream in [false, true] {
        let mut recorder =
            RecordingSelection::<Assignment<i32>, _, _>::new(Source::new(vec![0usize, 1]));
        let left = recorder.replaying();
        let right = recorder.replaying();
        let mut provider = move |_: &FiniteProblem<i32>, assignment: &Assignment<i32>, _: u64| {
            recorder.event(StepStarted, assignment);
            let mut result = vec![];
            {
                let mut recording = recorder.open(assignment);
                let mut a = left.cursor();
                let mut b = right.cursor();
                while recording.next().is_some() {
                    let variable = *a.next_replayed().unwrap().unwrap();
                    let companion = 1 - *b.next_replayed().unwrap().unwrap();
                    result.push(Move::Composite(vec![
                        Move::Change {
                            variable,
                            value: Some(3),
                        },
                        Move::Change {
                            variable: companion,
                            value: Some(3),
                        },
                    ]));
                }
            }
            recorder.event(StepEnded, assignment);
            result
        };
        let config = LocalSearchConfig {
            max_steps: 1,
            max_move_evaluations: Some(1),
            assert_scores: true,
            ..Default::default()
        };
        let result = if stream {
            let mut builder = StreamGraphBuilder::new();
            let source = builder.source::<usize, (usize, Option<i32>)>();
            builder
                .score("sum", source.port(), |row| {
                    SimpleScore::new(i64::from(row.1.unwrap()))
                })
                .unwrap();
            stream_local_search_from_with_provider(
                &problem,
                builder.build(),
                source,
                problem.initial_assignment(),
                config,
                &mut provider,
            )
            .unwrap()
        } else {
            local_search_from_with_provider(
                &problem,
                &network,
                problem.initial_assignment(),
                config,
                &mut provider,
            )
            .unwrap()
        };
        assert_eq!(result.best.unwrap().score, SimpleScore::new(6));
    }
}
