//! Original bounded-work, occurrence identity and local-search integration checks.
use std::{cell::Cell, num::NonZeroUsize, rc::Rc};
use timetuck::*;
fn budget(n: usize) -> NonZeroUsize {
    NonZeroUsize::new(n).unwrap()
}

#[test]
fn probes_consume_exact_budgets_without_lookahead_and_fuse_real_exhaustion() {
    let calls = Cell::new(0);
    let source = std::iter::from_fn(|| {
        let n = calls.get();
        calls.set(n + 1);
        (n != 3).then_some(n)
    });
    let mut filter = BoundedFilter::new(source, budget(2), |n: &usize| *n == 2);
    assert_eq!(calls.get(), 0);
    assert_eq!(filter.probe(), FilterProbe::BudgetExhausted);
    assert_eq!(calls.get(), 2);
    assert_eq!(filter.probe(), FilterProbe::Found(2));
    assert_eq!(calls.get(), 3);
    assert_eq!(filter.probe(), FilterProbe::SourceExhausted);
    assert_eq!(filter.probe(), FilterProbe::SourceExhausted);
    assert_eq!(
        calls.get(),
        4,
        "do not resume a non-fused source after None"
    );
}

#[test]
fn retirement_tracks_occurrences_including_nulls_and_only_the_latest_draw() {
    let mut pool = RetiringRandom::from_slots(
        vec![None, Some(None), Some(Some(7)), None, Some(Some(7))],
        3,
    );
    assert_eq!(pool.retire(), Err(NoDrawToRetire));
    assert_eq!(pool.remaining(), 3);
    let mut drained = Vec::new();
    while let Some(value) = pool.next() {
        drained.push(value);
        pool.retire().unwrap();
        assert_eq!(pool.retire(), Err(NoDrawToRetire));
    }
    drained.sort();
    assert_eq!(drained, [None, Some(7), Some(7)]);
    assert_eq!(pool.size_hint(), (0, Some(0)));
    let mut pool = RetiringRandom::new(0..10, 77);
    let first = pool.next().unwrap();
    let mut last = first;
    for _ in 0..100 {
        last = pool.next().unwrap();
        if last != first {
            break;
        }
    }
    assert_ne!(first, last);
    pool.retire().unwrap();
    let mut remaining = Vec::new();
    while let Some(v) = pool.next() {
        remaining.push(v);
        pool.retire().unwrap();
    }
    remaining.sort();
    assert_eq!(
        remaining,
        (0..10).filter(|v| *v != last).collect::<Vec<_>>()
    );
}

#[test]
fn borrowed_draws_need_no_clone_and_owned_iteration_clones_only_on_demand() {
    struct Unique(i32);
    let mut pool = RetiringRandom::new([Unique(5)], 1);
    assert_eq!(pool.draw().unwrap().0, 5);
    pool.retire().unwrap();
    assert!(pool.draw().is_none());
    struct Counted(Rc<Cell<usize>>);
    impl Clone for Counted {
        fn clone(&self) -> Self {
            self.0.set(self.0.get() + 1);
            Self(self.0.clone())
        }
    }
    let clones = Rc::new(Cell::new(0));
    let mut pool = RetiringRandom::new((0..100).map(|_| Counted(clones.clone())), 1);
    assert_eq!(clones.get(), 0);
    pool.draw().unwrap();
    assert_eq!(clones.get(), 0);
    pool.next().unwrap();
    assert_eq!(clones.get(), 1);
    pool.retire().unwrap();
    assert_eq!(clones.get(), 1);
}

