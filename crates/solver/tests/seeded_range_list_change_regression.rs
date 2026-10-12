//! Original runtime-oracle traces, bounded work and actual solver integration.
use std::{
    cell::Cell,
    num::NonZeroUsize,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
    sync::Arc,
};
use timetuck::*;
type Context = (Arc<ListProblem<i32>>, ListAssignment<i32>);
fn limits() -> CorrelatedListChangeLimits {
    CorrelatedListChangeLimits {
        values_per_item: NonZeroUsize::new(64).unwrap(),
        destinations_per_value: NonZeroUsize::new(64).unwrap(),
    }
}
fn fixture(mode: &str) -> Context {
    let p = ListProblem::new(
        vec![1, 2, 3],
        vec![
            ListEntity::new(vec![2, 1]).with_value_range(if mode == "filtered" {
                vec![1, 2, 3]
            } else {
                vec![1, 2]
            }),
            ListEntity::new(vec![3]).with_value_range(vec![2, 3]),
        ],
    )
    .unwrap();
    let p = Arc::new(if mode == "optional" {
        p.allowing_unassigned()
    } else {
        p
    });
    let a = p.initial_assignment();
    (p, a)
}
fn position(p: Option<ListPosition>) -> String {
    p.map_or_else(
        || "null".into(),
        |p| format!("{}[{}]", char::from(b'A' + p.entity as u8), p.index),
    )
}
fn code(v: &ListValueChange<i32>) -> String {
    format!(
        "{} {{{}->{}}}",
        v.value(),
        position(v.source()),
        position(v.destination())
    )
}
fn source(mode: &str, seed: u64) -> Box<dyn SelectionSource<Context, ListValueChange<i32>>> {
    if mode == "filtered" {
        Box::new(SeededEntityRangeListChangeSource::with_anchor_filter(
            seed,
            limits(),
            true,
            |c: &Context, v: &i32| c.1.location(v).is_some_and(|p| p.entity != 1),
        ))
    } else {
        Box::new(SeededEntityRangeListChangeSource::new(seed, limits(), true))
    }
}
#[test]
fn longer_seeded_traces_match_actual_pinned_community_runtime() {
    let rows: Vec<_> = include_str!("fixtures/seeded_range_list_change_oracle.tsv")
        .lines()
        .map(|l| l.split('\t').collect::<Vec<_>>())
        .collect();
    assert_eq!(rows.len(), 144);
    for chunk in rows.chunks_exact(12) {
        let mode = chunk[0][0];
        let seed = chunk[0][1].parse().unwrap();
        let c = fixture(mode);
        let mut s = source(mode, seed);
        s.event(SelectionLifecycleEvent::SolveStarted, &c);
        s.event(SelectionLifecycleEvent::PhaseStarted, &c);
        let mut it = s.open(&c).peekable();
        for row in chunk {
            assert!(it.peek().is_some());
            let v = it.next().unwrap();
            assert_eq!(code(&v), row[3], "mode={mode} seed={seed} index={}", row[2]);
            assert!(c.0.apply(&mut c.1.clone(), v.movement()).is_ok());
        }
        assert!(it.peek().is_some());
    }
}
#[test]
fn openings_continue_one_stream_and_solve_reset_reproduces_it() {
    let c = fixture("plain");
    let mut s = source("plain", 0);
    let first: Vec<_> = s.open(&c).take(12).map(|v| code(&v)).collect();
    let next: Vec<_> = s.open(&c).take(12).map(|v| code(&v)).collect();
    assert_ne!(first, next);
    s.event(SelectionLifecycleEvent::SolveStarted, &c);
    assert_eq!(
        s.open(&c).take(12).map(|v| code(&v)).collect::<Vec<_>>(),
        first
    );
    s.event(SelectionLifecycleEvent::StepStarted, &c);
    let other = s.open(&c).take(12).map(|v| code(&v)).collect::<Vec<_>>();
    assert_ne!(other, first);
    let mut fresh = source("plain", 1);
    assert_eq!(
        fresh
            .open(&c)
            .take(12)
            .map(|v| code(&v))
            .collect::<Vec<_>>(),
        other
    );
    let changed = (c.0.clone(), ListAssignment::new(vec![vec![1], vec![2, 3]]));
    c.0.validate(&changed.1).unwrap();
    for item in s.open(&changed).take(32) {
        assert_eq!(item.source(), changed.1.location(item.value()));
        assert!(
            changed
                .0
                .apply(&mut changed.1.clone(), item.movement())
                .is_ok()
        );
    }
}
#[test]
fn pins_and_ranges_survive_sampling_and_reopening() {
    let p = Arc::new(
        ListProblem::new(
            vec![1, 2, 3],
            vec![
                ListEntity::new(vec![2, 1])
                    .with_pinned_prefix(1)
                    .with_value_range(vec![1, 2]),
                ListEntity::new(vec![3])
                    .with_pinned(true)
                    .with_value_range(vec![2, 3]),
            ],
        )
        .unwrap(),
    );
    let c = (p.clone(), p.initial_assignment());
    let mut s = SeededEntityRangeListChangeSource::new(7, limits(), true);
    for v in s.open(&c).take(64) {
        assert_eq!(v.value(), &1);
        assert_eq!(v.destination().unwrap().entity, 0);
        assert!(v.destination().unwrap().index >= 1);
        assert!(p.apply(&mut c.1.clone(), v.movement()).is_ok());
    }
    let full = Arc::new(
        ListProblem::new(vec![1], vec![ListEntity::new(vec![1]).with_pinned(true)]).unwrap(),
    );
    let c = (full.clone(), full.initial_assignment());
    let mut s = SeededEntityRangeListChangeSource::new(0, limits(), true);
    let mut it = s.open(&c);
    assert!(it.next().is_none());
    assert!(it.next().is_none());
    drop(it);
    let empty = Arc::new(ListProblem::<i32>::new(vec![], vec![]).unwrap());
    let c = (empty.clone(), empty.initial_assignment());
    assert!(s.open(&c).next().is_none());
}
#[test]
fn decorated_callback_panics_fuse_the_opening_without_mutating_the_layout() {
    let c = fixture("filtered");
    let panic = Rc::new(Cell::new(false));
    let armed = panic.clone();
    let mut s = SeededEntityRangeListChangeSource::with_anchor_filter(
        0,
        limits(),
        true,
        move |_: &Context, _: &i32| {
            assert!(!armed.get(), "injected anchor error");
            true
        },
    );
    let before = c.1.clone();
    let mut it = s.open(&c);
    panic.set(true);
    assert!(catch_unwind(AssertUnwindSafe(|| it.next())).is_err());
    panic.set(false);
    assert!(it.next().is_none());
    assert_eq!(c.1, before);
}
fn provider(
    p: Arc<ListProblem<i32>>,
) -> impl MoveProvider<ListProblem<i32>, ListAssignment<i32>, ListMove<i32>> {
    let selection = SelectionBuilder::new(SelectionConfig {
        order: Some(SelectionOrder::Original),
        ..Default::default()
    })
    .build(ConfiguredSelectionPolicy::default(), |_| {
        SeededEntityRangeListChangeSource::new(0, limits(), false)
    })
    .unwrap();
    ConfiguredMoveProvider::new(
        selection,
        ConfiguredProviderConfig {
            maximum_draws: 64,
            seed: 0,
        },
        move |_: &ListProblem<i32>, a: &ListAssignment<i32>, _: u64| (p.clone(), a.clone()),
        |_: &ListProblem<i32>, _: &ListAssignment<i32>, _: u64, v: &ListValueChange<i32>| {
            Some(v.movement().clone())
        },
    )
}
#[test]
fn shared_stream_source_drives_dense_and_streamed_list_search_to_same_optimum() {
    let p = ListProblem::new(
        vec![1, 2, 3],
        vec![
            ListEntity::new(vec![2, 1]).with_value_range(vec![1, 2, 3]),
            ListEntity::new(vec![3]).with_value_range(vec![1, 2, 3]),
        ],
    )
    .unwrap();
    let config = LocalSearchConfig {
        max_steps: 8,
        assert_scores: true,
        ..Default::default()
    };
    let mut n = ListConstraintNetwork::new(2, 3);
    n.add_constraint("receiver", vec![ListScope::Entity(1)], |f| {
        let ListFact::Entity(row) = f[0] else {
            unreachable!()
        };
        SimpleScore::new(row.len() as i64)
    })
    .unwrap();
    let dense = list_local_search_from_with_provider(
        &p,
        &n,
        p.initial_assignment(),
        config.clone(),
        &mut provider(Arc::new(p.clone())),
    )
    .unwrap()
    .best
    .unwrap();
    assert_eq!(dense.score, SimpleScore::new(3));
    let mut b = StreamGraphBuilder::new();
    let roots = ListStreamSources::new(&mut b);
    b.score("receiver", roots.entities.port(), |(e, row)| {
        SimpleScore::new(if *e == 1 { row.len() as i64 } else { 0 })
    })
    .unwrap();
    let mut director =
        ListStreamScoreDirector::new(&p, b.build(), roots, p.initial_assignment()).unwrap();
    let streamed = director
        .run_local_search_with_provider(config, &mut provider(Arc::new(p.clone())))
        .unwrap()
        .best
        .unwrap();
    assert_eq!(streamed.score, dense.score);
    assert_eq!(streamed.assignment, dense.assignment);
}
