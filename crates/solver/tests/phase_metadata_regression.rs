//! Original phase inspection and runtime-notification integration checks.
use timetuck::*;
fn basic(kind: SolverPhaseKind) -> SolverPhase<'static> {
    match kind {
        SolverPhaseKind::Construction => SolverPhase::Construction(Default::default()),
        SolverPhaseKind::Custom => SolverPhase::Custom(Default::default()),
        SolverPhaseKind::Exact => SolverPhase::Exact(Default::default()),
        SolverPhaseKind::LocalSearch => SolverPhase::LocalSearch(LocalSearchConfig {
            max_steps: 1,
            ..Default::default()
        }),
        SolverPhaseKind::RuinRecreate => SolverPhase::RuinRecreate(RuinRecreateConfig {
            max_attempts: 1,
            ..Default::default()
        }),
        SolverPhaseKind::Initial => unreachable!(),
    }
}
fn mixed(kind: SolverPhaseKind) -> MixedSolverPhase<'static> {
    match kind {
        SolverPhaseKind::Construction => MixedSolverPhase::Construction(Default::default()),
        SolverPhaseKind::Custom => MixedSolverPhase::Custom(Default::default()),
        SolverPhaseKind::Exact => MixedSolverPhase::Exact(Default::default()),
        SolverPhaseKind::LocalSearch => MixedSolverPhase::LocalSearch(Default::default()),
        SolverPhaseKind::RuinRecreate => MixedSolverPhase::RuinRecreate(Default::default()),
        SolverPhaseKind::Initial => unreachable!(),
    }
}
#[test]
fn all_short_basic_and_mixed_sequences_have_one_shared_boundary_or_none() {
    use SolverPhaseKind::*;
    let kinds = [Construction, Custom, Exact, LocalSearch, RuinRecreate];
    assert!(!Initial.can_initialize());
    for length in 0..=4u32 {
        for mut code in 0..5usize.pow(length) {
            let order = (0..length)
                .map(|_| {
                    let k = kinds[code % 5];
                    code /= 5;
                    k
                })
                .collect::<Vec<_>>();
            let a = SolverSequence {
                phases: order.iter().copied().map(basic).collect(),
                ..Default::default()
            };
            let b = MixedSolverSequence {
                phases: order.iter().copied().map(mixed).collect(),
                ..Default::default()
            };
            let expected = order
                .iter()
                .take_while(|k| !matches!(k, LocalSearch | RuinRecreate))
                .count();
            let last = (expected > 0 && expected < order.len()).then(|| expected - 1);
            let metadata = a.phase_metadata().collect::<Vec<_>>();
            assert_eq!(metadata, b.phase_metadata().collect::<Vec<_>>());
            assert_eq!(metadata.len(), order.len());
            for (index, phase) in metadata.iter().enumerate() {
                assert_eq!((phase.index, phase.kind), (index, order[index]));
                assert_eq!(phase.is_last_initializing, last == Some(index));
                assert_eq!(
                    phase.kind.can_initialize(),
                    matches!(phase.kind, Construction | Custom | Exact)
                );
            }
            assert_eq!(a.initialization_phase(), last.map(|i| metadata[i]));
            assert_eq!(a.initialization_phase(), b.initialization_phase());
        }
    }
    let mut sequence = SolverSequence::<SimpleScore>::default();
    let saved = sequence.initialization_phase().unwrap();
    sequence.phases.reverse();
    assert_eq!(sequence.initialization_phase(), None);
    assert_eq!(saved.kind, Construction);
}
#[test]
fn reported_runtime_initialization_matches_inspection_including_custom_and_exact() {
    use SolverPhaseKind::*;
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap();
    let n = ConstraintNetwork::<_, SimpleScore>::new(1);
    for order in [
        vec![Construction, LocalSearch],
        vec![Construction, Construction, LocalSearch],
        vec![Construction, Custom, LocalSearch],
        vec![Custom, Construction, LocalSearch],
        vec![Exact, LocalSearch],
        vec![Construction, RuinRecreate],
        vec![Custom, Custom],
        vec![LocalSearch, Construction, LocalSearch],
    ] {
        let sequence = SolverSequence {
            phases: order.iter().copied().map(basic).collect(),
            ..Default::default()
        };
        let expected = sequence.initialization_phase();
        let mut observed = vec![];
        solve_with_commands_and_events(
            &p,
            &n,
            sequence,
            |_, _| Ok(()),
            |event| {
                if let SolverEvent::FirstInitialized { initialization, .. } = event {
                    observed.push((initialization.phase, initialization.kind));
                }
            },
        )
        .unwrap();
        assert_eq!(
            observed,
            expected
                .map(|p| (p.index, p.kind))
                .into_iter()
                .collect::<Vec<_>>(),
            "{order:?}"
        );
    }
    // Configuration can identify a boundary even when an all-pinned model skips it.
    let p = FiniteProblem::new(vec![
        Variable::new(vec![0, 1]).with_initial(0).with_pinned(true),
    ])
    .unwrap();
    let sequence = SolverSequence::default();
    assert!(sequence.initialization_phase().is_some());
    solve_with_events(&p, &n, sequence, |event| {
        assert!(!matches!(event, SolverEvent::FirstInitialized { .. }))
    })
    .unwrap();
}
