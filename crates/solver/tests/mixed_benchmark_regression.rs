//! Original bounded workload qualification; no upstream method credit.
#[path = "../examples/mixed_benchmark_support/mod.rs"]
mod support;
use support::{Case, cold_problem, run, run_cold};
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

#[test]
fn cold_mixed_solve_constructs_both_variable_kinds_and_reaches_the_small_optimum() {
    let case = Case::parse(&format!("{SMALL}\t0:-12")).unwrap();
    let cold = cold_problem(&case).unwrap();
    assert_eq!(cold.initial_assignment().basic().values(), &[None, None]);
    assert!(
        cold.initial_assignment()
            .lists()
            .lists()
            .iter()
            .all(Vec::is_empty)
    );
    assert!(!cold.is_complete(&cold.initial_assignment()));
    let mut previous = None;
    for stream in [false, true] {
        let result = run_cold(&case, stream).unwrap();
        assert!(result.complete);
        assert_eq!(result.score, Some(HardSoftScore::new(0, -12)));
        case.validate(&result.assignment, result.score.unwrap())
            .unwrap();
        let mut stronger_floor = case.clone();
        stronger_floor.quality_floor = Some(HardSoftScore::new(0, -11));
        assert!(
            stronger_floor
                .validate(&result.assignment, result.score.unwrap())
                .is_err()
        );
        assert_eq!(result.phases.len(), 2);
        assert_eq!(result.work.construction_steps, 6);
        assert!(result.work.completed_steps <= case.steps);
        assert!(result.work.evaluations <= case.evaluations);
        assert_eq!(
            result.work.evaluations,
            result
                .phases
                .iter()
                .map(|p| p.work.evaluations)
                .sum::<u64>()
        );
        assert!(result.phases.iter().all(|p| p.work.evaluations > 0));
        let logical = (
            result.assignment,
            result.work.evaluations,
            result.work.completed_steps,
        );
        if let Some(previous) = &previous {
            assert_eq!(&logical, previous);
        }
        previous = Some(logical);
    }
}

fn clustered_case() -> Case {
    let homes = [(0, 0), (30, 0), (0, 30), (30, 30)];
    let offsets = [(1, 0), (2, 1), (3, 0), (1, 2), (2, 3), (3, 2)];
    let points = homes
        .into_iter()
        .flat_map(|(x, y)| {
            offsets
                .into_iter()
                .map(move |(dx, dy)| format!("{}:{}", x + dx, y + dy))
        })
        .collect::<Vec<_>>()
        .join(",");
    let remaining: Vec<_> = (6..24).filter(|v| ![6, 12, 18].contains(v)).collect();
    let routes = std::iter::once("0,1,2,3,4,5".into())
        .chain((1..4).map(|owner| {
            std::iter::once(6 * owner)
                .chain(remaining.iter().skip(owner - 1).step_by(3).copied())
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(",")
        }))
        .collect::<Vec<String>>()
        .join(";");
    Case::parse(&format!("mixed-clustered-pins-cold\t{points}\t0:0,30:0,0:30,30:30\t6,6,6,6\t0,3,1,2\t{routes}\t1000\t0,1,1,1\t24\t40000")).unwrap()
}

#[test]
fn cold_clustered_solve_keeps_pins_and_reaches_an_independent_feasible_certificate() {
    let case = clustered_case();
    let cold = cold_problem(&case).unwrap();
    let initial = cold.initial_assignment();
    assert_eq!(initial.basic().values(), &[Some(0), None, None, None]);
    assert_eq!(
        initial.lists().lists(),
        &[vec![0, 1, 2, 3, 4, 5], vec![6], vec![12], vec![18]]
    );
    let mut certificate = case.problem.initial_assignment();
    for owner in 1..4 {
        // Check the feasible certificate with the full scorer, independently of search.
        case.problem
            .apply(
                &mut certificate,
                &MixedMove {
                    basic: Move::Change {
                        variable: owner,
                        value: Some(owner),
                    },
                    lists: ListMove::Composite(vec![]),
                },
            )
            .unwrap()
            .commit();
    }
    // A fresh complete model avoids editing through the pinned-prefix move interface.
    let certificate = MixedAssignment::new(
        certificate.basic().clone(),
        ListProblem::new(
            (0..24).collect(),
            (0..4)
                .map(|i| ListEntity::new((6 * i..6 * i + 6).collect()))
                .collect(),
        )
        .unwrap()
        .initial_assignment(),
    );
    case.validate(&certificate, HardSoftScore::new(0, -72))
        .unwrap();
    for stream in [false, true] {
        let result = run_cold(&case, stream).unwrap();
        assert!(result.complete);
        case.validate(&result.assignment, result.score.unwrap())
            .unwrap();
        assert!(result.score.unwrap() >= HardSoftScore::new(0, -72));
        assert_eq!(result.work.construction_steps, 18);
        assert!(result.work.evaluations <= case.evaluations);
        assert!(result.work.completed_steps <= case.steps);
    }
}

