// Adapted from pinned DefaultSolverTest and its test helpers/fixtures.
// Apache-2.0; see compat/provenance.json for native event-delivery substitutions.
use timetuck::*;
type PhaseDeliveryResult = (
    SolverResult<Assignment<i32>, SimpleScore>,
    Vec<BufferedSolverUpdate<Assignment<i32>, SimpleScore>>,
);
fn run(mode: usize, count: usize, custom: bool, partial: bool) -> PhaseDeliveryResult {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]); count]).unwrap();
    let config = SolverSequence {
        phases: if custom {
            vec![SolverPhase::Custom(CustomPhaseConfig::default())]
        } else if partial {
            vec![SolverPhase::Construction(ConstructionConfig {
                max_steps: Some(2),
                ..Default::default()
            })]
        } else {
            vec![
                SolverPhase::Construction(ConstructionConfig::default()),
                SolverPhase::LocalSearch(LocalSearchConfig {
                    max_steps: 10,
                    ..Default::default()
                }),
            ]
        },
        ..Default::default()
    };
    let mut buffer =
        PhaseBestBuffer::new(SimpleScore::ZERO.with_structural(-(count as i64)), false);
    let mut delivered = vec![];
    let mut observer = |u: SolverUpdate<'_, Assignment<i32>, SimpleScore>| {
        if let Some(update) = buffer.observe(u) {
            delivered.push(update);
        }
    };
    let result = if mode == 0 {
        let n = ConstraintNetwork::<_, SimpleScore>::new(count);
        if custom {
            solve_with_commands_and_events(
                &p,
                &n,
                config,
                |_, _| Ok(()),
                |event| {
                    if let SolverEvent::Best(u) = event {
                        observer(u);
                    }
                },
            )
            .unwrap()
        } else {
            solve_with_observer(&p, &n, config, &mut observer).unwrap()
        }
    } else {
        let graph = || {
            let mut b = StreamGraphBuilder::new();
            let source = b.source::<usize, (usize, Option<i32>)>();
            b.score("zero", source.port(), |_| SimpleScore::ZERO)?;
            Ok((b.build(), source))
        };
        if custom {
            solve_stream_with_commands_and_events(
                &p,
                graph,
                config,
                |_, _| Ok(()),
                |event| {
                    if let SolverEvent::Best(u) = event {
                        observer(u);
                    }
                },
            )
            .unwrap()
        } else {
            solve_stream_with_observer(&p, graph, config, &mut observer).unwrap()
        }
    };
    if let Some(update) = buffer.flush() {
        delivered.push(update);
    }
    (result, delivered)
}
#[test]
fn solve() {
    for mode in 0..2 {
        let (result, events) = run(mode, 2, false, false);
        assert!(result.assignment.values().iter().all(Option::is_some));
        assert_eq!(events.len(), 1);
        assert_eq!(
            (
                events[0].score,
                events[0].complete,
                events[0].kind,
                events[0].phase
            ),
            (SimpleScore::ZERO, true, SolverPhaseKind::Construction, 0)
        );
    }
}
#[test]
fn solve_stops_when_uninitialized() {
    for mode in 0..2 {
        let (result, events) = run(mode, 5, true, false);
        assert!(result.score.is_some());
        assert!(events.is_empty());
        assert!(result.assignment.values().iter().all(Option::is_none));
    }
}
#[test]
fn solve_stops_when_partially_initialized() {
    for mode in 0..2 {
        let (result, events) = run(mode, 5, false, true);
        assert!(result.assignment.values().iter().any(Option::is_none));
        assert!(!events.is_empty());
        assert_eq!(result.assignment.values().iter().flatten().count(), 2);
        assert_eq!(events.last().unwrap().assignment, result.assignment);
    }
}
