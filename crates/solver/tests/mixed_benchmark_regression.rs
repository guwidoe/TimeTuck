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

// Original graph-policy regressions; no upstream-method credit.
#[cfg(test)]
mod owner_links_qualification {
    use super::*;
    use std::cell::{Cell, RefCell};
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use support::{
        OwnerLinkId, OwnerPair, OwnerRouteConstraint, StreamGraphPolicy, declare_owner_routes,
    };

    #[derive(Clone)]
    enum Held {
        Ordinary(SnapshotMap<(usize, usize), (OwnerPair, HardSoftScore)>),
        Links(SnapshotMap<OwnerLinkId, (OwnerPair, HardSoftScore)>),
        Fused(SnapshotMap<OwnerLinkId, (OwnerPair, HardSoftScore)>),
    }
    fn hold(g: &StreamGraph<'_, HardSoftScore>, h: &OwnerRouteConstraint) -> Held {
        match h {
            OwnerRouteConstraint::Ordinary(h) => {
                Held::Ordinary(g.constraint_matches(h).unwrap().clone())
            }
            OwnerRouteConstraint::OwnerLinks(h) => {
                Held::Links(g.constraint_matches(h).unwrap().clone())
            }
            OwnerRouteConstraint::FusedOwnerLinks(h) => {
                Held::Fused(g.constraint_matches(h).unwrap().clone())
            }
        }
    }
    fn assert_matches(snapshot: &Held, case: &Case, a: &MixedAssignment<usize, usize>, cold: bool) {
        let expected: Vec<_> = a
            .lists()
            .lists()
            .iter()
            .enumerate()
            .map(|(owner, route)| {
                let pair = ((owner, a.basic().values()[owner]), (owner, route.clone()));
                let score = contribution(case, &pair, cold);
                (owner, (pair, score))
            })
            .collect();
        // Assert ACTUAL namespaces and complete ordered entries, not just a normalization.
        match snapshot {
            Held::Ordinary(rows) => assert_eq!(
                rows.iter()
                    .map(|(id, v)| (*id, v.clone()))
                    .collect::<Vec<_>>(),
                expected
                    .iter()
                    .map(|(o, v)| ((*o, *o), v.clone()))
                    .collect::<Vec<_>>()
            ),
            Held::Links(rows) | Held::Fused(rows) => assert_eq!(
                rows.iter()
                    .map(|(id, v)| (*id, v.clone()))
                    .collect::<Vec<_>>(),
                expected
                    .iter()
                    .map(|(o, v)| (((*o, *o), *o), v.clone()))
                    .collect::<Vec<_>>()
            ),
        }
    }
    fn contribution(case: &Case, pair: &OwnerPair, cold: bool) -> HardSoftScore {
        let ((owner, depot), (_, route)) = pair;
        let base = depot.map_or_else(
            || {
                HardSoftScore::new(
                    -1 - route.len().saturating_sub(case.capacities[*owner]) as i64,
                    0,
                )
            },
            |d| case.route_score(d, route, *owner),
        );
        if cold {
            HardSoftScore::new(
                base.hard() + route.len() as i64
                    - if *owner == 0 {
                        case.points.len() as i64
                    } else {
                        0
                    },
                base.soft(),
            )
        } else {
            base
        }
    }
    // Independently compute the complete/partial objective from coordinates and ownership.
    fn independent(case: &Case, a: &MixedAssignment<usize, usize>) -> HardSoftScore {
        let missing = case.points.len() - a.lists().lists().iter().map(Vec::len).sum::<usize>()
            + a.basic().values().iter().filter(|v| v.is_none()).count();
        let mut hard = -(missing as i64);
        let mut distance = 0;
        for (owner, route) in a.lists().lists().iter().enumerate() {
            hard -= route.len().saturating_sub(case.capacities[owner]) as i64;
            if let Some(d) = a.basic().values()[owner] {
                let home = case.depots[d];
                let mut previous = home;
                for &v in route {
                    let point = case.points[v];
                    distance += (point[0] - previous[0]).abs() + (point[1] - previous[1]).abs();
                    previous = point;
                }
                distance += (home[0] - previous[0]).abs() + (home[1] - previous[1]).abs();
            }
        }
        HardSoftScore::new(hard, -distance).with_structural(-(missing as i64))
    }
    fn assert_rows(
        g: &StreamGraph<'_, HardSoftScore>,
        s: &MixedStreamSources<usize, usize>,
        p: &MixedProblem<usize, usize>,
        a: &MixedAssignment<usize, usize>,
    ) {
        let basics = g.source_rows(&s.basic).unwrap();
        assert_eq!(basics.len(), a.basic().values().len());
        for (i, v) in a.basic().values().iter().enumerate() {
            assert_eq!(basics[&i], (i, *v));
        }
        let entities = g.source_rows(&s.lists.entities).unwrap();
        assert_eq!(entities.len(), a.lists().lists().len());
        for (i, r) in a.lists().lists().iter().enumerate() {
            assert_eq!(entities[&i], (i, r.clone()));
        }
        let values = g.source_rows(&s.lists.values).unwrap();
        assert_eq!(values.len(), p.lists().values().len());
        for (id, &v) in p.lists().values().iter().enumerate() {
            let mut expected = ListValueRow {
                range_index: id,
                value: v,
                position: None,
                previous: None,
                next: None,
            };
            for (owner, route) in a.lists().lists().iter().enumerate() {
                if let Some(index) = route.iter().position(|x| *x == v) {
                    expected.position = Some(ListPosition {
                        entity: owner,
                        index,
                    });
                    expected.previous = index.checked_sub(1).map(|i| route[i]);
                    expected.next = route.get(index + 1).copied();
                }
            }
            assert_eq!(values[&id], expected);
        }
    }
    fn edit(variable: usize) -> MixedMove<usize, usize> {
        MixedMove {
            basic: Move::Change {
                variable,
                value: Some(1),
            },
            lists: ListMove::Swap {
                left: ListPosition {
                    entity: 0,
                    index: 0,
                },
                right: ListPosition {
                    entity: 0,
                    index: 1,
                },
            },
        }
    }
    #[test]
    fn actual_policy_ids_payloads_shadows_and_joint_publication_survive_commit_and_drop() {
        let case = Case::parse(SMALL).unwrap();
        for policy in [
            StreamGraphPolicy::OrdinaryJoin,
            StreamGraphPolicy::OwnerLinks,
            StreamGraphPolicy::FusedOwnerLinks,
        ] {
            let trace = RefCell::new(Vec::new());
            let mut b = StreamGraphBuilder::new();
            let s = MixedStreamSources::new(&mut b);
            let inspect = s.clone();
            let h = declare_owner_routes(
                &mut b,
                s.basic.port(),
                s.lists.entities.port(),
                2,
                policy,
                |pair| {
                    trace.borrow_mut().push(pair.clone());
                    contribution(&case, pair, false)
                },
            )
            .unwrap();
            let mut d = MixedStreamScoreDirector::new(
                &case.problem,
                b.build(),
                s,
                case.problem.initial_assignment(),
            )
            .unwrap();
            let original = d.assignment().clone();
            let saved = hold(d.graph(), &h);
            assert_matches(&saved, &case, &original, false);
            assert_rows(d.graph(), &inspect, &case.problem, &original);
            trace.borrow_mut().clear();
            let tx = d.apply(&edit(0)).unwrap();
            let final_pair = ((0, Some(1)), (0, vec![0, 2]));
            let expected = if policy == StreamGraphPolicy::OrdinaryJoin {
                vec![((0, Some(1)), (0, vec![2, 0])), final_pair]
            } else {
                vec![final_pair]
            };
            assert_eq!(*trace.borrow(), expected);
            assert_eq!(tx.constraint_evaluations(), expected.len() as u128);
            assert_eq!(tx.score(), independent(&case, tx.assignment()));
            assert_rows(tx.graph(), &inspect, &case.problem, tx.assignment());
            let candidate = tx.assignment().clone();
            let candidate_snapshot = hold(tx.graph(), &h);
            assert_matches(&candidate_snapshot, &case, &candidate, false);
            drop(tx);
            assert_eq!(d.assignment(), &original);
            assert_matches(&hold(d.graph(), &h), &case, &original, false);
            assert_matches(&candidate_snapshot, &case, &candidate, false);
            assert_matches(&saved, &case, &original, false);
            trace.borrow_mut().clear();
            // Different owner: basic owner1 and route owner0. Publication order differs.
            let tx = d.apply(&edit(1)).unwrap();
            let owner0 = ((0, Some(0)), (0, vec![0, 2]));
            let owner1 = ((1, Some(1)), (1, vec![3, 1]));
            assert_eq!(
                *trace.borrow(),
                if policy == StreamGraphPolicy::OrdinaryJoin {
                    vec![owner1, owner0]
                } else {
                    vec![owner0, owner1]
                }
            );
            assert_eq!(tx.score(), independent(&case, tx.assignment()));
            let committed = tx.assignment().clone();
            tx.commit();
            assert_eq!(d.assignment(), &committed);
            assert_rows(d.graph(), &inspect, &case.problem, &committed);
            assert_matches(&hold(d.graph(), &h), &case, &committed, false);
            assert_matches(&saved, &case, &original, false);
            assert_matches(&candidate_snapshot, &case, &candidate, false);
            d.assert_consistent().unwrap();
        }
    }
    #[test]
    fn callback_panic_restores_real_director_rows_matches_and_allows_recovery() {
        let case = Case::parse(SMALL).unwrap();
        for policy in [
            StreamGraphPolicy::OrdinaryJoin,
            StreamGraphPolicy::OwnerLinks,
            StreamGraphPolicy::FusedOwnerLinks,
        ] {
            let fail = Cell::new(false);
            let trace = RefCell::new(Vec::new());
            let mut b = StreamGraphBuilder::new();
            let s = MixedStreamSources::new(&mut b);
            let inspect = s.clone();
            let h = declare_owner_routes(
                &mut b,
                s.basic.port(),
                s.lists.entities.port(),
                2,
                policy,
                |pair| {
                    trace.borrow_mut().push(pair.clone());
                    assert!(!fail.get(), "original injected route panic");
                    contribution(&case, pair, false)
                },
            )
            .unwrap();
            let mut d = MixedStreamScoreDirector::new(
                &case.problem,
                b.build(),
                s,
                case.problem.initial_assignment(),
            )
            .unwrap();
            let before = d.assignment().clone();
            let snapshot = hold(d.graph(), &h);
            trace.borrow_mut().clear();
            fail.set(true);
            assert!(
                catch_unwind(AssertUnwindSafe(|| {
                    let _tx = d.apply(&edit(0)).unwrap();
                }))
                .is_err()
            );
            fail.set(false);
            // Ordinary errors at its intermediate pair; both link policies error at the final pair.
            // Fused v2 scores immediately after each projection; v1 projects the link batch first.
            assert_eq!(
                trace.borrow()[0],
                (
                    (0, Some(1)),
                    (
                        0,
                        if policy == StreamGraphPolicy::OrdinaryJoin {
                            vec![2, 0]
                        } else {
                            vec![0, 2]
                        }
                    )
                )
            );
            assert_eq!(d.assignment(), &before);
            assert_eq!(d.score(), independent(&case, &before));
            assert_rows(d.graph(), &inspect, &case.problem, &before);
            assert_matches(&hold(d.graph(), &h), &case, &before, false);
            assert_matches(&snapshot, &case, &before, false);
            d.assert_consistent().unwrap();
            let tx = d.apply(&edit(0)).unwrap();
            assert_eq!(tx.score(), independent(&case, tx.assignment()));
            tx.commit();
            assert_rows(d.graph(), &inspect, &case.problem, d.assignment());
            assert_matches(&snapshot, &case, &before, false);
        }
    }
    #[test]
    fn cold_partial_objective_and_complete_metadata_are_independently_checked() {
        let case = Case::parse(SMALL).unwrap();
        let p = cold_problem(&case).unwrap();
        for policy in [
            StreamGraphPolicy::OrdinaryJoin,
            StreamGraphPolicy::OwnerLinks,
            StreamGraphPolicy::FusedOwnerLinks,
        ] {
            let mut b = StreamGraphBuilder::new();
            let s = MixedStreamSources::new(&mut b);
            let inspect = s.clone();
            let h = declare_owner_routes(
                &mut b,
                s.basic.port(),
                s.lists.entities.port(),
                2,
                policy,
                |pair| contribution(&case, pair, true),
            )
            .unwrap();
            let mut d =
                MixedStreamScoreDirector::new(&p, b.build(), s, p.initial_assignment()).unwrap();
            assert_eq!(d.score(), independent(&case, d.assignment()));
            let initial = d.assignment().clone();
            let saved = hold(d.graph(), &h);
            let movement = MixedMove {
                basic: Move::Change {
                    variable: 0,
                    value: Some(1),
                },
                lists: ListMove::Assign {
                    value: 0,
                    destination: ListPosition {
                        entity: 0,
                        index: 0,
                    },
                },
            };
            let tx = d.apply(&movement).unwrap();
            assert_eq!(tx.score(), independent(&case, tx.assignment()));
            assert_matches(&hold(tx.graph(), &h), &case, tx.assignment(), true);
            assert_rows(tx.graph(), &inspect, &p, tx.assignment());
            drop(tx);
            assert_eq!(d.assignment(), &initial);
            assert_matches(&saved, &case, &initial, true);
            let tx = d.apply(&movement).unwrap();
            tx.commit();
            assert_eq!(d.score(), independent(&case, d.assignment()));
            d.assert_consistent().unwrap();
        }
    }
    #[test]
    fn every_real_factory_policy_backend_keeps_small_quality_and_phase_budgets() {
        for policy in [
            StreamGraphPolicy::OrdinaryJoin,
            StreamGraphPolicy::OwnerLinks,
            StreamGraphPolicy::FusedOwnerLinks,
        ] {
            for stream in [false, true] {
                let mut warm = Case::parse(SMALL).unwrap();
                warm.stream_graph_policy = policy;
                let result = run(&warm, stream).unwrap();
                let best = result.best.unwrap();
                warm.validate(&best.assignment, best.score).unwrap();
                assert_eq!(best.score, independent(&warm, &best.assignment));
                assert!(
                    result.completed_steps <= warm.steps && result.evaluations <= warm.evaluations
                );
                let mut cold = Case::parse(&format!("{SMALL}\t0:-12")).unwrap();
                cold.stream_graph_policy = policy;
                let result = run_cold(&cold, stream).unwrap();
                assert!(result.complete);
                assert_eq!(result.score, Some(HardSoftScore::new(0, -12)));
                assert_eq!(
                    result.score.unwrap(),
                    independent(&cold, &result.assignment)
                );
                assert_eq!(result.work.construction_steps, 6);
                assert!(
                    result.work.evaluations <= cold.evaluations
                        && result.work.completed_steps <= cold.steps
                );
                for (case, cold_start) in [(&warm, false), (&cold, true)] {
                    // Real multi-phase sequence exercises search saved candidates/phase handoffs.
                    let result =
                        support::run_component_refinement(case, stream, cold_start).unwrap();
                    assert!(result.complete);
                    case.validate(&result.assignment, result.score.unwrap())
                        .unwrap();
                    assert_eq!(result.score, Some(HardSoftScore::new(0, -12)));
                    assert_eq!(result.score.unwrap(), independent(case, &result.assignment));
                    assert!(
                        result.work.evaluations <= case.evaluations
                            && result.work.completed_steps <= case.steps
                    );
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
                }
            }
        }
    }
}
