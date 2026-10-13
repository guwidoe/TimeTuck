// Adapted pinned Community SubPillarSwapMoveProviderTest, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;
#[test]
fn shared_sampler_instance_on_both_sides_matches_two_separate_instances() {
    let p = FiniteProblem::new(
        (0..10)
            .map(|i| Variable::new(vec![0, 1]).with_initial(usize::from(i >= 5)))
            .collect(),
    )
    .unwrap();
    let a = p.initial_assignment();
    let policy = PillarSelectionPolicy::subsets(2, 4).unwrap();
    let separate = SharedSamplerSubPillarSwaps {
        left: StatefulPillarSampler::shared(policy.clone()),
        right: StatefulPillarSampler::shared(policy.clone()),
        seed: 0,
        attempts_per_step: 200,
    };
    let expected = separate
        .select(&p, &a, 0)
        .unwrap()
        .take(200)
        .collect::<Vec<_>>();
    let shared = StatefulPillarSampler::shared(policy);
    let together = SharedSamplerSubPillarSwaps {
        left: shared.clone(),
        right: shared.clone(),
        seed: 0,
        attempts_per_step: 200,
    };
    let actual = together
        .select(&p, &a, 0)
        .unwrap()
        .take(200)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
    assert_eq!(actual.len(), 200);
    assert_eq!(shared.borrow().reset_count(), 400);
    assert_eq!(shared.borrow().completed_count(), 400);
    assert_eq!(shared.borrow().pending_rows(), 0);
    assert_eq!(separate.left.borrow().completed_count(), 200);
    assert_eq!(separate.right.borrow().completed_count(), 200);
}
