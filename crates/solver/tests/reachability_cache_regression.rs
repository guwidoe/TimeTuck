use SelectionLifecycleEvent::*;
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;
type Row = ReachabilityRange<usize, i32>;
#[derive(Default)]
struct Rows {
    rows: Vec<Row>,
    opens: usize,
    events: Vec<SelectionLifecycleEvent>,
    panic: bool,
    repeat: bool,
    polls: Rc<Cell<usize>>,
}
impl SelectionSource<(), Row> for Rows {
    fn event(&mut self, e: SelectionLifecycleEvent, _: &()) {
        self.events.push(e)
    }
    fn size(&mut self, _: &()) -> Option<usize> {
        None
    }
    fn open<'a>(&'a mut self, _: &'a ()) -> Box<dyn Iterator<Item = Row> + 'a>
    where
        Row: 'a,
    {
        self.opens += 1;
        assert!(!self.panic, "source build panic");
        let polls = self.polls.clone();
        if self.repeat {
            Box::new(
                std::iter::repeat(self.rows[0].clone())
                    .inspect(move |_| polls.set(polls.get() + 1)),
            )
        } else {
            Box::new(
                self.rows
                    .iter()
                    .cloned()
                    .inspect(move |_| polls.set(polls.get() + 1)),
            )
        }
    }
}
fn rows(values: Vec<i32>) -> Rows {
    Rows {
        rows: vec![(0, values, false)],
        ..Default::default()
    }
}
#[test]
fn each_scope_refreshes_only_at_its_boundary_and_snapshots_survive_release() {
    for (scope, builds) in [
        (SelectionCacheScope::Solve, 1),
        (SelectionCacheScope::Phase, 2),
        (SelectionCacheScope::Step, 4),
    ] {
        let mut c = ReachabilityCache::new(rows(vec![1]), scope, 5).unwrap();
        assert!(c.index().is_err());
        c.event(SolveStarted, &()).unwrap();
        let mut captured = vec![];
        let mut version = 1;
        for _ in 0..2 {
            c.event(PhaseStarted, &()).unwrap();
            for _ in 0..2 {
                c.event(StepStarted, &()).unwrap();
                captured.push(c.select(ReachabilityQuery::All).unwrap());
                version += 1;
                c.source_mut().rows = vec![(0, vec![version], false)];
                c.event(StepEnded, &()).unwrap();
            }
            c.event(PhaseEnded, &()).unwrap();
        }
        c.event(SolveEnded, &()).unwrap();
        assert!(c.index().is_err());
        assert_eq!(c.source().opens, builds);
        let expected = match scope {
            SelectionCacheScope::Solve => [1, 1, 1, 1],
            SelectionCacheScope::Phase => [1, 1, 3, 3],
            _ => [1, 2, 3, 4],
        };
        assert_eq!(
            captured.iter().map(|s| s.items()[0]).collect::<Vec<_>>(),
            expected
        );
        assert_eq!(c.source().events.len(), 14);
    }
}
#[test]
fn duplicate_failed_and_panicking_rebuilds_never_expose_old_indexes() {
    let mut c = ReachabilityCache::new(rows(vec![1, 2]), SelectionCacheScope::Step, 3).unwrap();
    c.event(SolveStarted, &()).unwrap();
    c.event(PhaseStarted, &()).unwrap();
    c.event(StepStarted, &()).unwrap();
    let held = c.index().unwrap();
    c.event(StepEnded, &()).unwrap();
    c.source_mut().rows = vec![(0, vec![3], false), (0, vec![4], false)];
    assert!(matches!(
        c.event(StepStarted, &()),
        Err(ReachabilityCacheError::Domain(
            ReachabilityError::DuplicateEntity(0)
        ))
    ));
    assert!(c.index().is_err());
    c.event(StepEnded, &()).unwrap();
    c.source_mut().rows = vec![(0, vec![5], true)];
    c.source_mut().panic = true;
    assert!(catch_unwind(AssertUnwindSafe(|| c.event(StepStarted, &()))).is_err());
    assert!(c.index().is_err());
    c.event(StepEnded, &()).unwrap();
    c.source_mut().panic = false;
    c.event(StepStarted, &()).unwrap();
    assert_eq!(c.select(ReachabilityQuery::All).unwrap().items(), [5]);
    assert_eq!(held.values().copied().collect::<Vec<_>>(), [1, 2]);
    let events = c.source().events.len();
    assert!(matches!(
        c.event(PhaseStarted, &()),
        Err(ReachabilityCacheError::Cache(
            SelectionCacheError::InvalidTransition { .. }
        ))
    ));
    assert_eq!(c.source().events.len(), events);
    assert_eq!(c.source_size().unwrap(), 1);
}
#[test]
fn collection_limits_bound_misbehaving_sources_and_reject_uncached_scope() {
    assert!(matches!(
        ReachabilityCache::new(rows(vec![]), SelectionCacheScope::Uncached, 1),
        Err(ReachabilityCacheError::Cache(
            SelectionCacheError::UncachedScope
        ))
    ));
    let mut source = rows(vec![1]);
    source.repeat = true;
    let polls = source.polls.clone();
    let mut c = ReachabilityCache::new(source, SelectionCacheScope::Solve, 2).unwrap();
    assert!(matches!(
        c.event(SolveStarted, &()),
        Err(ReachabilityCacheError::Cache(SelectionCacheError::Weights(
            WeightedSelectionError::CandidateLimit { maximum: 2 }
        )))
    ));
    assert_eq!(polls.get(), 3);
    assert!(c.index().is_err());
}
#[test]
fn deduplicated_views_keep_source_cardinality_optional_rules_and_independent_sampling() {
    let source = Rows {
        rows: vec![
            (0, vec![3, 1, 2, 2], true),
            (1, vec![2, 3, 1], false),
            (2, vec![3, 4], false),
        ],
        ..Default::default()
    };
    let mut c = ReachabilityCache::new(source, SelectionCacheScope::Solve, 3).unwrap();
    c.event(SolveStarted, &()).unwrap();
    assert_eq!(c.source_size().unwrap(), 4);
    assert_eq!(c.index().unwrap().entities(&2), [0, 1]);
    let selected = c.select(ReachabilityQuery::CoValues(&1)).unwrap();
    assert_eq!(selected.items(), [2, 3]);
    assert_eq!(
        c.index().unwrap().destinations(Some(&1)),
        [None, Some(&2), Some(&3)]
    );
    assert!(
        c.select(ReachabilityQuery::CoValues(&99))
            .unwrap()
            .is_empty()
    );
    assert!(c.select(ReachabilityQuery::Entity(&99)).unwrap().is_empty());
    let samples = selected.samples(7).take(20).copied().collect::<Vec<_>>();
    assert_eq!(
        samples,
        selected.samples(7).take(20).copied().collect::<Vec<_>>()
    );
    assert!(samples.contains(&2) && samples.contains(&3));
    c.event(SolveEnded, &()).unwrap();
    assert_eq!(selected.items(), [2, 3]);
}
#[test]
fn native_sources_capture_model_changes_and_unrestricted_list_owners() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![1, 2]).with_initial(1).with_pinned(true),
    ])
    .unwrap();
    let q = FiniteProblem::new(vec![Variable::new(vec![2, 3]).allowing_unassigned()]).unwrap();
    let mut c = ReachabilityCache::new(BasicRangeSource, SelectionCacheScope::Phase, 1).unwrap();
    c.event(SolveStarted, &p).unwrap();
    c.event(PhaseStarted, &p).unwrap();
    let old = c.index().unwrap();
    c.event(PhaseEnded, &p).unwrap();
    c.event(PhaseStarted, &q).unwrap();
    assert_eq!(c.index().unwrap().domain(&0).unwrap().as_ref(), [2, 3]);
    assert!(!old.entity_accepts(&0, None));
    assert!(c.index().unwrap().entity_accepts(&0, None));
    let lists = ListProblem::new(
        vec![1, 2],
        vec![
            ListEntity::new(vec![1]).with_value_range(vec![1]),
            ListEntity::new(vec![2]),
        ],
    )
    .unwrap()
    .allowing_unassigned();
    let mut c = ReachabilityCache::new(ListRangeSource, SelectionCacheScope::Solve, 2).unwrap();
    c.event(SolveStarted, &lists).unwrap();
    assert_eq!(c.index().unwrap().entities(&1), [0, 1]);
    assert_eq!(c.index().unwrap().entities(&2), [1]);
    assert!(c.index().unwrap().entity_accepts(&0, None));
}
#[test]
fn step_cached_related_values_supply_moves_in_both_backends() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2, 3]).with_initial(0)]).unwrap();
    let mut n = ConstraintNetwork::new(1);
    n.add_constraint("value", vec![0], |v| {
        SimpleScore::new(i64::from(*v[0].unwrap()))
    })
    .unwrap();
    for stream in [false, true] {
        let mut c = ReachabilityCache::new(BasicRangeSource, SelectionCacheScope::Step, 1).unwrap();
        c.event(SolveStarted, &p).unwrap();
        c.event(PhaseStarted, &p).unwrap();
        let mut provider = |p: &FiniteProblem<i32>, a: &Assignment<i32>, _: u64| {
            c.event(StepStarted, p).unwrap();
            let candidates = c
                .select(ReachabilityQuery::CoValues(a.values()[0].as_ref().unwrap()))
                .unwrap();
            c.event(StepEnded, p).unwrap();
            candidates
                .items()
                .iter()
                .map(|v| Move::Change {
                    variable: 0,
                    value: Some(*v),
                })
                .collect::<Vec<_>>()
        };
        let config = LocalSearchConfig {
            max_steps: 2,
            selection: MoveSelection::BestAccepted,
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
            local_search_from_with_provider(&p, &n, p.initial_assignment(), config, &mut provider)
                .unwrap()
        };
        assert_eq!(result.best.unwrap().score, SimpleScore::new(3));
        c.event(PhaseEnded, &p).unwrap();
        c.event(SolveEnded, &p).unwrap();
    }
}
