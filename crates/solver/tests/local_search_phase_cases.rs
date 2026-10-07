// Adapted from pinned Timefold Community DefaultLocalSearchPhaseTest and its
// fixtures, Apache-2.0. See compat/provenance.json for native API substitutions.
use timetuck::*;
const VALUES: [&str; 3] = ["v1", "v2", "v3"];

fn sequence(tabu: bool) -> SolverSequence<'static> {
    SolverSequence {
        limits: SolverLimits {
            incomplete_handoff: IncompleteHandoffPolicy::Error,
            ..Default::default()
        },
        phases: vec![SolverPhase::LocalSearch(LocalSearchConfig {
            max_evaluations: 10,
            max_steps: 10,
            assert_scores: true,
            acceptance: if tabu {
                Acceptance::VariableTabu {
                    tenure: 1,
                    aspiration: true,
                }
            } else {
                Acceptance::HillClimbing
            },
            ..Default::default()
        })],
    }
}
fn problem(
    values: &[Option<&'static str>],
    pinned: bool,
    optional: bool,
) -> FiniteProblem<&'static str> {
    FiniteProblem::new(
        values
            .iter()
            .enumerate()
            .map(|(i, initial)| {
                let mut v = Variable::new(VALUES.to_vec()).with_pinned(pinned && i == 2);
                if let Some(value) = initial {
                    v = v.with_initial(*value);
                }
                if optional {
                    v = v.allowing_unassigned();
                }
                v
            })
            .collect(),
    )
    .unwrap()
}
fn run(
    p: &FiniteProblem<&'static str>,
    tabu: bool,
    stream: bool,
) -> Result<SolverResult<Assignment<&'static str>, SimpleScore>, SolverError<LocalSearchError>> {
    let mut events = 0;
    let result = if stream {
        solve_stream_with_observer(
            p,
            || {
                let mut b = StreamGraphBuilder::<SimpleScore>::new();
                let source = b.source::<usize, (usize, Option<&'static str>)>();
                Ok((b.build(), source))
            },
            sequence(tabu),
            |_| events += 1,
        )
    } else {
        // The source fixture uses a zero-valued easy score calculator.
        let network = ConstraintNetwork::<_, SimpleScore>::new(p.variables().len());
        solve_with_observer(p, &network, sequence(tabu), |_| events += 1)
    };
    if let Ok(result) = &result {
        // Preserve PlannerTestUtils.solve's nonempty-best-event assertion.
        assert!(events > 0);
        assert!(result.solution().is_some());
        assert!(result.work.evaluations <= 10);
        p.validate(&result.assignment).unwrap();
    }
    result
}
fn initialized(tabu: bool) {
    let p = problem(&[Some("v1"), Some("v2"), Some("v1")], false, false);
    for stream in [false, true] {
        let r = run(&p, tabu, stream).unwrap();
        // Stable native variable slots replace the three code-bearing entities.
        let entities: Vec<_> = ["e1", "e2", "e3"]
            .into_iter()
            .zip(r.assignment.values())
            .collect();
        assert_eq!(entities.len(), 3);
        for (index, code) in ["e1", "e2", "e3"].iter().enumerate() {
            assert_eq!(&entities[index].0, code);
            assert!(entities[index].1.is_some());
        }
    }
}
fn pinned(tabu: bool) {
    let p = problem(&[Some("v1"), Some("v2"), Some("v3")], true, false);
    for stream in [false, true] {
        let r = run(&p, tabu, stream).unwrap();
        let entities: Vec<_> = ["e1", "e2", "e3"]
            .into_iter()
            .zip(r.assignment.values())
            .collect();
        assert_eq!(entities[2].0, "e3");
        assert_eq!(entities[2].1, &Some("v3"));
    }
}
fn empty(tabu: bool) {
    let p = problem(&[], false, false);
    for stream in [false, true] {
        assert!(
            run(&p, tabu, stream)
                .unwrap()
                .assignment
                .values()
                .is_empty()
        );
    }
}
#[test]
fn solve_with_initialized_entities() {
    initialized(false);
}
#[test]
fn solve_with_pinned_entities() {
    pinned(false);
}
#[test]
fn solve_with_pinned_entities_when_unassigned_allowed_and_pinned_to_null() {
    let p = problem(&[None, Some("v2"), None], true, true);
    for stream in [false, true] {
        let r = run(&p, false, stream).unwrap();
        assert_eq!(r.score, Some(SimpleScore::ZERO));
        assert_eq!(r.assignment.values()[2], None);
    }
}
#[test]
fn solve_with_pinned_entities_when_unassigned_not_allowed_and_pinned_to_null() {
    let p = problem(&[None, Some("v2"), None], true, false);
    for stream in [false, true] {
        let error = run(&p, false, stream).unwrap_err();
        let SolverError::Phase {
            phase: 0,
            error: LocalSearchError::PinnedUnassignedVariable { variable },
        } = error
        else {
            panic!("expected a pinned-variable diagnostic: {error:?}")
        };
        // The native slot identifies the source's entity e3 and its value field.
        let identities = [("e1", "value"), ("e2", "value"), ("e3", "value")];
        assert_eq!(identities[variable], ("e3", "value"));
    }
}
#[test]
fn solve_with_empty_entity_list() {
    empty(false);
}
#[test]
fn solve_tabu_search_with_initialized_entities() {
    initialized(true);
}
#[test]
fn solve_tabu_search_with_pinned_entities() {
    pinned(true);
}
#[test]
fn solve_tabu_search_with_empty_entity_list() {
    empty(true);
}
#[test]
fn solve_list_variable() {
    // generateUninitializedSolution(6,2): six registered values, two empty lists.
    let p = ListProblem::new(
        (0..6).map(|i| format!("Generated Value {i}")).collect(),
        vec![ListEntity::new(vec![]); 2],
    )
    .unwrap();
    for stream in [false, true] {
        let mut events = 0;
        let config = SolverConfig {
            local_search: vec![LocalSearchConfig {
                max_steps: 10,
                acceptance: Acceptance::HillClimbing,
                assert_scores: true,
                ..Default::default()
            }],
            ..Default::default()
        };
        let result = if stream {
            solve_list_stream_with_observer(
                &p,
                || {
                    let mut b = StreamGraphBuilder::<SimpleScore>::new();
                    let sources = ListStreamSources::new(&mut b);
                    Ok((b.build(), sources))
                },
                config,
                |_| events += 1,
            )
        } else {
            let n = ListConstraintNetwork::<_, SimpleScore>::new(2, 6);
            solve_list_with_observer(&p, &n, config, |_| events += 1)
        }
        .unwrap();
        assert!(result.solution().is_some());
        assert!(events > 0);
        assert!(p.is_complete(&result.assignment));
    }
}
#[test]
fn fails_fast_with_uninitialized_solution_basic_variable() {
    // generateTestdataSolution("s1",2): two required unset variables and v1,v2.
    let p = FiniteProblem::new(vec![Variable::new(vec!["v1", "v2"]); 2]).unwrap();
    for stream in [false, true] {
        assert!(matches!(
            run(&p, false, stream),
            Err(SolverError::Phase {
                phase: 0,
                error: LocalSearchError::IncompleteInitialAssignment
            })
        ));
    }
}
