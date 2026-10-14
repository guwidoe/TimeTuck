//! Original bounded workload qualification; no upstream method credit.
#[path = "../examples/mixed_benchmark_support/mod.rs"]
mod support;
use support::{Case, run};
use timetuck::*;
const SMALL: &str =
    "mixed-small\t1:1,2:0,18:0,19:1\t0:0,20:0\t2,2\t0,0\t2,0;3,1\t00\t0,0\t16\t10000";
#[test]
fn both_real_mixed_backends_improve_the_complete_coupled_warm_start() {
    let case = Case::parse(SMALL).unwrap();
    assert_eq!(case.id, "mixed-small");
    assert_eq!(
        case.score(&case.problem.initial_assignment()),
        HardSoftScore::new(0, -78)
    );
    let exact = exhaustive_mixed_search(&case.problem, |a| case.score(a), 10000);
    assert!(exact.is_optimal());
    let optimum = exact.best.unwrap().score;
    assert_eq!(optimum, HardSoftScore::new(0, -12));
    for stream in [false, true] {
        let result = run(&case, stream).unwrap();
        let best = result.best.unwrap();
        case.validate(&best.assignment, best.score).unwrap();
        assert!(best.score > HardSoftScore::new(0, -78));
        assert!(best.score <= optimum);
        assert!(result.evaluations <= case.evaluations && result.completed_steps <= case.steps);
        assert!(result.constraint_evaluations > 0);
    }
}
#[test]
fn whole_owner_and_prefix_pins_are_retained_by_actual_search() {
    let case = Case::parse(
        "mixed-pins\t1:1,2:0,18:0,19:1\t0:0,20:0\t1,3\t0,0\t0;3,1,2\t10\t0,1\t8\t10000",
    )
    .unwrap();
    for stream in [false, true] {
        let result = run(&case, stream).unwrap();
        let best = result.best.unwrap();
        case.validate(&best.assignment, best.score).unwrap();
        assert_eq!(best.assignment.basic().values()[0], Some(0));
        assert_eq!(best.assignment.lists().lists()[0], vec![0]);
        assert_eq!(best.assignment.lists().lists()[1][0], 3);
    }
}
#[test]
fn malformed_or_incomplete_protocol_does_not_become_a_timed_workload() {
    for text in [
        SMALL.replace("2,0;3,1", "2,0;3,0"),
        SMALL.replace("2,0;3,1", "2,0;3"),
        SMALL.replace("\t10000", "\t0"),
        SMALL.replace("\t00\t0,0", "\t10\t1,0"),
    ] {
        assert!(Case::parse(&text).is_err());
    }
}
