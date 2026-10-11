use std::{
    cell::{Cell, RefCell},
    num::NonZeroUsize,
    rc::Rc,
};
use timetuck::*;

#[derive(Clone, Default)]
struct Stats {
    reads: Rc<Cell<usize>>,
    opens: Rc<Cell<usize>>,
    events: Rc<RefCell<Vec<SelectionLifecycleEvent>>>,
}
struct Owners {
    rows: Vec<usize>,
    cycle: bool,
    stats: Stats,
}
impl Owners {
    fn new(rows: Vec<usize>, cycle: bool) -> Self {
        Self {
            rows,
            cycle,
            stats: Stats::default(),
        }
    }
}
impl<C> SelectionSource<C, usize> for Owners {
    fn is_never_ending(&self) -> bool {
        self.cycle
    }
    fn size(&mut self, _: &C) -> Option<usize> {
        Some(self.rows.len())
    }
    fn event(&mut self, e: SelectionLifecycleEvent, _: &C) {
        self.stats.events.borrow_mut().push(e)
    }
    fn open<'a>(&'a mut self, _: &'a C) -> Box<dyn Iterator<Item = usize> + 'a>
    where
        usize: 'a,
    {
        self.stats.opens.set(self.stats.opens.get() + 1);
        let mut offset = 0;
        Box::new(std::iter::from_fn(move || {
            self.stats.reads.set(self.stats.reads.get() + 1);
            if self.rows.is_empty() {
                return None;
            }
            let result = self
                .rows
                .get(if self.cycle {
                    offset % self.rows.len()
                } else {
                    offset
                })
                .copied();
            offset += 1;
            result
        }))
    }
}
fn budget(n: usize) -> NonZeroUsize {
    NonZeroUsize::new(n).unwrap()
}
fn bounds() -> SubListSize {
    SubListSize::new(1, 3).unwrap()
}
fn model() -> ListProblem<usize> {
    ListProblem::new(
        vec![0, 1, 2],
        vec![
            ListEntity::new(vec![0, 1]),
            ListEntity::new(vec![]),
            ListEntity::new(vec![2]).with_pinned(true),
        ],
    )
    .unwrap()
}
fn repeats<C, T>(s: &impl SelectionSource<C, T>, _: &C) -> bool {
    s.is_never_ending()
}

#[test]
fn repeating_budget_resets_after_success_and_total_budget_stays_total() {
    let p = model();
    let a = p.initial_assignment();
    let c = ListSelectionContext::new(&p, &a).unwrap();
    let owners = Owners::new(vec![0], true);
    let stats = owners.stats.clone();
    let mut source = EntitySubListSource::repeating(owners, bounds(), budget(3), 17);
    assert!(repeats(&source, &c));
    assert_eq!(source.size(&c), None);
    assert_eq!(source.span_count(&c), 3);
    assert_eq!(source.open(&c).take(10_000).count(), 10_000);
    assert_eq!(stats.reads.get(), 10_000);
    let owners = Owners::new(vec![0], true);
    let stats = owners.stats.clone();
    let mut total = EntitySubListSource::new(owners, bounds(), 3, 17);
    assert!(!repeats(&total, &c));
    assert_eq!(total.open(&c).count(), 3);
    assert_eq!(stats.reads.get(), 3);
    assert_eq!(total.open(&c).count(), 3);
    assert_eq!(stats.reads.get(), 6);
}

#[test]
fn rejection_runs_fuse_at_the_limit_and_reopen_with_a_fresh_budget() {
    let p = model();
    let a = p.initial_assignment();
    let c = ListSelectionContext::new(&p, &a).unwrap();
    let owners = Owners::new(vec![9, 1, 2, 0], true);
    let stats = owners.stats.clone();
    let draws = Cell::new(0);
    let mut source = EntitySubListSource::repeating_with_draw(owners, bounds(), budget(3), |_| {
        draws.set(draws.get() + 1);
        0
    });
    assert!(repeats(&source, &c));
    {
        let mut iter = source.open(&c);
        assert_eq!(iter.next(), None);
        assert_eq!(stats.reads.get(), 3);
        assert_eq!(iter.next(), None);
        assert_eq!(stats.reads.get(), 3);
    }
    assert_eq!(draws.get(), 0);
    assert_eq!(source.open(&c).next(), None);
    assert_eq!(stats.reads.get(), 6);
    assert_eq!(stats.opens.get(), 2);
    let owners = Owners::new(vec![9, 1, 2, 0], true);
    let stats = owners.stats.clone();
    let mut productive = EntitySubListSource::repeating(owners, bounds(), budget(4), 17);
    assert_eq!(productive.open(&c).take(100).count(), 100);
    assert_eq!(stats.reads.get(), 400);
}

