//! Original workload representation checks; no upstream-method credit.
#[path = "../examples/repair_scoring/mod.rs"]
mod repair_scoring;
#[path = "../examples/solver_support/mod.rs"]
mod solver_support;
use repair_scoring::{ScoringModel, network};
use serde_json::Value;
use solver_support::Case;
use timetuck::*;

fn independent(case: &Case, assignment: &Assignment<usize>) -> HardSoftScore {
    let mut hard = 0;
    let mut soft = 0;
    for &(left, right) in &case.edges {
        if let (Some(a), Some(b)) = (assignment.values()[left], assignment.values()[right])
            && a == b
        {
            hard -= 1;
        }
    }
    for (index, value) in assignment.values().iter().enumerate() {
        if let Some(color) = value {
            soft += case.weights[index][*color];
        }
    }
    HardSoftScore::new(hard, soft)
        .with_structural(-(assignment.values().iter().filter(|v| v.is_none()).count() as i64))
}

fn score_pair(value: &Value) -> HardSoftScore {
    HardSoftScore::new(value[0].as_i64().unwrap(), value[1].as_i64().unwrap())
}

fn case_from_json(value: &Value) -> Case {
    let list = |field: &str| {
        value[field]
            .as_array()
            .unwrap()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",")
    };
    let edges = value["edges"]
        .as_array()
        .unwrap()
        .iter()
        .map(|edge| format!("{}:{}", edge[0], edge[1]))
        .collect::<Vec<_>>()
        .join(",");
    let weights = value["weights"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|row| row.as_array().unwrap())
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let pins = value["pins"]
        .as_array()
        .unwrap()
        .iter()
        .map(|pin| if pin.as_bool().unwrap() { '1' } else { '0' })
        .collect::<String>();
    Case::parse(
        &format!(
            "{}\t{}\t{}\t{pins}\t{edges}\t{weights}",
            value["id"].as_str().unwrap(),
            value["value_count"],
            list("initial")
        ),
        true,
    )
    .unwrap()
}

type LogicalWork = (u64, u64, u64, u64, u64, u64, u64, RuinRecreateTermination);
fn logical(result: &RuinRecreateResult<usize, HardSoftScore>) -> LogicalWork {
    (
        result.evaluations,
        result.repair_move_evaluations,
        result.fine_move_attempts,
        result.fine_move_evaluations,
        result.attempts,
        result.complete_reconstructions,
        result.accepted_moves,
        result.termination,
    )
}

