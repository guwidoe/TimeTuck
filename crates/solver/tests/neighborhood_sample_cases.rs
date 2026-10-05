// Adapted from pinned Community SamplersTest and SampleTest, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md. Native iterator/set
// ownership and seeded randomness replace Java containers and PRNG traces.
use std::{
    cell::{Cell, RefCell},
    collections::{BTreeSet, HashSet},
    hash::{Hash, Hasher},
};
use timetuck::*;
fn draw(n: usize, s: &mut impl Sampler<usize>) -> Option<NeighborhoodSample<usize>> {
    assemble_sample(0..n, s, &mut SampleRandom::new(0)).unwrap()
}
#[test]
fn exactly_yields_exact_size_on_a_larger_source() {
    assert_eq!(
        draw(10, &mut SizeSampler::exactly(4).unwrap())
            .unwrap()
            .len(),
        4
    );
}
#[test]
fn exactly_returns_null_when_source_is_smaller_than_minimum() {
    assert!(draw(3, &mut SizeSampler::exactly(10).unwrap()).is_none());
}
#[test]
fn between_returns_null_when_source_is_smaller_than_minimum() {
    assert!(draw(2, &mut SizeSampler::between(3, 7).unwrap()).is_none());
}
#[test]
fn up_to_and_all_have_no_floor_and_stay_productive_on_a_short_source() {
    for mut s in [SizeSampler::up_to(8).unwrap(), SizeSampler::all()] {
        assert_eq!(draw(1, &mut s).unwrap().len(), 1);
    }
}
#[test]
fn exactly_one_stops_at_the_first_candidate() {
    assert_eq!(
        draw(10, &mut SizeSampler::exactly(1).unwrap())
            .unwrap()
            .len(),
        1
    );
}
#[test]
fn exactly_one_consumes_only_one_element_from_the_source() {
    let count = Cell::new(0);
    let source = (0..10).inspect(|_| count.set(count.get() + 1));
    let sample = assemble_sample(
        source,
        &mut SizeSampler::exactly(1).unwrap(),
        &mut SampleRandom::new(0),
    )
    .unwrap()
    .unwrap();
    assert_eq!(sample.len(), 1);
    assert_eq!(count.get(), 1);
}
fn sizes(min: usize, max: usize, mut rule: SizeSampler) {
    let mut r = SampleRandom::new(0);
    let mut sizes = HashSet::new();
    for _ in 0..50 {
        let n = assemble_sample(0..20, &mut rule, &mut r)
            .unwrap()
            .unwrap()
            .len();
        assert!((min..=max).contains(&n));
        sizes.insert(n);
    }
    assert!(sizes.len() > 1);
}
#[test]
fn up_to_stays_within_range_and_varies() {
    sizes(1, 8, SizeSampler::up_to(8).unwrap());
}
#[test]
fn between_stays_within_range_and_varies() {
    sizes(3, 7, SizeSampler::between(3, 7).unwrap());
}
#[test]
fn between_minimum_size_is_the_declared_floor_not_the_drawn_target() {
    let mut r = SampleRandom::new(0);
    let mut s = SizeSampler::between(2, 5).unwrap();
    for _ in 0..50 {
        assert!(assemble_sample(0..20, &mut s, &mut r).unwrap().is_some());
        assert_eq!(s.minimum_size(), 2);
    }
}
#[test]
fn all_drains_the_source() {
    assert_eq!(draw(15, &mut SizeSampler::all()).unwrap().len(), 15);
}
#[test]
fn constructor_guards_reject_invalid_sizes() {
    assert!(SizeSampler::exactly(0).is_err());
    assert!(SizeSampler::up_to(0).is_err());
    assert!(SizeSampler::between(3, 2).is_err());
}
#[test]
fn evaluate_sees_the_first_candidate_at_size_zero() {
    let mut recorded = Vec::new();
    let mut rule = |n, _: &usize| {
        recorded.push(n);
        SampleDecision::AcceptAndStop
    };
    let s = draw(5, &mut rule).unwrap();
    assert_eq!(recorded, vec![0]);
    assert_eq!(s.len(), 1);
}
#[test]
fn reject_at_size_zero_keeps_drawing_for_a_first_member() {
    let mut calls = 0;
    let mut rule = |_, _: &usize| {
        calls += 1;
        if calls < 3 {
            SampleDecision::Reject
        } else {
            SampleDecision::AcceptAndStop
        }
    };
    let s = draw(5, &mut rule).unwrap();
    assert_eq!(s.len(), 1);
    assert_eq!(s.representative(), Some(&2));
}
struct Trace<'a>(&'a RefCell<Vec<&'static str>>);
impl Sampler<usize> for Trace<'_> {
    fn reset(&mut self, _: &mut SampleRandom) {
        self.0.borrow_mut().push("reset");
    }
    fn evaluate(&mut self, _: usize, _: &usize) -> SampleDecision {
        self.0.borrow_mut().push("evaluate");
        SampleDecision::AcceptAndStop
    }
}
#[test]
fn reset_runs_before_the_first_evaluate_call() {
    let order = RefCell::new(vec![]);
    draw(3, &mut Trace(&order));
    assert_eq!(*order.borrow(), vec!["reset", "evaluate"]);
}
struct Rule {
    minimum: usize,
    target: usize,
    decision: SampleDecision,
}
impl Sampler<usize> for Rule {
    fn minimum_size(&self) -> usize {
        self.minimum
    }
    fn target_size(&self) -> usize {
        self.target
    }
    fn evaluate(&mut self, _: usize, _: &usize) -> SampleDecision {
        self.decision
    }
}
#[test]
fn minimum_size_below_one_throws() {
    let err = assemble_sample(
        0..3,
        &mut Rule {
            minimum: 0,
            target: 1,
            decision: SampleDecision::AcceptAndStop,
        },
        &mut SampleRandom::new(0),
    )
    .unwrap_err();
    assert_eq!(err, SampleError::InvalidMinimum);
    assert!(err.to_string().contains("minimum size"));
}
#[test]
fn sampler_stopping_below_its_own_minimum_size_throws() {
    let err = assemble_sample(
        0..10,
        &mut Rule {
            minimum: 5,
            target: 5,
            decision: SampleDecision::AcceptAndStop,
        },
        &mut SampleRandom::new(0),
    )
    .unwrap_err();
    assert_eq!(
        err,
        SampleError::StoppedBelowMinimum {
            actual: 1,
            minimum: 5
        }
    );
    assert!(err.to_string().contains("minimum size"));
}
#[test]
fn dry_stop_without_sampler_stop_returns_null_not_an_exception() {
    assert!(
        draw(
            2,
            &mut Rule {
                minimum: 5,
                target: 5,
                decision: SampleDecision::Accept
            }
        )
        .is_none()
    );
}
#[test]
fn pillar_forwards_reset_to_the_wrapped_sampler() {
    let order = RefCell::new(vec![]);
    assemble_keyed_sample(
        0..3,
        &"key",
        &mut PillarSampler(Trace(&order)),
        &mut SampleRandom::new(0),
    )
    .unwrap();
    assert_eq!(order.borrow().iter().filter(|&&s| s == "reset").count(), 1);
}
#[test]
fn pillar_forwards_evaluate_decisions_unchanged() {
    for d in [
        SampleDecision::Accept,
        SampleDecision::Reject,
        SampleDecision::AcceptAndStop,
        SampleDecision::Stop,
    ] {
        let mut s = PillarSampler(move |_, _: &usize| d);
        assert_eq!(s.evaluate(0, &1), d);
    }
}
#[test]
fn pillar_forwards_minimum_size_and_enforces_it_through_the_keyed_assemble() {
    let mut s = PillarSampler(SizeSampler::exactly(4).unwrap());
    assert_eq!(<PillarSampler<_> as Sampler<usize>>::minimum_size(&s), 4);
    assert!(
        assemble_keyed_sample(0..3, &"key", &mut s, &mut SampleRandom::new(0))
            .unwrap()
            .is_none()
    );
}
#[test]
fn all_reports_an_expected_size_when_given_one() {
    assert_eq!(SizeSampler::all().target_size(), 1);
    let mut s = SizeSampler::all_with_expected_size(12).unwrap();
    assert_eq!(s.target_size(), 12);
    assert_eq!(draw(5, &mut s).unwrap().len(), 5);
    assert_eq!(
        SizeSampler::all_with_expected_size(0).unwrap_err(),
        SampleError::InvalidExpectedSize
    );
    assert!(
        SizeSampler::all_with_expected_size(0)
            .unwrap_err()
            .to_string()
            .contains("expected size (0)")
    );
}
#[test]
fn between_reports_its_drawn_target_size_after_reset() {
    let mut s = SizeSampler::between(3, 7).unwrap();
    s.reset(&mut SampleRandom::new(0));
    assert!((3..=7).contains(&s.target_size()));
    assert_eq!(SizeSampler::exactly(4).unwrap().target_size(), 4);
}
#[test]
fn pillar_forwards_target_size() {
    let mut s = PillarSampler(SizeSampler::exactly(6).unwrap());
    <PillarSampler<_> as KeyedSampler<&str, usize>>::reset_for_key(
        &mut s,
        &mut SampleRandom::new(0),
        &"key",
    );
    assert_eq!(<PillarSampler<_> as Sampler<usize>>::minimum_size(&s), 6);
    assert_eq!(<PillarSampler<_> as Sampler<usize>>::target_size(&s), 6);
}
#[test]
fn assemble_rejects_a_sampler_whose_target_size_is_below_its_minimum() {
    let err = assemble_sample(
        0..5,
        &mut Rule {
            minimum: 3,
            target: 1,
            decision: SampleDecision::Accept,
        },
        &mut SampleRandom::new(0),
    )
    .unwrap_err();
    assert_eq!(
        err,
        SampleError::TargetBelowMinimum {
            target: 1,
            minimum: 3
        }
    );
    assert!(err.to_string().contains("target size (1)"));
    assert!(err.to_string().contains("at least the minimum size (3)"));
}
fn hash(v: &impl Hash) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    v.hash(&mut h);
    h.finish()
}
#[test]
fn of_removes_duplicates() {
    let s = NeighborhoodSample::new(["a", "b", "a"]);
    assert_eq!(s.len(), 2);
    assert_eq!(s.members(), &["a", "b"]);
}
#[test]
fn of_copies_so_later_mutation_does_not_leak() {
    let mut v = vec!["a", "b"];
    let s = NeighborhoodSample::new(v.iter().copied());
    v.push("c");
    assert_eq!(s.len(), 2);
    assert!(!s.contains(&"c"));
}
#[test]
fn of_always_copies_even_a_sequenced_set() {
    let mut v = BTreeSet::from(["a", "b"]);
    let s = NeighborhoodSample::new(v.iter().copied());
    v.insert("c");
    assert_eq!(s.len(), 2);
    assert!(!s.contains(&"c"));
}
#[test]
fn null_is_a_legal_member() {
    let s = NeighborhoodSample::new([Some("a"), None]);
    assert_eq!(s.len(), 2);
    assert!(s.contains(&None));
    assert!(!s.contains(&Some("zzz")));
}
#[test]
fn equality_is_order_insensitive() {
    let a = NeighborhoodSample::new(["a", "b", "c"]);
    let b = NeighborhoodSample::new(["c", "a", "b"]);
    assert_eq!(a, b);
    assert_eq!(hash(&a), hash(&b));
}
#[test]
fn equality_tolerates_null_members() {
    let a = NeighborhoodSample::new([Some("a"), None]);
    let b = NeighborhoodSample::new([None, Some("a")]);
    assert_eq!(a, b);
    assert_eq!(hash(&a), hash(&b));
}
#[test]
fn different_members_are_not_equal() {
    assert_ne!(
        NeighborhoodSample::new(["a"]),
        NeighborhoodSample::new(["b"])
    );
}
#[test]
fn equality_holds_across_set_implementations() {
    let a = NeighborhoodSample::new(HashSet::from(["c", "a", "b"]));
    let b = NeighborhoodSample::new(["a", "b", "c"]);
    assert_eq!(a, b);
    assert_eq!(b, a);
    assert_eq!(hash(&a), hash(&b));
}
#[test]
fn equality_holds_across_set_implementations_above_the_list_threshold() {
    let a = NeighborhoodSample::new((0..20).collect::<BTreeSet<_>>().into_iter().rev());
    let b = NeighborhoodSample::new(0..20);
    assert_eq!(a, b);
    assert_eq!(b, a);
    assert_eq!(hash(&a), hash(&b));
}
