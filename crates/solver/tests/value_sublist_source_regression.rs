//! Original checks for bounded selection, fresh ownership and real search.
use std::{
    cell::{Cell, RefCell},
    num::NonZeroUsize,
    rc::Rc,
    sync::Arc,
};
use timetuck::*;
struct Rows<T> {
    rows: Vec<T>,
    repeating: bool,
    reads: Rc<Cell<usize>>,
    opens: Rc<Cell<usize>>,
    events: Rc<RefCell<Vec<SelectionLifecycleEvent>>>,
}
impl<T> Rows<T> {
    fn new(rows: Vec<T>, repeating: bool) -> Self {
        Self {
            rows,
            repeating,
            reads: Default::default(),
            opens: Default::default(),
            events: Default::default(),
        }
    }
}
impl<C, T: Clone> SelectionSource<C, T> for Rows<T> {
    fn event(&mut self, event: SelectionLifecycleEvent, _: &C) {
        self.events.borrow_mut().push(event);
    }
    fn is_never_ending(&self) -> bool {
        self.repeating
    }
    fn size(&mut self, _: &C) -> Option<usize> {
        Some(self.rows.len())
    }
    fn open<'a>(&'a mut self, _: &'a C) -> Box<dyn Iterator<Item = T> + 'a>
    where
        T: 'a,
    {
        self.opens.set(self.opens.get() + 1);
        let mut offset = 0;
        Box::new(std::iter::from_fn(move || {
            self.reads.set(self.reads.get() + 1);
            if self.rows.is_empty() {
                return None;
            }
            let index = if self.repeating {
                offset % self.rows.len()
            } else {
                offset
            };
            offset += 1;
            self.rows.get(index).cloned()
        }))
    }
}
fn limits(entities: usize, seeds: usize) -> ValueSubListSourceLimits {
    ValueSubListSourceLimits {
        entity_reads: entities,
        seed_reads_per_item: NonZeroUsize::new(seeds).unwrap(),
    }
}
fn range(entity: usize, start: usize, end: usize) -> ListRange {
    ListRange { entity, start, end }
}
#[test]
fn catalog_restrictions_dedup_pins_and_every_rank_match_independent_spans() {
    let p = ListProblem::new(
        (0..8).collect(),
        vec![
            ListEntity::new(vec![0, 1, 2, 3]).with_pinned_prefix(1),
            ListEntity::new(vec![4, 5]).with_pinned(true),
            ListEntity::new(vec![6]),
        ],
    )
    .unwrap()
    .allowing_unassigned();
    let a = p.initial_assignment();
    let c = ListSelectionContext::new(&p, &a).unwrap();
    for min in 1..=4 {
        for max in min..=5 {
            for catalog in [vec![0, 0, 1, 99], vec![2], vec![], vec![2, 0]] {
                let mut expected = vec![];
                for (e, row) in a.lists().iter().enumerate() {
                    if !catalog.contains(&e) || p.entities()[e].is_pinned() {
                        continue;
                    }
                    for length in (min..=max).rev() {
                        for start in p.entities()[e].pinned_prefix()..row.len() {
                            if start + length <= row.len() {
                                expected.push(range(e, start, start + length));
                            }
                        }
                    }
                }
                let bounds = SubListSize::new(min, max).unwrap();
                let mut count_source = ValueSubListSource::new(
                    Rows::new(catalog.clone(), false),
                    Rows::<i32>::new(vec![], true),
                    bounds,
                    limits(100, 1),
                    0,
                );
                assert_eq!(count_source.span_count(&c), expected.len() as u128);
                for seed in 0..=8 {
                    let eligible: Vec<_> = a
                        .location(&seed)
                        .filter(|pos| pos.index >= p.entities()[pos.entity].pinned_prefix())
                        .map(|pos| {
                            expected
                                .iter()
                                .filter(|r| r.entity == pos.entity)
                                .copied()
                                .collect()
                        })
                        .unwrap_or_default();
                    if eligible.is_empty() {
                        let mut s = ValueSubListSource::with_draw(
                            Rows::new(catalog.clone(), false),
                            Rows::new(vec![seed], true),
                            bounds,
                            limits(100, 1),
                            |_| panic!("ineligible seed must not draw"),
                        );
                        assert_eq!(s.open(&c).next(), None);
                    } else {
                        for (rank, expected) in eligible.iter().enumerate() {
                            let n = eligible.len();
                            let mut s = ValueSubListSource::with_draw(
                                Rows::new(catalog.clone(), false),
                                Rows::new(vec![seed], true),
                                bounds,
                                limits(100, 1),
                                move |bound| {
                                    assert_eq!(bound, n as u128);
                                    rank as u128
                                },
                            );
                            assert_eq!(s.open(&c).next(), Some(*expected));
                        }
                    }
                }
            }
        }
    }
}
#[test]
fn rejection_limits_fuse_and_success_resets_budget_with_value_weighting() {
    let p = ListProblem::new(vec![1, 2, 3, 4], vec![ListEntity::new(vec![1, 2, 3])])
        .unwrap()
        .allowing_unassigned();
    let a = p.initial_assignment();
    let c = ListSelectionContext::new(&p, &a).unwrap();
    let entities = Rows::new(vec![0], true);
    let er = entities.reads.clone();
    let seeds = Rows::new(vec![4, 3, 3], true);
    let sr = seeds.reads.clone();
    let mut source = ValueSubListSource::with_draw(
        entities,
        seeds,
        SubListSize::new(1, 1).unwrap(),
        limits(2, 2),
        |_| 0,
    );
    assert_eq!(
        source.open(&c).take(100).collect::<Vec<_>>(),
        vec![range(0, 0, 1); 100]
    );
    assert_eq!(er.get(), 2);
    assert_eq!(sr.get(), 150);
    // A seed at the end may choose the beginning, preserving owner weighting.
    let seeds = Rows::new(vec![4], true);
    let sr = seeds.reads.clone();
    let mut source = ValueSubListSource::new(
        Rows::new(vec![0], false),
        seeds,
        SubListSize::new(1, 3).unwrap(),
        limits(2, 3),
        0,
    );
    let mut cursor = source.open(&c);
    assert_eq!(cursor.next(), None);
    assert_eq!(cursor.next(), None);
    assert_eq!(sr.get(), 3);
}
#[test]
fn empty_catalogs_avoid_child_reads_and_draw_panics_poison_only_the_opening() {
    let p = ListProblem::new(vec![1], vec![ListEntity::new(vec![1])]).unwrap();
    let a = p.initial_assignment();
    let c = ListSelectionContext::new(&p, &a).unwrap();
    for (bounds, catalog, entity_opens) in [
        (SubListSize::new(2, 2).unwrap(), vec![0], 0),
        (SubListSize::new(1, 1).unwrap(), vec![], 1),
    ] {
        let entities = Rows::new(catalog, false);
        let eo = entities.opens.clone();
        let seeds = Rows::new(vec![1], true);
        let so = seeds.opens.clone();
        let mut s = ValueSubListSource::with_draw(entities, seeds, bounds, limits(10, 10), |_| {
            panic!("empty draw")
        });
        assert_eq!(s.open(&c).next(), None);
        assert_eq!(eo.get(), entity_opens);
        assert_eq!(so.get(), 0);
    }
    let calls = Rc::new(Cell::new(0));
    let d = calls.clone();
    let mut s = ValueSubListSource::with_draw(
        Rows::new(vec![0], false),
        Rows::new(vec![1], true),
        SubListSize::new(1, 1).unwrap(),
        limits(10, 10),
        move |_| {
            d.set(d.get() + 1);
            if d.get() == 1 {
                panic!("draw failure");
            }
            0
        },
    );
    let mut cursor = s.open(&c);
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| cursor.next())).is_err());
    assert_eq!(cursor.next(), None);
    assert_eq!(calls.get(), 1);
    drop(cursor);
    assert_eq!(s.open(&c).next(), Some(range(0, 0, 1)));
    assert_eq!(calls.get(), 2);
}
#[test]
fn committed_edits_and_seed_state_are_visible_on_reopening() {
    let p = ListProblem::new(
        vec![1, 2, 3],
        vec![ListEntity::new(vec![1, 2]), ListEntity::new(vec![3])],
    )
    .unwrap();
    let mut a = p.initial_assignment();
    let mut source = ValueSubListSource::with_draw(
        Rows::new(vec![0, 1], false),
        Rows::new(vec![2], true),
        SubListSize::new(1, 3).unwrap(),
        limits(10, 1),
        |_| 0,
    );
    assert_eq!(
        source
            .open(&ListSelectionContext::new(&p, &a).unwrap())
            .next(),
        Some(range(0, 0, 2))
    );
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
    assert_eq!(source.open(&c).next(), Some(range(1, 0, 2)));
    let make = || {
        ValueSubListSource::new(
            Rows::new(vec![0, 1], false),
            Rows::new(vec![2, 3], true),
            SubListSize::new(1, 3).unwrap(),
            limits(10, 1),
            57,
        )
    };
    let mut s = make();
    let mut continuous = make();
    let mut actual = s.open(&c).take(50).collect::<Vec<_>>();
    actual.extend(s.open(&c).take(50));
    assert_eq!(actual, continuous.open(&c).take(100).collect::<Vec<_>>());
}
#[test]
fn owned_source_drives_actual_search_and_balanced_provider_events() {
    let p = Arc::new(
        ListProblem::new(
            vec![1, 2, 3],
            vec![ListEntity::new(vec![1, 2, 3]), ListEntity::new(vec![])],
        )
        .unwrap(),
    );
    let spans = ValueSubListSource::with_draw(
        Rows::new(vec![0, 1], false),
        Rows::new(vec![1, 2, 3], true),
        SubListSize::new(2, 3).unwrap(),
        limits(10, 3),
        |_| 0,
    );
    let entity_events = spans.entities().events.clone();
    let seed_events = spans.seeds().events.clone();
    let destinations = Rows::new(
        vec![Some(ListPosition {
            entity: 1,
            index: 0,
        })],
        true,
    );
    let destination_events = destinations.events.clone();
    let source = SubListChangeSource::new(spans, destinations, false, 0);
    let configured = SelectionBuilder::new(SelectionConfig {
        order: Some(SelectionOrder::Original),
        ..Default::default()
    })
    .build(ConfiguredSelectionPolicy::default(), |_| source)
    .unwrap();
    let shared = p.clone();
    let mut provider = ConfiguredMoveProvider::new(
        configured,
        ConfiguredProviderConfig {
            maximum_draws: 8,
            seed: 0,
        },
        move |_: &ListProblem<i32>, a: &ListAssignment<i32>, _: u64| (shared.clone(), a.clone()),
        |_: &ListProblem<i32>, _: &ListAssignment<i32>, _: u64, m: &SubListChangeProposal| {
            (*m).into_move()
        },
    );
    let mut n = ListConstraintNetwork::new(2, 3);
    n.add_constraint("target", vec![ListScope::Entity(1)], |facts| {
        let [ListFact::Entity(row)] = facts else {
            panic!()
        };
        SimpleScore::new(row.len() as i64)
    })
    .unwrap();
    let trace = Rc::new(RefCell::new(vec![]));
    let t = trace.clone();
    let mut events = move |e: SolverEvent<'_, ListAssignment<i32>, SimpleScore>| {
        if let SolverEvent::Best(u) = e {
            t.borrow_mut().push(u.score);
        }
    };
    let mut extensions = SolverExtensions {
        events: Some(&mut events),
        ..Default::default()
    };
    extensions.move_providers.bind(0, &mut provider);
    let result = solve_list_with_extensions(
        &p,
        &n,
        SolverSequence {
            phases: vec![SolverPhase::LocalSearch(LocalSearchConfig {
                max_steps: 3,
                assert_scores: true,
                ..Default::default()
            })],
            ..Default::default()
        },
        extensions,
    )
    .unwrap();
    assert_eq!(result.score, Some(SimpleScore::new(3)));
    assert_eq!(result.assignment.lists(), &[vec![], vec![1, 2, 3]]);
    assert_eq!(trace.borrow().last(), Some(&SimpleScore::new(3)));
    for events in [entity_events, seed_events, destination_events] {
        let events = events.borrow();
        use SelectionLifecycleEvent::*;
        assert_eq!(events.first(), Some(&SolveStarted));
        assert_eq!(events.last(), Some(&SolveEnded));
        for (start, end) in [
            (SolveStarted, SolveEnded),
            (PhaseStarted, PhaseEnded),
            (StepStarted, StepEnded),
        ] {
            let started = events.iter().filter(|e| **e == start).count();
            assert!(started > 0);
            assert_eq!(started, events.iter().filter(|e| **e == end).count());
        }
    }
}
