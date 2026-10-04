// Adapted from Timefold Community RuinRecreateMoveSelectorTest.testRuiningNoAssignedValues.
// Apache-2.0; method and optional-domain fixtures recorded in compat/provenance.json.
use timetuck::*;
#[test]
fn ruining_no_assigned_values() {
    // Three optional entities, three shared values, initially one distinct value each.
    let p = FiniteProblem::new(
        (0..3)
            .map(|i| {
                Variable::new(vec![0, 1, 2])
                    .allowing_unassigned()
                    .with_initial(i)
            })
            .collect(),
    )
    .unwrap();
    let mut n = ConstraintNetwork::new(3);
    n.add_constraint("No Assigned", vec![0, 1, 2], |v| {
        SimpleScore::new(-(v.iter().filter(|v| v.is_some()).count() as i64))
    })
    .unwrap();
    let c = RuinRecreateConfig {
        minimum_ruined: 3,
        max_attempts: 1,
        assert_scores: true,
        ..Default::default()
    };
    let scoped = ruin_recreate_search_from(&p, &n, p.initial_assignment(), c.clone()).unwrap();
    let stream = ruin_recreate_stream_search_from(
        &p,
        || {
            let mut b = StreamGraphBuilder::new();
            let source = b.source::<usize, (usize, Option<i32>)>();
            b.score("No Assigned", source.port(), |(_, v)| {
                SimpleScore::new(-i64::from(v.is_some()))
            })?;
            Ok((b.build(), source))
        },
        p.initial_assignment(),
        c,
    )
    .unwrap();
    for result in [scoped, stream] {
        let solution = result.best.unwrap();
        assert_eq!(solution.assignment.get(0), Some(&None));
        assert_eq!(solution.assignment.get(1), Some(&None));
        assert_eq!(solution.assignment.get(2), Some(&None));
    }
}

// Adapted 5-value/30-entity fixtures and scores; provenance includes both generators
// and the optional easy calculator. Native plan work replaces Micrometer gauges.
fn generated_problem(optional: bool) -> FiniteProblem<usize> {
    FiniteProblem::new(
        (0..30)
            .map(|i| {
                let mut v = Variable::new((0..5).collect());
                if optional {
                    v = v.allowing_unassigned();
                }
                if !optional || i != 0 {
                    v = v.with_initial(i % 5);
                }
                v
            })
            .collect(),
    )
    .unwrap()
}
fn generated_score(values: &[Option<usize>], optional: bool) -> SimpleScore {
    if !optional {
        return SimpleScore::new(-(values.iter().flatten().count() as i64));
    }
    let nulls = values.iter().filter(|v| v.is_none()).count() as i64;
    let collisions = values
        .iter()
        .enumerate()
        .map(|(i, a)| {
            values
                .iter()
                .enumerate()
                .filter(|(j, b)| i != *j && a.is_some() && a == *b)
                .count() as i64
        })
        .sum::<i64>();
    SimpleScore::new(-nulls - 1000 * collisions)
}
fn generated_solve(
    optional: bool,
    stream: bool,
    mut observe: impl FnMut(SimpleScore),
) -> SolverResult<Assignment<usize>, SimpleScore> {
    let p = generated_problem(optional);
    let c = SolverSequence {
        phases: vec![
            SolverPhase::Construction(ConstructionConfig {
                max_evaluations: u64::MAX,
                assert_scores: true,
                ..Default::default()
            }),
            SolverPhase::RuinRecreate(RuinRecreateConfig {
                seed: 20261223,
                max_attempts: if optional { 100 } else { u64::MAX },
                max_evaluations: u64::MAX,
                assert_scores: true,
                ..Default::default()
            }),
        ],
        limits: SolverLimits {
            max_evaluations: u64::MAX,
            max_unimproved_time: (!optional).then_some(std::time::Duration::from_millis(1000)),
            ..Default::default()
        },
    };
    let result = if stream {
        solve_stream_with_observer(
            &p,
            || {
                let mut b = StreamGraphBuilder::new();
                let source = b.source::<usize, (usize, Option<usize>)>();
                if optional {
                    b.score("Unassigned", source.port(), |(_, v)| {
                        SimpleScore::new(-i64::from(v.is_none()))
                    })?;
                    let assigned = b.filter(source.port(), |(_, v)| v.is_some())?;
                    let pairs = b.join_equal(&assigned, &assigned, |(_, v)| *v, |(_, v)| *v)?;
                    let distinct = b.filter(&pairs, |(a, b)| a.0 != b.0)?;
                    b.score("Ordered collisions", &distinct, |_| SimpleScore::new(-1000))?;
                } else {
                    b.score("Always penalize", source.port(), |(_, v)| {
                        SimpleScore::new(-i64::from(v.is_some()))
                    })?;
                }
                Ok((b.build(), source))
            },
            c,
            |u| observe(u.score),
        )
        .unwrap()
    } else {
        let mut n = ConstraintNetwork::new(30);
        n.add_constraint("Fixture score", (0..30).collect(), move |v| {
            generated_score(&v.iter().map(|v| v.copied()).collect::<Vec<_>>(), optional)
        })
        .unwrap();
        solve_with_observer(&p, &n, c, |u| observe(u.score)).unwrap()
    };
    assert!(result.complete);
    assert_eq!(
        result.score,
        Some(generated_score(result.assignment.values(), optional))
    );
    result
}
#[test]
fn ruining() {
    for stream in [false, true] {
        // Successful solve retains the source assertDoesNotThrow assertion.
        let result = generated_solve(false, stream, |_| {});
        assert_eq!(result.termination, SolverTermination::UnimprovedTimeLimit);
    }
}
#[test]
fn ruining_with_metric() {
    for stream in [false, true] {
        let mut published = Vec::new();
        let result = generated_solve(false, stream, |s| published.push(s));
        assert!(!published.is_empty());
        let score_count = result.work.evaluations;
        let move_count = result.work.repair_attempts;
        assert!(score_count > 0);
        assert!(move_count > 0);
        assert!(score_count > move_count);
    }
}
#[test]
fn ruining_allows_unassigned() {
    for stream in [false, true] {
        let result = generated_solve(true, stream, |_| {});
        // The source's 100-step bound becomes 100 full native reconstruction attempts.
        // No Java accepted-step or PRNG trace equivalence is asserted.
        assert_eq!(result.work.repair_attempts, 100);
        assert_eq!(result.work.complete_reconstructions, 100);
    }
}
