// Original native sampler regressions; no upstream credits.
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
#[test]
fn cache_budgets_preserve_traces_and_never_exceed_the_limit() {
    let left = [0, 1, 2, 3, 4, 5];
    let right: Vec<_> = (0..50).collect();
    let run = |budget| {
        let mut pairs = FilteredPairSampler::new(
            &left,
            &right,
            |a, b| (*a == 0 && *b == 49) || (*a == 3 && b % 2 == 0) || (*a == 5 && *b < 7),
            79,
            budget,
        );
        let mut out = vec![];
        for _ in 0..1000 {
            out.push(pairs.next().map(|(a, b)| (*a, *b)).unwrap());
            assert!(pairs.cached_match_count() <= budget);
        }
        assert_eq!(pairs.remaining_left(), 3);
        out
    };
    let expected = run(0);
    for budget in [1, 3, 7, 16, 100, usize::MAX] {
        assert_eq!(run(budget), expected);
    }
    for a in [0, 3, 5] {
        assert!(expected.iter().any(|(left, _)| *left == a));
    }
}
#[test]
fn empty_rows_are_scanned_once_and_nonempty_cached_rows_stop_rechecking() {
    let calls = Cell::new(0);
    let left = [0];
    let right = [0, 1, 2, 3];
    let mut pairs = FilteredPairSampler::new(
        &left,
        &right,
        |_, _| {
            calls.set(calls.get() + 1);
            true
        },
        0,
        4,
    );
    assert_eq!(calls.get(), 0);
    assert_eq!(pairs.by_ref().take(100).count(), 100);
    assert_eq!(calls.get(), 4);
    assert_eq!(pairs.cached_match_count(), 4);
    let left = [0, 1, 2];
    calls.set(0);
    let mut empty = FilteredPairSampler::new(
        &left,
        &right,
        |_, _| {
            calls.set(calls.get() + 1);
            false
        },
        0,
        4,
    );
    assert_eq!(empty.size_hint(), (0, None));
    assert_eq!(empty.next(), None);
    assert_eq!(calls.get(), 12);
    assert_eq!(empty.next(), None);
    assert_eq!(calls.get(), 12);
    assert_eq!(empty.remaining_left(), 0);
    assert_eq!(empty.size_hint(), (0, Some(0)));
    let mut empty =
        FilteredPairSampler::new(&left, &[] as &[i32], |_, _| panic!("empty predicate"), 0, 0);
    assert_eq!(empty.next(), None);
}
#[test]
fn a_predicate_panic_does_not_retire_a_left_or_install_a_partial_cache() {
    let calls = Cell::new(0);
    let mut pairs = FilteredPairSampler::new(
        &[0, 1],
        &[0, 1, 2],
        |_, _| {
            calls.set(calls.get() + 1);
            assert_ne!(calls.get(), 2, "injected predicate panic");
            true
        },
        0,
        6,
    );
    assert!(catch_unwind(AssertUnwindSafe(|| pairs.next())).is_err());
    assert_eq!(pairs.cached_match_count(), 0);
    assert_eq!(pairs.remaining_left(), 2);
    let values: Vec<_> = pairs.by_ref().take(100).map(|(a, b)| (*a, *b)).collect();
    assert!(values.iter().any(|(a, _)| *a == 0));
    assert!(values.iter().any(|(a, _)| *a == 1));
    assert_eq!(pairs.cached_match_count(), 6);
}
#[test]
fn equal_payloads_retain_occurrence_identity_without_clone_bounds() {
    struct Value(i32);
    let left = [Value(1), Value(1)];
    let right = [Value(9), Value(9)];
    let mut seen = [[false; 2]; 2];
    let pairs = FilteredPairSampler::new(&left, &right, |a, b| a.0 < b.0, 17, 4);
    for (a, b) in pairs.take(1000) {
        let ai = usize::from(std::ptr::eq(a, &left[1]));
        let bi = usize::from(std::ptr::eq(b, &right[1]));
        seen[ai][bi] = true;
    }
    assert!(seen.into_iter().flatten().all(|s| s));
}

struct Improvements {
    left: [usize; 3],
    right: [i32; 4],
    cache: usize,
}
impl MoveProvider<FiniteProblem<i32>, Assignment<i32>, Move<i32>> for Improvements {
    fn moves<'a>(
        &'a mut self,
        p: &'a FiniteProblem<i32>,
        a: &'a Assignment<i32>,
        step: u64,
    ) -> Box<dyn Iterator<Item = Move<i32>> + 'a>
    where
        Move<i32>: 'a,
    {
        Box::new(
            FilteredPairSampler::new(
                &self.left,
                &self.right,
                move |variable, value| {
                    p.variables()[*variable].domain().contains(value)
                        && *value > a.values()[*variable].unwrap()
                },
                step,
                self.cache,
            )
            .take(32)
            .map(|(variable, value)| Move::Change {
                variable: *variable,
                value: Some(*value),
            }),
        )
    }
}
#[test]
fn filtered_provider_rebuilds_from_committed_state_in_both_scoring_backends() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![0, 1, 2]).with_initial(0),
        Variable::new(vec![0, 1, 2, 3]).with_initial(0),
        Variable::new(vec![0]).with_initial(0),
    ])
    .unwrap();
    let mut network = ConstraintNetwork::new(3);
    for i in 0..3 {
        network
            .add_constraint(format!("value-{i}"), vec![i], |v| {
                SimpleScore::new(i64::from(*v[0].unwrap()))
            })
            .unwrap();
    }
    for cache in [0, 64] {
        for stream in [false, true] {
            let mut provider = Improvements {
                left: [0, 1, 2],
                right: [0, 1, 2, 3],
                cache,
            };
            let config = LocalSearchConfig {
                assert_scores: true,
                ..Default::default()
            };
            let result = if stream {
                let mut b = StreamGraphBuilder::new();
                let source = b.source::<usize, (usize, Option<i32>)>();
                b.score("value", source.port(), |(_, v)| {
                    SimpleScore::new(i64::from(v.unwrap()))
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
                    &network,
                    p.initial_assignment(),
                    config,
                    &mut provider,
                )
                .unwrap()
            };
            assert_eq!(result.best.unwrap().score, SimpleScore::new(5));
            assert!((2..=5).contains(&result.completed_steps));
        }
    }
}
