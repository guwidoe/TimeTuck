//! Original sampler state, complete sample ownership and real current-fact controls.
use timetuck::*;
#[test]
fn partial_sampling_resets_and_finished_samples_keep_all_rows_and_ownership() {
    let mut sampler = StatefulPillarSampler::new(PillarSelectionPolicy::whole());
    let left = Pillar::new(0..5).unwrap();
    let right = Pillar::new(10..13).unwrap();
    assert!(sampler.begin(&left, 1));
    assert_eq!(sampler.next_member(), Some(vec![0]));
    assert_eq!(sampler.remaining_rows(), 4);
    let full = sampler.finish().unwrap();
    assert_eq!(full, left);
    assert_eq!(sampler.pending_rows(), 0);
    assert!(sampler.begin(&left, 2));
    assert_eq!(sampler.next_member(), Some(vec![0]));
    assert!(sampler.begin(&right, 3));
    assert_eq!(sampler.remaining_rows(), 3);
    assert_eq!(sampler.finish(), Some(right));
    assert_eq!(full, left);
    assert_eq!(sampler.reset_count(), 3);
    assert_eq!(sampler.completed_count(), 2);
    assert_eq!(sampler.finish(), None);
    sampler.begin(&left, 4);
    sampler.reset();
    assert_eq!(sampler.next_member(), None);
    assert_eq!(sampler.finish(), None);
}
#[test]
fn independent_rules_execute_on_live_assignments_and_pins() {
    let p = FiniteProblem::new(
        (0..12)
            .map(|i| {
                Variable::new(vec![0, 1])
                    .with_initial(i % 2)
                    .with_pinned(i == 0)
            })
            .collect(),
    )
    .unwrap();
    let mut a = p.initial_assignment();
    let provider = SharedSamplerSubPillarSwaps {
        left: StatefulPillarSampler::shared(PillarSelectionPolicy::subsets(1, 1).unwrap()),
        right: StatefulPillarSampler::shared(PillarSelectionPolicy::subsets(3, 3).unwrap()),
        seed: 29,
        attempts_per_step: 100,
    };
    for step in 0..10 {
        let moves = provider.select(&p, &a, step).unwrap().collect::<Vec<_>>();
        assert!(!moves.is_empty());
        for m in &moves {
            assert_eq!(m.touched_variables().len(), 4);
            assert!(!m.touched_variables().contains(&0));
            let Move::Composite(writes) = m else { panic!() };
            assert_eq!(writes.len(), 4);
            let first = match &writes[0] {
                Move::Change { variable, .. } => *variable,
                _ => panic!(),
            };
            let opposite = match &writes[1] {
                Move::Change { variable, .. } => *variable,
                _ => panic!(),
            };
            assert_ne!(a.values()[first], a.values()[opposite]);
            let original = a.clone();
            {
                let _undo = p.apply(&mut a, m).unwrap();
            }
            assert_eq!(a, original);
        }
        p.apply(&mut a, &moves[0]).unwrap().commit();
        p.validate(&a).unwrap();
        assert_eq!(a.values()[0], Some(0));
    }
}
#[test]
fn repeated_large_samples_keep_only_selected_metadata_and_ineligible_reset_discards_state() {
    let group = Pillar::new(0..20_000).unwrap();
    let mut sampler = StatefulPillarSampler::new(PillarSelectionPolicy::subsets(2, 4).unwrap());
    for seed in 0..64 {
        assert!(sampler.begin(&group, seed));
        assert!((2..=4).contains(&sampler.pending_rows()));
        let sample = sampler.finish().unwrap();
        assert!((2..=4).contains(&sample.rows().len()));
        assert_eq!(sampler.pending_rows(), 0);
    }
    sampler.begin(&group, 9);
    assert!(!sampler.begin(&Pillar::new([40_000]).unwrap(), 10));
    assert_eq!(sampler.pending_rows(), 0);
    assert_eq!(sampler.finish(), None);
    assert_eq!(sampler.completed_count(), 64);
}