#[test]
fn finite_child_exhaustion_fuses_and_zero_catalog_does_not_open_children() {
    let p = model();
    let a = p.initial_assignment();
    let c = ListSelectionContext::new(&p, &a).unwrap();
    let owners = Owners::new(vec![0, 0], false);
    let stats = owners.stats.clone();
    let mut source = EntitySubListSource::repeating(owners, bounds(), budget(10), 4);
    assert!(!repeats(&source, &c));
    let mut iter = source.open(&c);
    assert!(iter.next().is_some());
    assert!(iter.next().is_some());
    assert_eq!(iter.next(), None);
    assert_eq!(stats.reads.get(), 3);
    assert_eq!(iter.next(), None);
    assert_eq!(stats.reads.get(), 3);
    let owners = Owners::new(vec![0], true);
    let stats = owners.stats.clone();
    let mut empty =
        EntitySubListSource::repeating(owners, SubListSize::new(3, 3).unwrap(), budget(10), 4);
    assert_eq!(empty.span_count(&c), 0);
    assert_eq!(empty.size(&c), Some(0));
    assert!(repeats(&empty, &c));
    assert_eq!(empty.open(&c).next(), None);
    assert_eq!(stats.opens.get(), 0);
    assert_eq!(stats.reads.get(), 0);
}

#[test]
fn lifecycle_is_forwarded_once_and_random_state_continues_across_openings() {
    let p = model();
    let a = p.initial_assignment();
    let c = ListSelectionContext::new(&p, &a).unwrap();
    let owners = Owners::new(vec![0], true);
    let stats = owners.stats.clone();
    let mut source = EntitySubListSource::repeating(owners, bounds(), budget(1), 81);
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
    for event in events {
        source.event(event, &c)
    }
    assert_eq!(*stats.events.borrow(), events);
    let mut observed = source.open(&c).take(10).collect::<Vec<_>>();
    observed.extend(source.open(&c).take(10));
    let mut fresh =
        EntitySubListSource::repeating(Owners::new(vec![0], true), bounds(), budget(1), 81);
    assert_eq!(observed, fresh.open(&c).take(20).collect::<Vec<_>>());
}

#[test]
fn reopening_after_committed_moves_rebuilds_span_coordinates() {
    let p = ListProblem::new(
        vec![0, 1, 2],
        vec![ListEntity::new(vec![0, 1]), ListEntity::new(vec![2])],
    )
    .unwrap();
    let mut a = p.initial_assignment();
    let mut source = EntitySubListSource::repeating_with_draw(
        Owners::new(vec![0], true),
        bounds(),
        budget(1),
        |_| 0,
    );
    {
        let c = ListSelectionContext::new(&p, &a).unwrap();
        assert_eq!(source.span_count(&c), 4);
        assert_eq!(
            source.open(&c).next(),
            Some(ListRange {
                entity: 0,
                start: 0,
                end: 2
            })
        );
    }
    p.apply(
        &mut a,
        &ListMove::Change {
            source: ListPosition {
                entity: 0,
                index: 1,
            },
            destination: ListPosition {
                entity: 1,
                index: 1,
            },
        },
    )
    .unwrap()
    .commit();
    let c = ListSelectionContext::new(&p, &a).unwrap();
    assert_eq!(source.span_count(&c), 4);
    assert_eq!(
        source.open(&c).next(),
        Some(ListRange {
            entity: 0,
            start: 0,
            end: 1
        })
    );
}
