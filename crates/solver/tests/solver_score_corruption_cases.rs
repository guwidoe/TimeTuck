// Adapted from DefaultSolverTest corrupted easy-score cases and testdomain fixtures.
// Apache-2.0; see compat/provenance.json.
use std::cell::Cell;
use timetuck::*;
fn run(
    initialized: bool,
    asserted: bool,
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
    let mut n = ConstraintNetwork::new(2);
    // Replace source Random.nextInt(1000) with a deterministic changing score in
    // the same range. Every consecutive call differs; no probabilistic failure.
    n.add_constraint("CorruptedEasyScoreCalculator", vec![0, 1], |_| {
        let count = calls.get();
        calls.set(count + 1);
        SimpleScore::new(count % 1000)
    })
    .unwrap();
    let plan = SolverConfig {
        construction: ConstructionConfig {
            assert_scores: asserted,
            ..Default::default()
        },
        local_search: vec![LocalSearchConfig {
            max_steps: 10,
            assert_scores: asserted,
            ..Default::default()
        }],
        ..Default::default()
    };
    let mut events = 0;
    let result = solve_with_events(&p, &n, plan, |e| {
        if matches!(e, SolverEvent::Best(_)) {
            events += 1;
        }
    });
    if result.is_ok() {
        assert!(events > 0);
    }
    result
}
fn assert_corruption(initialized: bool) {
    let error = run(initialized, true).unwrap_err();
    let text = error.to_string();
    assert!(text.contains("Score corruption"), "{text}");
    // Native diagnostic labels replace workingScore/uncorruptedScore. The native
    // scoped calculator can identify the failing constraint, so report that
    // evidence instead of the source's 'analysis could not be generated' fallback.
    assert!(text.contains("working score"), "{text}");
    assert!(text.contains("recomputed score"), "{text}");
    assert!(text.contains("CorruptedEasyScoreCalculator"), "{text}");
}
#[test]
fn solve_corrupted_easy_uninitialized() {
    assert_corruption(false);
}
#[test]
fn solve_corrupted_easy_initialized() {
    assert_corruption(true);
}
#[test]
fn solve_corrupted_easy_unasserted() {
    assert!(run(false, false).is_ok());
}
