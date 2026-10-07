// Adapted from DefaultConstructionHeuristicPhaseTest, PlannerTestUtils and testdomain fixtures.
// Apache-2.0; see compat/provenance.json.
use timetuck::*;
type Value = &'static str;
fn phase<S: AdditiveScore>(
    optional: OptionalConstruction,
    steps: Option<u64>,
) -> SolverSequence<'static, S> {
    SolverSequence {
        phases: vec![SolverPhase::Construction(ConstructionConfig {
            optional,
            max_steps: steps,
            assert_scores: true,
            ..Default::default()
        })],
        ..Default::default()
    }
}
fn basic<S: AdditiveScore + 'static>(
    p: &FiniteProblem<Value>,
    stream: bool,
    optional: OptionalConstruction,
    steps: Option<u64>,
    score: fn(&[Option<Value>]) -> S,
) -> SolverResult<Assignment<Value>, S> {
    let mut events = 0;
    let mut observe = |e: SolverEvent<'_, Assignment<Value>, S>| {
        if let SolverEvent::Best(_) = e {
            events += 1;
        }
    };
    let result = if stream {
        solve_stream_with_events(
            p,
            || {
                let mut b = StreamGraphBuilder::new();
                let source = b.source::<usize, (usize, Option<Value>)>();
                let all = b.collect_list(source.port(), |(_, v)| *v)?;
                b.score("objective", &all, move |v| {
                    score(&v.iter().copied().collect::<Vec<_>>())
                })?;
                Ok((b.build(), source))
            },
            phase(optional, steps),
            &mut observe,
        )
        .unwrap()
    } else {
        let mut n = ConstraintNetwork::new(p.variables().len());
        n.add_constraint("objective", (0..p.variables().len()).collect(), move |v| {
            score(&v.iter().map(|v| v.copied()).collect::<Vec<_>>())
        })
        .unwrap();
        solve_with_events(p, &n, phase(optional, steps), &mut observe).unwrap()
    };
    // PlannerTestUtils.solve asserts that at least one best-solution event exists.
    assert!(events > 0);
    result
}
fn lists<S: AdditiveScore + 'static>(
    p: &ListProblem<Value>,
    stream: bool,
    optional: OptionalConstruction,
    steps: Option<u64>,
    score: fn(usize) -> S,
) -> SolverResult<ListAssignment<Value>, S> {
    let mut events = 0;
    let mut observe = |e: SolverEvent<'_, ListAssignment<Value>, S>| {
        if let SolverEvent::Best(_) = e {
            events += 1;
        }
    };
    let result = if stream {
        solve_list_stream_with_events(
            p,
            || {
                let mut b = StreamGraphBuilder::new();
                let sources = ListStreamSources::new(&mut b);
                b.score("objective", sources.entities.port(), move |(_, v)| {
                    score(v.len())
                })?;
                Ok((b.build(), sources))
            },
            phase(optional, steps),
            &mut observe,
        )
        .unwrap()
    } else {
        let mut n = ListConstraintNetwork::new(p.entities().len(), p.values().len());
        for e in 0..p.entities().len() {
            n.add_constraint(
                format!("size{e}"),
                vec![ListScope::Entity(e)],
                move |rows| {
                    let ListFact::Entity(v) = rows[0] else {
                        panic!()
                    };
                    score(v.len())
                },
            )
            .unwrap();
        }
        solve_list_with_events(p, &n, phase(optional, steps), &mut observe).unwrap()
    };
    assert!(events > 0);
    result
}
fn variable() -> Variable<Value> {
    Variable::new(vec!["v1", "v2", "v3"])
}
fn zero(_: &[Option<Value>]) -> SimpleScore {
    SimpleScore::ZERO
}
#[test]
fn solve_with_initialized_entities() {
    let p = FiniteProblem::new(vec![
        variable(),
        variable().with_initial("v2"),
        variable().with_initial("v1"),
    ])
    .unwrap();
    for stream in [false, true] {
        let r = basic(&p, stream, OptionalConstruction::Preserve, None, zero);
        assert!(r.assignment.values()[0].is_some());
        assert_eq!(r.assignment.values()[1..], [Some("v2"), Some("v1")]);
        assert!(r.complete);
        assert_eq!(
            p.initial_assignment().values(),
            &[None, Some("v2"), Some("v1")]
        );
    }
}
#[test]
fn solve_with_initialized_solution() {
    let p = FiniteProblem::new(vec![
        variable().with_initial("v1"),
        variable().with_initial("v2"),
        variable().with_initial("v3"),
    ])
    .unwrap();
    let input = p.initial_assignment();
    for stream in [false, true] {
        let r = basic(&p, stream, OptionalConstruction::Preserve, None, zero);
        // Native assignments own independent storage even when no change is needed.
        assert_ne!(input.values().as_ptr(), r.assignment.values().as_ptr());
        assert_eq!(r.assignment, input);
        assert_eq!(p.initial_assignment(), input);
    }
}
#[test]
fn solve_with_pinned_entities() {
    let p = FiniteProblem::new(vec![
        variable(),
        variable().with_initial("v2"),
        variable().with_initial("v3").with_pinned(true),
    ])
    .unwrap();
    for stream in [false, true] {
        let r = basic(&p, stream, OptionalConstruction::Preserve, None, zero);
        assert!(r.assignment.values()[0].is_some());
        assert_eq!(r.assignment.values()[1..], [Some("v2"), Some("v3")]);
        assert_eq!(r.score, Some(SimpleScore::ZERO));
    }
}
#[test]
fn solve_with_pinned_entities_when_unassigned_allowed_and_pinned_to_null() {
    let p = FiniteProblem::new(vec![
        variable().allowing_unassigned(),
        variable().allowing_unassigned().with_initial("v2"),
        variable().allowing_unassigned().with_pinned(true),
    ])
    .unwrap();
    for stream in [false, true] {
        let r = basic(&p, stream, OptionalConstruction::Preserve, None, zero);
        assert_eq!(r.score, Some(SimpleScore::ZERO));
        assert_eq!(r.assignment.values(), &[None, Some("v2"), None]);
    }
}
#[test]
fn solve_with_empty_entity_list() {
    // The source also has three unused problem facts; native domains belong to entities.
    let p = FiniteProblem::new(vec![]).unwrap();
    for stream in [false, true] {
        let r = basic(&p, stream, OptionalConstruction::Preserve, None, zero);
        assert!(r.assignment.values().is_empty());
    }
}
#[test]
fn solve_with_entity_value_range_basic_variable() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec!["v1", "v2"]).allowing_unassigned(),
        Variable::new(vec!["v3"]).allowing_unassigned(),
    ])
    .unwrap();
    for stream in [false, true] {
        let r = basic(
            &p,
            stream,
            OptionalConstruction::EvaluateUnassigned,
            None,
            |values| {
                SimpleScore::new(
                    values
                        .iter()
                        .map(|v| if v.is_some() { 1 } else { -1 })
                        .sum(),
                )
            },
        );
        assert!(r.assignment.values()[0].is_some());
        assert_eq!(r.assignment.values()[1], Some("v3"));
    }
}
#[test]
fn solve_with_entity_value_range_list_variable() {
    let p = ListProblem::from_entity_ranges(vec![
        ListEntity::new(vec![]).with_value_range(vec!["v1", "v2"]),
        ListEntity::new(vec![]).with_value_range(vec!["v2", "v3"]),
    ])
    .unwrap();
    for stream in [false, true] {
        let r = lists(&p, stream, OptionalConstruction::Preserve, None, |len| {
            SimpleScore::new(if len >= 2 { 2 } else { 1 })
        });
        let mut first = r.assignment.list(0).unwrap().to_vec();
        first.sort();
        assert_eq!(first, vec!["v1", "v2"]);
        assert_eq!(r.assignment.list(1).unwrap(), &["v3"]);
    }
}
#[test]
fn penalize_basic_variable() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec!["Generated Value 0"]).allowing_unassigned(),
    ])
    .unwrap();
    for stream in [false, true] {
        let r = basic(
            &p,
            stream,
            OptionalConstruction::EvaluateUnassigned,
            Some(3),
            |values| HardSoftScore::new(-(values.iter().flatten().count() as i64), 0),
        );
        assert_eq!(r.assignment.values(), &[None]);
        assert_eq!(r.score, Some(HardSoftScore::ZERO));
    }
}
#[test]
fn penalize_list_variable() {
    let p = ListProblem::new(vec!["Generated Value 0"], vec![ListEntity::new(vec![])])
        .unwrap()
        .allowing_unassigned();
    for stream in [false, true] {
        let r = lists(
            &p,
            stream,
            OptionalConstruction::EvaluateUnassigned,
            Some(3),
            |len| HardSoftScore::new(if len > 0 { -1 } else { 0 }, 0),
        );
        assert!(r.assignment.list(0).unwrap().is_empty());
        assert_eq!(r.score, Some(HardSoftScore::ZERO));
    }
}
