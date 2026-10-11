//! Original batch integration for owned list/sublist selection contexts.
use std::{
    cell::{Cell, RefCell},
    num::NonZeroUsize,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
    sync::Arc,
};
use timetuck::*;
type World = (Arc<ListProblem<i32>>, ListAssignment<i32>);
fn spans(random: bool) -> SubListSource {
    SubListSource::configured(
        SelectionSourceRequest {
            random,
            cache_scope: SelectionCacheScope::Uncached,
        },
        SubListSize::new(1, 1).unwrap(),
        17,
    )
}
fn network(values: usize) -> ListConstraintNetwork<'static, i32, SimpleScore> {
    let mut n = ListConstraintNetwork::new(2, values);
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
fn solve<T: Clone, S: SelectionSource<World, T>>(
    p: &ListProblem<i32>,
    source: S,
    draws: usize,
    mapper: impl FnMut(&ListProblem<i32>, &ListAssignment<i32>, u64, &T) -> Option<ListMove<i32>>,
) -> (ListAssignment<i32>, SimpleScore, Vec<ListAssignment<i32>>) {
    let shared = Arc::new(p.clone());
    let captures = Rc::new(RefCell::new(vec![]));
    let record = captures.clone();
    let selection = SelectionBuilder::new(SelectionConfig {
        order: Some(SelectionOrder::Original),
        ..Default::default()
    })
    .build(ConfiguredSelectionPolicy::default(), |_| source)
    .unwrap();
    let mut provider = ConfiguredMoveProvider::new(
        selection,
        ConfiguredProviderConfig {
            maximum_draws: draws,
            seed: 17,
        },
        move |_: &ListProblem<i32>, a: &ListAssignment<i32>, _: u64| {
            record.borrow_mut().push(a.clone());
            (shared.clone(), a.clone())
        },
        mapper,
    );
    let result = list_local_search_from_with_provider(
        p,
        &network(p.values().len()),
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
    let captures = captures.borrow().clone();
    (best.assignment, best.score, captures)
}
struct Destination;
impl SelectionSource<World, Option<ListPosition>> for Destination {
    fn is_never_ending(&self) -> bool {
        true
    }
    fn size(&mut self, _: &World) -> Option<usize> {
        Some(1)
    }
    fn open<'a>(&'a mut self, c: &'a World) -> Box<dyn Iterator<Item = Option<ListPosition>> + 'a>
    where
        Option<ListPosition>: 'a,
    {
        Box::new(std::iter::repeat(Some(ListPosition {
            entity: 1,
            index: c.1.lists()[1].len(),
        })))
    }
}
struct Entities {
    entity: usize,
    reads: Rc<Cell<usize>>,
    events: Rc<RefCell<Vec<SelectionLifecycleEvent>>>,
}
impl<C> SelectionSource<C, usize> for Entities {
    fn is_never_ending(&self) -> bool {
        true
    }
    fn size(&mut self, _: &C) -> Option<usize> {
        None
    }
    fn event(&mut self, e: SelectionLifecycleEvent, _: &C) {
        self.events.borrow_mut().push(e);
    }
    fn open<'a>(&'a mut self, _: &'a C) -> Box<dyn Iterator<Item = usize> + 'a>
    where
        usize: 'a,
    {
        Box::new(std::iter::from_fn(move || {
            self.reads.set(self.reads.get() + 1);
            Some(self.entity)
        }))
    }
}
fn entities(entity: usize) -> Entities {
    Entities {
        entity,
        reads: Default::default(),
        events: Default::default(),
    }
}
#[test]
fn unassigned_value_source_drives_current_assignment_provider_without_reassigning_committed_values()
{
    let p = ListProblem::new(
        vec![1, 2, 3],
        vec![
            ListEntity::new(vec![1]).with_pinned_prefix(1),
            ListEntity::new(vec![]),
        ],
    )
    .unwrap()
    .allowing_unassigned();
    let seen = Rc::new(RefCell::new(vec![]));
    let record = seen.clone();
    let (a, score, captures) = solve(
        &p,
        ListValueSource(ListValueSelection::Unassigned),
        3,
        move |_, a, step, value| {
            assert!(a.location(value).is_none());
            record.borrow_mut().push((step, *value));
            Some(ListMove::Assign {
                value: *value,
                destination: ListPosition {
                    entity: 1,
                    index: a.lists()[1].len(),
                },
            })
        },
    );
    assert_eq!(a.lists(), [vec![1], vec![3, 2]]);
    assert_eq!(score, SimpleScore::new(51));
    assert_eq!(captures.len(), 2);
    assert_eq!(*seen.borrow(), [(0, 2), (0, 3), (1, 2)]);
}
#[test]
fn catalog_sublist_change_composes_with_owned_context_and_refreshes_pre_removal_positions() {
    let p = ListProblem::new(
        vec![1, 2, 3],
        vec![ListEntity::new(vec![1, 2]), ListEntity::new(vec![3])],
    )
    .unwrap();
    let source = SubListChangeSource::new(spans(false), Destination, false, 0);
    let selected = Rc::new(RefCell::new(vec![]));
    let record = selected.clone();
    let (a, score, captures) = solve(&p, source, 2, move |p, a, _, proposal| {
        record.borrow_mut().push(*proposal);
        let m = proposal.into_move()?;
        assert!(p.move_checker(a).unwrap().is_doable(&m).is_ok());
        Some(m)
    });
    assert_eq!(a.lists(), [vec![], vec![3, 2, 1]]);
    assert_eq!(score, SimpleScore::new(60));
    assert_eq!(
        captures
            .iter()
            .map(|a| a.lists()[0].len())
            .collect::<Vec<_>>(),
        [2, 1]
    );
    let selected = selected.borrow();
    assert_eq!(selected.len(), 4);
    assert_eq!(
        selected
            .iter()
            .map(|p| (
                p.source.entity,
                p.source.start,
                p.source.end,
                p.destination.unwrap().index
            ))
            .collect::<Vec<_>>(),
        [(0, 0, 1, 1), (0, 1, 2, 1), (0, 0, 1, 2), (1, 0, 1, 2)]
    );
}
#[test]
fn entity_sublist_swap_composes_with_catalog_source_and_preserves_draw_state_and_cardinality() {
    let p = ListProblem::new(
        vec![1, 2, 3, 4],
        vec![ListEntity::new(vec![3, 4]), ListEntity::new(vec![1, 2])],
    )
    .unwrap();
    let right = entities(1);
    let reads = right.reads.clone();
    let events = right.events.clone();
    let draws = Rc::new(Cell::new(0));
    let d = draws.clone();
    let right = EntitySubListSource::repeating_with_draw(
        right,
        SubListSize::new(1, 1).unwrap(),
        NonZeroUsize::new(2).unwrap(),
        move |bound| {
            assert_eq!(bound, 2);
            let rank = d.get() % 2;
            d.set(d.get() + 1);
            rank
        },
    );
    let mut source = SubListSwapSource::new(spans(false), right, false, 0);
    let c = (Arc::new(p.clone()), p.initial_assignment());
    assert_eq!(source.proposal_count(&c), Some(16));
    assert_eq!(reads.get(), 0);
    assert_eq!(draws.get(), 0);
    let (a, score, captures) = solve(&p, source, 2, |_, _, _, proposal| {
        Some(proposal.into_move())
    });
    assert_eq!(a.lists(), [vec![1, 2], vec![3, 4]]);
    assert_eq!(score, SimpleScore::new(73));
    assert_eq!(captures.len(), 2);
    assert_ne!(captures[0], captures[1]);
    assert_eq!((reads.get(), draws.get()), (4, 4));
    assert!(
        events
            .borrow()
            .contains(&SelectionLifecycleEvent::SolveEnded)
    );
}
#[test]
fn owned_and_borrowed_span_sources_preserve_seeded_openings_and_total_budget_metadata() {
    let p = Arc::new(
        ListProblem::new(
            vec![1, 2, 3, 4],
            vec![
                ListEntity::new(vec![1, 2]).with_pinned_prefix(1),
                ListEntity::new(vec![3, 4]),
            ],
        )
        .unwrap(),
    );
    let mut owned = (p.clone(), p.initial_assignment());
    let mut left = spans(true);
    let mut right = spans(true);
    let mut entity_left =
        EntitySubListSource::new(entities(1), SubListSize::new(1, 2).unwrap(), 3, 29);
    let mut entity_right =
        EntitySubListSource::new(entities(1), SubListSize::new(1, 2).unwrap(), 3, 29);
    for opening in 0..3 {
        let borrowed = ListSelectionContext::new(&p, &owned.1).unwrap();
        assert_eq!(left.size(&owned), right.size(&borrowed));
        assert_eq!(left.span_count(&owned), right.span_count(&borrowed));
        assert_eq!(
            left.open(&owned).take(9).collect::<Vec<_>>(),
            right.open(&borrowed).take(9).collect::<Vec<_>>()
        );
        assert_eq!(
            entity_left.span_count(&owned),
            entity_right.span_count(&borrowed)
        );
        assert_eq!(entity_left.size(&owned), entity_right.size(&borrowed));
        let a = entity_left.open(&owned).collect::<Vec<_>>();
        let b = entity_right.open(&borrowed).collect::<Vec<_>>();
        assert_eq!(a, b);
        assert_eq!(a.len(), 3);
        if opening == 0 {
            p.apply(
                &mut owned.1,
                &ListMove::Change {
                    source: ListPosition {
                        entity: 1,
                        index: 0,
                    },
                    destination: ListPosition {
                        entity: 0,
                        index: 2,
                    },
                },
            )
            .unwrap()
            .commit();
        }
    }
}
#[test]
fn malformed_owned_snapshots_fail_before_value_iteration_or_entity_child_reads() {
    let p = Arc::new(
        ListProblem::new(
            vec![1, 2],
            vec![ListEntity::new(vec![1]), ListEntity::new(vec![2])],
        )
        .unwrap(),
    );
    let c = (p, ListAssignment::new(vec![vec![1, 1], vec![2]]));
    let mut values = ListValueSource(ListValueSelection::Movable);
    assert!(catch_unwind(AssertUnwindSafe(|| values.size(&c))).is_err());
    assert!(catch_unwind(AssertUnwindSafe(|| values.open(&c).next())).is_err());
    let mut spans = spans(false);
    assert!(catch_unwind(AssertUnwindSafe(|| spans.open(&c).next())).is_err());
    let child = entities(0);
    let reads = child.reads.clone();
    let mut entities = EntitySubListSource::new(child, SubListSize::new(1, 1).unwrap(), 2, 0);
    assert!(catch_unwind(AssertUnwindSafe(|| entities.open(&c).next())).is_err());
    assert_eq!(reads.get(), 0);
}
