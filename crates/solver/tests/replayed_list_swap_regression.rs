//! Original recorded-origin sampling, failure budgets, refreshed snapshots and real search.
use std::{
    cell::Cell,
    num::{NonZeroU32, NonZeroUsize},
    rc::Rc,
    sync::Arc,
};
use timetuck::*;
struct Cycle<T> {
    values: Vec<T>,
    reads: Rc<Cell<usize>>,
}
impl<C, T: Clone> SelectionSource<C, T> for Cycle<T> {
    fn size(&mut self, _: &C) -> Option<usize> {
        Some(self.values.len())
    }
    fn is_never_ending(&self) -> bool {
        true
    }
    fn open<'a>(&'a mut self, _: &'a C) -> Box<dyn Iterator<Item = T> + 'a>
    where
        T: 'a,
    {
        let reads = self.reads.clone();
        Box::new(
            self.values
                .iter()
                .cloned()
                .cycle()
                .inspect(move |_| reads.set(reads.get() + 1)),
        )
    }
}
fn limits() -> ReplayedListSwapLimits {
    ReplayedListSwapLimits {
        left_reads: NonZeroUsize::new(5).unwrap(),
        partner_attempts: NonZeroUsize::new(8).unwrap(),
    }
}
fn source<T>(v: Vec<T>) -> ReplayedListValueSwapSource<Cycle<T>> {
    ReplayedListValueSwapSource::new(
        Cycle {
            values: v,
            reads: Rc::new(Cell::new(0)),
        },
        ListSwapPartnerOrder::Native { seed: 19 },
        limits(),
    )
}
#[test]
fn unreachable_origins_yield_fallback_without_spinning_or_consuming_new_origins() {
    let p = Arc::new(
        ListProblem::new(
            vec![1, 2],
            vec![
                ListEntity::new(vec![2]).with_value_range(vec![1, 2]),
                ListEntity::new(vec![1]).with_value_range(vec![1]),
            ],
        )
        .unwrap(),
    );
    let c = (p.clone(), p.initial_assignment());
    let mut s = source(vec![1, 2]);
    let reads = s.left().reads.clone();
    let moves = s.open(&c).take(100).collect::<Vec<_>>();
    assert_eq!(reads.get(), 100);
    assert_eq!(moves, vec![ListMove::Composite(vec![]); 100]);
}
#[test]
fn pinned_and_missing_origin_cycles_fuse_at_the_left_read_budget() {
    let p = Arc::new(
        ListProblem::new(vec![1, 2], vec![ListEntity::new(vec![1]).with_pinned(true)])
            .unwrap()
            .allowing_unassigned(),
    );
    let c = (p.clone(), p.initial_assignment());
    let mut s = source(vec![1, 2]);
    let reads = s.left().reads.clone();
    let mut it = s.open(&c);
    assert!(it.next().is_none());
    assert!(it.next().is_none());
    assert_eq!(reads.get(), 5);
}
#[test]
fn reopening_uses_current_positions_and_replays_seeded_proposals() {
    let p = Arc::new(
        ListProblem::new(
            vec![1, 2, 3],
            vec![ListEntity::new(vec![1, 2]), ListEntity::new(vec![3])],
        )
        .unwrap(),
    );
    let c = (p.clone(), p.initial_assignment());
    let mut s = source(vec![1, 2, 3]);
    let first = s.open(&c).take(40).collect::<Vec<_>>();
    assert_eq!(first, s.open(&c).take(40).collect::<Vec<_>>());
    for m in &first {
        assert!(p.move_checker(&c.1).unwrap().is_doable(m).unwrap());
    }
    let mut changed = c.1.clone();
    p.apply(
        &mut changed,
        &ListMove::Swap {
            left: ListPosition {
                entity: 0,
                index: 0,
            },
            right: ListPosition {
                entity: 1,
                index: 0,
            },
        },
    )
    .unwrap()
    .commit();
    let fresh = (p.clone(), changed);
    let movements = s.open(&fresh).take(40).collect::<Vec<_>>();
    assert_ne!(first, movements);
    for m in &movements {
        assert!(p.move_checker(&fresh.1).unwrap().is_doable(m).unwrap());
    }
}
#[test]
fn caught_unknown_origin_panics_fuse_the_opening() {
    let p = Arc::new(ListProblem::new(vec![1, 2], vec![ListEntity::new(vec![1, 2])]).unwrap());
    let c = (p.clone(), p.initial_assignment());
    let mut s = source(vec![99, 1]);
    let mut it = s.open(&c);
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| it.next())).is_err());
    assert!(it.next().is_none());
}
#[derive(Clone, Eq, PartialEq)]
struct EqOnly(i32);
#[test]
fn global_and_entity_ranges_accept_eq_only_values() {
    let p = Arc::new(
        ListProblem::new(
            vec![EqOnly(1), EqOnly(2)],
            vec![
                ListEntity::new(vec![EqOnly(1)]).with_value_range(vec![EqOnly(1), EqOnly(2)]),
                ListEntity::new(vec![EqOnly(2)]),
            ],
        )
        .unwrap(),
    );
    let c = (p.clone(), p.initial_assignment());
    let mut s = source(vec![EqOnly(1)]);
    assert!(matches!(
        s.open(&c).next(),
        Some(ListMove::Swap {
            left: ListPosition {
                entity: 0,
                index: 0
            },
            right: ListPosition {
                entity: 1,
                index: 0
            }
        })
    ));
}
#[test]
fn lcg48_matches_independent_jdk_bounded_draws_including_rejection() {
    let data = include_str!("fixtures/lcg48_reference.csv");
    let mut previous_seed = None;
    let mut r = Lcg48Random::new(0);
    for line in data.lines() {
        let row = line.split(',').collect::<Vec<_>>();
        let seed = row[0].parse::<i64>().unwrap();
        let bound = row[1].parse::<u32>().unwrap();
        let want = row[2].parse::<u32>().unwrap();
        if previous_seed != Some(seed) {
            r = Lcg48Random::new(seed as u64);
            previous_seed = Some(seed);
        }
        assert_eq!(r.below(NonZeroU32::new(bound).unwrap()).unwrap(), want);
    }
    let mut r = Lcg48Random::new(0);
    let mut unchanged = r.clone();
    assert_eq!(
        r.below(NonZeroU32::new(1 << 31).unwrap()),
        Err(InvalidLcg48Bound(1 << 31))
    );
    assert_eq!(
        r.below(NonZeroU32::new(100).unwrap()),
        unchanged.below(NonZeroU32::new(100).unwrap())
    );
}
fn provider(
    p: Arc<ListProblem<i32>>,
) -> impl MoveProvider<ListProblem<i32>, ListAssignment<i32>, ListMove<i32>> {
    let selected = SelectionBuilder::new(SelectionConfig {
        order: Some(SelectionOrder::Original),
        ..Default::default()
    })
    .build(ConfiguredSelectionPolicy::default(), |_| source(vec![1, 2]))
    .unwrap();
    ConfiguredMoveProvider::new(
        selected,
        ConfiguredProviderConfig {
            maximum_draws: 8,
            seed: 0,
        },
        move |_: &ListProblem<i32>, a: &ListAssignment<i32>, _: u64| (p.clone(), a.clone()),
        |_: &ListProblem<i32>, _: &ListAssignment<i32>, _: u64, m: &ListMove<i32>| Some(m.clone()),
    )
}
fn config() -> LocalSearchConfig<'static> {
    LocalSearchConfig {
        max_steps: 2,
        acceptance: Acceptance::HillClimbing,
        assert_scores: true,
        ..Default::default()
    }
}
#[test]
fn recorded_partner_source_drives_dense_and_stream_search_to_the_same_optimum() {
    let p = ListProblem::new(
        vec![1, 2],
        vec![ListEntity::new(vec![2]), ListEntity::new(vec![1])],
    )
    .unwrap();
    let mut n = ListConstraintNetwork::new(2, 2);
    n.add_constraint("reward", vec![ListScope::Entity(1)], |rows| {
        let ListFact::Entity(v) = rows[0] else {
            panic!()
        };
        SimpleScore::new(10 * i64::from(v[0]))
    })
    .unwrap();
    let mut dense_provider = provider(Arc::new(p.clone()));
    let dense = list_local_search_from_with_provider(
        &p,
        &n,
        p.initial_assignment(),
        config(),
        &mut dense_provider,
    )
    .unwrap()
    .best
    .unwrap();
    assert_eq!(dense.score, SimpleScore::new(20));
    assert_eq!(dense.assignment.lists(), &[vec![1], vec![2]]);
    let mut b = StreamGraphBuilder::new();
    let sources = ListStreamSources::new(&mut b);
    b.score("reward", sources.entities.port(), |(e, v)| {
        SimpleScore::new(if *e == 1 { 10 * i64::from(v[0]) } else { 0 })
    })
    .unwrap();
    let mut director =
        ListStreamScoreDirector::new(&p, b.build(), sources, p.initial_assignment()).unwrap();
    let streamed = director
        .run_local_search_with_provider(config(), &mut provider(Arc::new(p.clone())))
        .unwrap()
        .best
        .unwrap();
    assert_eq!(streamed.assignment, dense.assignment);
    assert_eq!(streamed.score, dense.score);
    director.assert_consistent().unwrap();
}