#[test]
fn real_two_phase_profile_preserves_handoff_updates_quality_and_logical_work() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/basic_repair_quality.json")).unwrap();
    let mut quality = [0usize; 4];
    for row in fixture["cases"].as_array().unwrap() {
        let case = case_from_json(&row["case"]);
        let size = case.problem.variables().len();
        let group = if row["mode"] == "half" {
            size.div_ceil(2)
        } else {
            size
        };
        let sequential_candidates = (group * case.weights[0].len()) as u64;
        let pooled_candidates = (group * (group + 1) / 2 * case.weights[0].len()) as u64;
        let cap = |candidates| 8 * (candidates + 1) + 2;
        let first = RuinRecreateConfig {
            seed: 20261222,
            minimum_ruined: group,
            maximum_ruined: group,
            max_attempts: 8,
            max_evaluations: cap(sequential_candidates),
            acceptance: Acceptance::StepCountingHillClimbing {
                period: 9,
                mode: StepCountingMode::Step,
            },
            ..Default::default()
        };
        let second = RuinRecreateConfig {
            ruin_sampling: RuinSampling::CoverageCycle,
            placement: ConstructionPlacement::Pooled,
            max_evaluations: cap(pooled_candidates),
            ..first.clone()
        };
        let plan = SolverSequence {
            limits: SolverLimits {
                max_evaluations: cap(sequential_candidates) + cap(pooled_candidates) + 4,
                ..Default::default()
            },
            phases: vec![
                SolverPhase::RuinRecreate(first),
                SolverPhase::RuinRecreate(second),
            ],
        };
        let results = [
            ScoringModel::Full,
            ScoringModel::Incremental,
            ScoringModel::IncrementalCached,
        ]
        .map(|model| {
            let network = network(&case, model).unwrap();
            let mut updates = Vec::new();
            let result = solve_with_observer(&case.problem, &network, plan.clone(), |update| {
                assert!(update.complete);
                assert_eq!(update.kind, SolverPhaseKind::RuinRecreate);
                assert!(update.phase < 2);
                assert_eq!(update.score, independent(&case, update.assignment));
                if let Some((phase, _, score)) = updates.last() {
                    assert!(update.phase >= *phase);
                    assert!(update.score > *score);
                }
                updates.push((update.phase, update.assignment.clone(), update.score));
            })
            .unwrap();
            assert!(result.complete && case.problem.is_complete(&result.assignment));
            case.problem.validate(&result.assignment).unwrap();
            assert_eq!(result.score, Some(independent(&case, &result.assignment)));
            assert_eq!(result.phases.len(), 2);
            assert_eq!(
                result.phases[0].score,
                Some(score_pair(&row["legacy_score"]))
            );
            assert_eq!(result.score, result.phases[1].score);
            assert!(result.score >= result.phases[0].score);
            assert_eq!(result.work.repair_attempts, 16);
            assert_eq!(result.work.complete_reconstructions, 16);
            assert!(result.work.evaluations < plan.limits.max_evaluations);
            assert_eq!(result.termination, SolverTermination::PlanCompleted);
            assert!(!result.is_optimal());
            for phase in &result.phases {
                assert_eq!(phase.work.repair_attempts, 8);
                assert_eq!(
                    phase.termination,
                    PhaseTermination::RuinRecreate(RuinRecreateTermination::AttemptLimit)
                );
            }
            (result, updates)
        });
        let logical_work = |mut work: SolverWork| {
            work.constraint_evaluations = 0;
            work
        };
        for (result, updates) in &results[1..] {
            assert_eq!(results[0].0.assignment, result.assignment);
            assert_eq!(results[0].0.score, result.score);
            assert_eq!(results[0].0.termination, result.termination);
            assert_eq!(&results[0].1, updates);
            assert_eq!(logical_work(results[0].0.work), logical_work(result.work));
            for (full, alternate) in results[0].0.phases.iter().zip(&result.phases) {
                let mut full = full.clone();
                let mut alternate = alternate.clone();
                full.work = logical_work(full.work);
                alternate.work = logical_work(alternate.work);
                assert_eq!(full, alternate);
            }
        }
        assert_eq!(
            results[1].0.phases[0].work.constraint_evaluations,
            results[2].0.phases[0].work.constraint_evaluations
        );
        assert!(
            results[2].0.phases[1].work.constraint_evaluations
                <= results[1].0.phases[1].work.constraint_evaluations
        );
        assert!(
            results[2].0.work.constraint_evaluations <= results[1].0.work.constraint_evaluations
        );
        let score = results[0].0.score.unwrap();
        quality[0] += usize::from(score == score_pair(&row["optimum"]));
        let comparison = match score.cmp(&score_pair(&row["legacy_score"])) {
            std::cmp::Ordering::Greater => 1,
            std::cmp::Ordering::Equal => 2,
            std::cmp::Ordering::Less => 3,
        };
        quality[comparison] += 1;
    }
    assert_eq!(quality, [48, 21, 43, 0]);
}

#[test]
fn both_models_preserve_all_64_fixture_searches_for_all_four_policies() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/basic_repair_quality.json")).unwrap();
    let rows = fixture["cases"].as_array().unwrap();
    assert_eq!(rows.len(), 64);
    let policies = [
        (RuinSampling::Independent, ConstructionPlacement::Sequential),
        (RuinSampling::Independent, ConstructionPlacement::Pooled),
        (
            RuinSampling::CoverageCycle,
            ConstructionPlacement::Sequential,
        ),
        (RuinSampling::CoverageCycle, ConstructionPlacement::Pooled),
    ];
    let mut quality = [[0usize; 4]; 4];
    for row in rows {
        let case = case_from_json(&row["case"]);
        let size = case.problem.variables().len();
        let group = match row["mode"].as_str().unwrap() {
            "half" => size.div_ceil(2),
            "all" => size,
            _ => panic!("fixture group mode"),
        };
        let full = network(&case, ScoringModel::Full).unwrap();
        let incremental = network(&case, ScoringModel::Incremental).unwrap();
        let cached = network(&case, ScoringModel::IncrementalCached).unwrap();
        assert_eq!(full.constraint_count(), 1);
        assert_eq!(incremental.constraint_count(), case.edges.len() + size);
        assert_eq!(cached.constraint_count(), incremental.constraint_count());
        for (index, &(sampling, placement)) in policies.iter().enumerate() {
            let placements = match placement {
                ConstructionPlacement::Sequential => group,
                ConstructionPlacement::Pooled => group * (group + 1) / 2,
                _ => unreachable!(),
            };
            let candidates = (placements * case.weights[0].len()) as u64;
            let budget = 8 * (candidates + 1) + 2;
            let config = RuinRecreateConfig {
                seed: 20261222,
                ruin_sampling: sampling,
                placement,
                minimum_ruined: group,
                maximum_ruined: group,
                max_attempts: 8,
                max_evaluations: budget,
                max_move_evaluations: Some(8 * candidates + 1),
                acceptance: Acceptance::StepCountingHillClimbing {
                    period: 9,
                    mode: StepCountingMode::Step,
                },
                ..Default::default()
            };
            let results = [&full, &incremental, &cached].map(|network| {
                ruin_recreate_search_from(
                    &case.problem,
                    network,
                    case.problem.initial_assignment(),
                    config.clone(),
                )
                .unwrap()
            });
            for result in &results[1..] {
                assert_eq!(results[0].best, result.best, "{} {index}", case.id);
                assert_eq!(logical(&results[0]), logical(result));
            }
            if placement == ConstructionPlacement::Sequential {
                assert_eq!(
                    results[2].constraint_evaluations,
                    results[1].constraint_evaluations
                );
            } else {
                assert!(results[2].constraint_evaluations <= results[1].constraint_evaluations);
            }
            assert_eq!(
                results[0].constraint_evaluations,
                u128::from(results[0].evaluations)
            );
            for result in &results {
                assert_eq!(result.attempts, 8);
                assert_eq!(result.complete_reconstructions, 8);
                assert_eq!(result.termination, RuinRecreateTermination::AttemptLimit);
                assert_eq!(result.evaluations, 8 * candidates + 9);
                assert_eq!(result.repair_move_evaluations, 8 * candidates);
                assert!(result.evaluations < budget);
                let best = result.best.as_ref().unwrap();
                case.problem.validate(&best.assignment).unwrap();
                assert!(case.problem.is_complete(&best.assignment));
                assert_eq!(best.score, independent(&case, &best.assignment));
                assert_eq!(best.score, case.score(&best.assignment));
                assert!(best.score <= score_pair(&row["optimum"]));
            }
            let score = results[0].best.as_ref().unwrap().score;
            let legacy = score_pair(&row["legacy_score"]);
            if index == 0 {
                assert_eq!(score, legacy);
            }
            quality[index][0] += usize::from(score == score_pair(&row["optimum"]));
            let comparison = match score.cmp(&legacy) {
                std::cmp::Ordering::Greater => 1,
                std::cmp::Ordering::Equal => 2,
                std::cmp::Ordering::Less => 3,
            };
            quality[index][comparison] += 1;
        }
    }
    assert_eq!(
        quality,
        [
            [33, 0, 64, 0],
            [46, 21, 40, 3],
            [36, 5, 59, 0],
            [47, 22, 40, 2]
        ]
    );
}