#[test]
fn every_dead_pool_has_a_finite_exact_probe_bound_and_never_reopens() {
    for left_count in 0..20 {
        for attempts in 1..8 {
            let calls = Cell::new(0);
            let per_left = (0..left_count).map(|_| Cell::new(0)).collect::<Vec<_>>();
            let mut pairs = RetiringPairs::new(
                0..left_count,
                budget(attempts),
                17,
                |left: &usize, _: &mut SampleRandom| {
                    calls.set(calls.get() + 1);
                    per_left[*left].set(per_left[*left].get() + 1);
                    None::<i32>
                },
            );
            assert_eq!(calls.get(), 0);
            assert_eq!(pairs.next(), None);
            assert_eq!(pairs.remaining_lefts(), 0);
            assert_eq!(calls.get(), left_count * attempts);
            assert!(per_left.iter().all(|n| n.get() == attempts));
            assert_eq!(pairs.next(), None);
            assert_eq!(calls.get(), left_count * attempts);
        }
    }
}

#[test]
fn seeded_pairs_are_reproducible_and_fair_to_left_rows_not_pair_counts() {
    let run = |seed| {
        RetiringPairs::new(
            [0, 1],
            budget(3),
            seed,
            |left: &i32, r: &mut SampleRandom| {
                Some(r.index(budget(if *left == 0 { 1 } else { 9 })))
            },
        )
        .take(20_000)
        .collect::<Vec<_>>()
    };
    let draws = run(42);
    assert_eq!(draws, run(42));
    assert_ne!(draws, run(43));
    let first = draws.iter().filter(|(left, _)| *left == 0).count();
    assert!((9000..11000).contains(&first), "left draws: {first}");
    let mut right_counts = [0; 9];
    for (left, right) in draws {
        if left == 0 {
            assert_eq!(right, 0);
        } else {
            right_counts[right] += 1;
        }
    }
    assert!(right_counts.into_iter().all(|n| (800..1400).contains(&n)));
}

struct ImprovingPairs;
impl MoveProvider<FiniteProblem<i32>, Assignment<i32>, Move<i32>> for ImprovingPairs {
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
            RetiringPairs::new(
                0..p.variables().len(),
                budget(3),
                19_u64.wrapping_add(step),
                move |i: &usize, _: &mut SampleRandom| {
                    let v = &p.variables()[*i];
                    (!v.is_pinned() && a.values()[*i] == Some(0) && v.domain().contains(&1))
                        .then_some(1)
                },
            )
            .take(16)
            .map(|(variable, value)| Move::Change {
                variable,
                value: Some(value),
            }),
        )
    }
}
#[test]
fn retiring_pair_providers_solve_in_both_backends_and_reopen_after_each_step() {
    let p = FiniteProblem::new(
        (0..8)
            .map(|i| {
                Variable::new(if i == 1 { vec![0] } else { vec![0, 1] })
                    .with_initial(0)
                    .with_pinned(i == 0)
            })
            .collect(),
    )
    .unwrap();
    for stream in [false, true] {
        let config = LocalSearchConfig {
            max_steps: 10,
            max_move_evaluations: Some(128),
            assert_scores: true,
            ..Default::default()
        };
        let result = if stream {
            let mut b = StreamGraphBuilder::new();
            let source = b.source::<usize, (usize, Option<i32>)>();
            b.score("sum", source.port(), |(_, v)| {
                SimpleScore::new(i64::from(v.unwrap()))
            })
            .unwrap();
            stream_local_search_from_with_provider(
                &p,
                b.build(),
                source,
                p.initial_assignment(),
                config,
                &mut ImprovingPairs,
            )
            .unwrap()
        } else {
            let mut network = ConstraintNetwork::new(8);
            network
                .add_constraint("sum", (0..8).collect(), |vs| {
                    SimpleScore::new(vs.iter().map(|v| i64::from(*v.unwrap())).sum())
                })
                .unwrap();
            local_search_from_with_provider(
                &p,
                &network,
                p.initial_assignment(),
                config,
                &mut ImprovingPairs,
            )
            .unwrap()
        };
        let best = result.best.as_ref().unwrap();
        assert_eq!(best.score, SimpleScore::new(6));
        assert_eq!(
            best.assignment.values(),
            &[
                Some(0),
                Some(0),
                Some(1),
                Some(1),
                Some(1),
                Some(1),
                Some(1),
                Some(1)
            ]
        );
        assert_eq!(result.completed_steps, 6);
        assert!(result.move_evaluations() <= 96);
    }
}
