//! Original sampler boundary, reproducibility, keyed lifecycle and solver checks.
use std::{cell::Cell, num::NonZeroUsize};
use timetuck::*;
#[test]
fn seeded_sequences_repeat_without_resetting_random_state_per_assembly() {
    let sequence = |seed| {
        let mut random = SampleRandom::new(seed);
        let mut rule = SizeSampler::between(2, 8).unwrap();
        (0..64)
            .map(|_| {
                assemble_sample(0..100, &mut rule, &mut random)
                    .unwrap()
                    .unwrap()
                    .len()
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(sequence(13), sequence(13));
    assert_ne!(sequence(13), sequence(14));
    let mut r = SampleRandom::new(91);
    for bound in [1, 2, 37, usize::MAX] {
        for _ in 0..100 {
            assert!(r.index(NonZeroUsize::new(bound).unwrap()) < bound);
        }
    }
}
#[test]
fn accepted_duplicates_do_not_advance_the_floor_and_stopping_does_not_prefetch() {
    let mut source = [0, 0, 1, 1, 2, 3].into_iter();
    let mut seen = vec![];
    let mut rule = |n, v: &i32| {
        seen.push((n, *v));
        if n == 2 {
            SampleDecision::AcceptAndStop
        } else {
            SampleDecision::Accept
        }
    };
    let s = assemble_sample(&mut source, &mut rule, &mut SampleRandom::new(0))
        .unwrap()
        .unwrap();
    assert_eq!(s.members(), &[0, 1, 2]);
    assert_eq!(seen, vec![(0, 0), (1, 1), (2, 2)]);
    assert_eq!(source.next(), Some(3));
    assert!(
        assemble_sample(
            [7; 20],
            &mut SizeSampler::exactly(2).unwrap(),
            &mut SampleRandom::new(0)
        )
        .unwrap()
        .is_none()
    );
}
#[test]
fn empty_sources_huge_hints_and_caller_bounded_rejection_do_not_allocate_to_target() {
    let mut r = SampleRandom::new(0);
    assert!(
        assemble_sample(std::iter::empty::<()>(), &mut SizeSampler::all(), &mut r)
            .unwrap()
            .is_none()
    );
    assert!(
        assemble_sample([1], &mut SizeSampler::exactly(usize::MAX).unwrap(), &mut r)
            .unwrap()
            .is_none()
    );
    let s = assemble_sample(
        [1],
        &mut SizeSampler::all_with_expected_size(usize::MAX).unwrap(),
        &mut r,
    )
    .unwrap()
    .unwrap();
    assert_eq!(s.members(), &[1]);
    let count = Cell::new(0);
    let source = (0..).inspect(|_| count.set(count.get() + 1)).take(100);
    let mut reject = |_, _: &usize| SampleDecision::Reject;
    assert!(
        assemble_sample(source, &mut reject, &mut r)
            .unwrap()
            .is_none()
    );
    assert_eq!(count.get(), 100);
    let s = NeighborhoodSample::new([3, 1, 3, 2]);
    assert_eq!(s.representative(), Some(&3));
    assert_eq!(s.into_iter().collect::<Vec<_>>(), vec![3, 1, 2]);
}
struct KeyRule {
    target: usize,
    resets: usize,
    plain_resets: usize,
}
impl Sampler<usize> for KeyRule {
    fn reset(&mut self, _: &mut SampleRandom) {
        self.plain_resets += 1;
    }
    fn minimum_size(&self) -> usize {
        self.target
    }
    fn target_size(&self) -> usize {
        self.target
    }
    fn evaluate(&mut self, n: usize, _: &usize) -> SampleDecision {
        if n + 1 == self.target {
            SampleDecision::AcceptAndStop
        } else {
            SampleDecision::Accept
        }
    }
}
impl KeyedSampler<usize, usize> for KeyRule {
    fn reset_for_key(&mut self, _: &mut SampleRandom, key: &usize) {
        self.target = *key;
        self.resets += 1;
    }
}
#[test]
fn keyed_rules_reset_once_and_validate_before_consuming_any_source() {
    let mut rule = KeyRule {
        target: 0,
        resets: 0,
        plain_resets: 0,
    };
    let mut r = SampleRandom::new(0);
    for size in [3, 1, 5] {
        let object: &mut dyn KeyedSampler<usize, usize> = &mut rule;
        let s = assemble_keyed_sample(0..8, &size, object, &mut r)
            .unwrap()
            .unwrap();
        assert_eq!(s.len(), size);
    }
    let pulls = Cell::new(0);
    let err = assemble_keyed_sample(
        (0..8).inspect(|_| pulls.set(pulls.get() + 1)),
        &0,
        &mut rule,
        &mut r,
    )
    .unwrap_err();
    assert_eq!(err, SampleError::InvalidMinimum);
    assert_eq!(pulls.get(), 0);
    assert_eq!(rule.resets, 4);
    assert_eq!(rule.plain_resets, 0);
}
#[test]
fn sampled_composites_work_with_both_solver_scoring_paths() {
    let p = FiniteProblem::new(
        (0..7)
            .map(|i| {
                Variable::new(vec![0, 1])
                    .with_initial(0)
                    .with_pinned(i == 6)
            })
            .collect(),
    )
    .unwrap();
    for stream in [false, true] {
        let mut random = SampleRandom::new(42);
        let mut rule = SizeSampler::up_to(3).unwrap();
        let mut samples = vec![];
        let mut provider = |p: &FiniteProblem<i32>, a: &Assignment<i32>, _| {
            let source = (0..p.variables().len())
                .filter(|&i| !p.variables()[i].is_pinned() && a.values()[i] == Some(0));
            assemble_sample(source, &mut rule, &mut random)
                .unwrap()
                .into_iter()
                .map(|sample| {
                    samples.push(sample.members().to_vec());
                    Move::Composite(
                        sample
                            .into_iter()
                            .map(|variable| Move::Change {
                                variable,
                                value: Some(1),
                            })
                            .collect(),
                    )
                })
                .collect::<Vec<_>>()
        };
        let config = LocalSearchConfig {
            max_steps: 10,
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
                &mut provider,
            )
            .unwrap()
        } else {
            let mut n = ConstraintNetwork::new(7);
            n.add_constraint("sum", (0..7).collect(), |v| {
                SimpleScore::new(v.iter().map(|v| i64::from(*v.unwrap())).sum())
            })
            .unwrap();
            local_search_from_with_provider(&p, &n, p.initial_assignment(), config, &mut provider)
                .unwrap()
        };
        let best = result.best.unwrap();
        assert_eq!(best.score, SimpleScore::new(6));
        assert_eq!(
            best.assignment.values(),
            &[
                Some(1),
                Some(1),
                Some(1),
                Some(1),
                Some(1),
                Some(1),
                Some(0)
            ]
        );
        assert!(samples.iter().all(|s| (1..=3).contains(&s.len())));
        assert_eq!(samples.concat(), (0..6).collect::<Vec<_>>());
        assert_eq!(p.initial_assignment().values(), &[Some(0); 7]);
    }
}

#[test]
fn stop_discards_its_candidate_and_distinguishes_premature_stop_from_exhaustion() {
    let mut source = 0..5;
    let mut rule = |n, _: &usize| {
        if n == 2 {
            SampleDecision::Stop
        } else {
            SampleDecision::Accept
        }
    };
    let mut random = SampleRandom::new(0);
    let sample = assemble_sample(&mut source, &mut rule, &mut random)
        .unwrap()
        .unwrap();
    assert_eq!(sample.members(), &[0, 1]);
    assert_eq!(source.next(), Some(3));
    let mut stop = |_, _: &usize| SampleDecision::Stop;
    assert_eq!(
        assemble_sample(0..5, &mut stop, &mut random).unwrap_err(),
        SampleError::StoppedBelowMinimum {
            actual: 0,
            minimum: 1
        }
    );
    assert!(
        assemble_sample(0..0, &mut stop, &mut random)
            .unwrap()
            .is_none()
    );
}