#[test]
fn cold_construction_obeys_the_shared_budget_and_returns_actual_partial_work() {
    let mut case = Case::parse(SMALL).unwrap();
    case.evaluations = 1;
    for stream in [false, true] {
        let result = run_cold(&case, stream).unwrap();
        assert!(!result.complete);
        assert_eq!(result.work.evaluations, 1);
        assert_eq!(result.work.construction_steps, 0);
        assert_eq!(result.work.completed_steps, 0);
        assert_eq!(
            result.score,
            Some(HardSoftScore::new(-6, 0).with_structural(-6))
        );
        assert_eq!(
            result.assignment,
            cold_problem(&case).unwrap().initial_assignment()
        );
    }
}

#[test]
fn component_refinement_keeps_quality_and_shared_caps_across_phase_handoffs() {
    for stream in [false, true] {
        let mut case = Case::parse(SMALL).unwrap();
        let result = support::run_component_refinement(&case, stream, false).unwrap();
        let score = result.score.unwrap();
        case.validate(&result.assignment, score).unwrap();
        assert_eq!(score, HardSoftScore::new(0, -12));
        assert_eq!(result.phases.len(), 2);
        assert_eq!(
            result.phases[0].termination,
            PhaseTermination::LocalSearch(LocalSearchTermination::NoAcceptedMove)
        );
        assert!(
            result.work.completed_steps <= case.steps
                && result.work.evaluations <= case.evaluations
        );
        case.steps = 1;
        let capped = support::run_component_refinement(&case, stream, false).unwrap();
        assert!(capped.complete);
        assert_eq!(capped.work.completed_steps, 1);
        assert_eq!(capped.phases.len(), 1);
        assert_eq!(capped.termination, SolverTermination::Condition);
        case.steps = 16;
        case.evaluations = 1;
        let capped = support::run_component_refinement(&case, stream, false).unwrap();
        assert!(capped.work.evaluations <= 1 && capped.phases.len() <= 1);
        assert_eq!(capped.work.completed_steps, 0);
        let cold = support::run_component_refinement(&case, stream, true).unwrap();
        assert!(!cold.complete);
        assert!(cold.work.evaluations <= 1 && cold.phases.len() <= 1);
        assert_eq!(cold.work.completed_steps, 0);
    }
}

#[test]
fn cold_customer_penalties_match_independent_math_at_partial_and_final_caps() {
    fn independent(case: &Case, assignment: &MixedAssignment<usize, usize>) -> HardSoftScore {
        let routes = assignment.lists().lists();
        let missing_depots = assignment
            .basic()
            .values()
            .iter()
            .filter(|v| v.is_none())
            .count();
        let assigned_customers: usize = routes.iter().map(Vec::len).sum();
        let missing = missing_depots + case.points.len() - assigned_customers;
        let mut hard = -(missing as i64);
        let mut soft = 0;
        for (owner, route) in routes.iter().enumerate() {
            hard -= route.len().saturating_sub(case.capacities[owner]) as i64;
            if let Some(depot) = assignment.basic().values()[owner] {
                // A closed tour has one edge between every adjacent pair in
                // [home, customer..., home]; absent depots have no tour edges.
                let home = case.depots[depot];
                let mut points = vec![home];
                points.extend(route.iter().map(|&id| case.points[id]));
                points.push(home);
                soft -= points
                    .windows(2)
                    .map(|pair| {
                        pair[0]
                            .iter()
                            .zip(pair[1])
                            .map(|(a, b)| (a - b).abs())
                            .sum::<i64>()
                    })
                    .sum::<i64>();
            }
        }
        HardSoftScore::new(hard, soft).with_structural(-(missing as i64))
    }
    let small = Case::parse(&format!("{SMALL}\t0:-12")).unwrap();
    let mut large = clustered_case();
    large.quality_floor = Some(HardSoftScore::new(0, -72));
    for original in [small, large] {
        let full_budget = original.evaluations;
        for cap in [1, 2, 7, 32, 256, 2048, full_budget] {
            let mut case = original.clone();
            case.evaluations = cap;
            let cold = cold_problem(&case).unwrap();
            for stream in [false, true] {
                let result = support::run_component_refinement(&case, stream, true).unwrap();
                cold.validate(&result.assignment).unwrap();
                assert_eq!(
                    result.score,
                    Some(independent(&case, &result.assignment)),
                    "{} cap={cap} stream={stream}",
                    case.id
                );
                assert_eq!(result.complete, cold.is_complete(&result.assignment));
                assert!(result.work.evaluations <= cap);
                assert!(result.work.completed_steps <= original.steps);
                assert_eq!(
                    result.work.evaluations,
                    result
                        .phases
                        .iter()
                        .map(|p| p.work.evaluations)
                        .sum::<u64>()
                );
                assert_eq!(
                    result.work.completed_steps,
                    result
                        .phases
                        .iter()
                        .map(|p| p.work.completed_steps)
                        .sum::<u64>()
                );
                assert_eq!(
                    result.work.construction_steps,
                    result
                        .phases
                        .iter()
                        .map(|p| p.work.construction_steps)
                        .sum::<u64>()
                );
                if cap == full_budget {
                    assert!(result.complete);
                    case.validate(&result.assignment, result.score.unwrap())
                        .unwrap();
                    assert!(result.score.unwrap() >= original.quality_floor.unwrap());
                }
            }
        }
    }
}