#[test]
fn duplicate_reversed_self_edges_and_unassigned_zero_survive_moves_and_undo() {
    let case = Case::parse(
        "edge-boundary\t3\t0,0,0\t000\t0:0,0:1,0:1,1:0,1:2\t-4,2,5,-3,7,9,-2,3,6",
        true,
    )
    .unwrap();
    let models = [
        ScoringModel::Full,
        ScoringModel::Incremental,
        ScoringModel::IncrementalCached,
    ];
    let networks = models.map(|model| network(&case, model).unwrap());
    // All64 partial states: None and each of three colors independently per slot.
    for mut encoded in 0..64 {
        let values = (0..3)
            .map(|_| {
                let color = encoded % 4;
                encoded /= 4;
                (color > 0).then(|| color - 1)
            })
            .collect();
        let assignment = Assignment::new(values);
        let expected = independent(&case, &assignment);
        for network in &networks {
            assert_eq!(
                network.calculate(&case.problem, &assignment).unwrap(),
                expected
            );
            let mut director =
                ScoreDirector::new(&case.problem, network, assignment.clone()).unwrap();
            for value in [None, Some(0), Some(1), Some(2)] {
                let movement = Move::Change { variable: 2, value };
                let before = director.score();
                {
                    let trial = director.apply(&movement).unwrap();
                    assert_eq!(trial.score(), independent(&case, trial.assignment()));
                }
                assert_eq!(director.score(), before);
                assert_eq!(director.assignment(), &assignment);
            }
            director.assert_consistent().unwrap();
        }
    }
    let mut director = ScoreDirector::new(
        &case.problem,
        &networks[1],
        case.problem.initial_assignment(),
    )
    .unwrap();
    let trial = director
        .apply(&Move::Change {
            variable: 2,
            value: Some(1),
        })
        .unwrap();
    // Only the weight and edge1:2 callbacks are affected, not the duplicate/self edges.
    assert_eq!(trial.constraint_evaluations(), 2);
    trial.commit();
    director.assert_consistent().unwrap();
    assert_eq!(director.score(), independent(&case, director.assignment()));
}

#[test]
fn scoring_model_defaults_and_explicit_modes_are_backward_compatible() {
    assert_eq!(ScoringModel::default(), ScoringModel::Full);
    assert_eq!(ScoringModel::parse(None), Ok(ScoringModel::Full));
    assert_eq!(ScoringModel::parse(Some("full")), Ok(ScoringModel::Full));
    assert_eq!(
        ScoringModel::parse(Some("incremental")),
        Ok(ScoringModel::Incremental)
    );
    assert_eq!(
        ScoringModel::parse(Some("incremental-cached")),
        Ok(ScoringModel::IncrementalCached)
    );
    assert!(ScoringModel::parse(Some("unknown")).is_err());
}
