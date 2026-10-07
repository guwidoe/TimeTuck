// Adapted from DefaultSolverTest and its nested incremental calculator fixtures.
// Apache-2.0; see compat/provenance.json.
use std::cell::Cell;
use timetuck::*;
fn run(
    initialized: bool,
    corrupted: bool,
) -> Result<SolverResult<Assignment<&'static str>, SimpleScore>, SolverError<LocalSearchError>> {
    let variables = (0..2)
        .map(|i| {
            let v = Variable::new(vec!["v1", "v2"]);
            if initialized {
                v.with_initial(if i == 0 { "v1" } else { "v2" })
            } else {
                v
            }
        })
        .collect();
    let p = FiniteProblem::new(variables).unwrap();
    let calls = Cell::new(0);
    let config = SolverConfig {
        construction: ConstructionConfig {
            assert_scores: true,
            ..Default::default()
        },
        local_search: vec![LocalSearchConfig {
            assert_scores: true,
            max_steps: 10,
            ..Default::default()
        }],
        ..Default::default()
    };
    let mut events = 0;
    let result = solve_stream_with_events(
        &p,
        || {
            let mut b = StreamGraphBuilder::<SimpleScore>::new();
            let source = b.source::<usize, (usize, Option<&'static str>)>();
            if corrupted {
                // Source returns Random.nextInt(1000), inconsistent with registered
                // evidence. Deterministic changes retain corruption without flakiness.
                b.score("CorruptedIncrementalScoreCalculator", source.port(), |_| {
                    let count = calls.get();
                    calls.set(count + 1);
                    SimpleScore::new(count % 1000)
                })?;
            } else {
                b.score("initialized entity count", source.port(), |row| {
                    SimpleScore::new(i64::from(row.1.is_some()))
                })?;
            }
            Ok((b.build(), source))
        },
        config,
        |e| {
            if matches!(e, SolverEvent::Best(_)) {
                events += 1;
            }
        },
    );
    if result.is_ok() {
        assert!(events > 0);
    }
    result
}
#[test]
fn solve_incremental_initialized() {
    let result = run(true, false).unwrap();
    assert_eq!(result.score, Some(SimpleScore::new(2)));
    assert!(result.assignment.values().iter().all(Option::is_some));
}
fn corrupted(initialized: bool) {
    let error = run(initialized, true).unwrap_err();
    let text = error.to_string();
    for expected in [
        "Score corruption",
        "working score",
        "recomputed score",
        "CorruptedIncrementalScoreCalculator",
        "retained matches",
    ] {
        assert!(text.contains(expected), "missing {expected} in {text}");
    }
}
#[test]
fn solve_corrupted_incremental_uninitialized() {
    corrupted(false)
}
#[test]
fn solve_corrupted_incremental_initialized() {
    corrupted(true)
}
