//! Original graph-coloring quality controls; no upstream fixture or method credit.
//! These cases measure bounded native policies, not universal quality guarantees.
use serde_json::Value;
use std::cell::{Cell, RefCell};
use timetuck::*;

const SEED: u64 = 20_261_222;
const ATTEMPTS: u64 = 8;

#[derive(Debug)]
struct Case {
    id: String,
    mode: String,
    colors: usize,
    initial: Vec<usize>,
    edges: Vec<(usize, usize)>,
    weights: Vec<Vec<i64>>,
    optimum: HardSoftScore,
    legacy: HardSoftScore,
}

fn integer(value: &Value) -> usize {
    usize::try_from(value.as_u64().expect("nonnegative fixture integer")).unwrap()
}

fn stored_score(value: &Value) -> HardSoftScore {
    let pair = value.as_array().expect("score pair");
    assert_eq!(pair.len(), 2);
    HardSoftScore::new(pair[0].as_i64().unwrap(), pair[1].as_i64().unwrap())
}

fn fixtures() -> Vec<Case> {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/basic_repair_quality.json")).unwrap();
    assert!(fixture["origin"].as_str().unwrap().contains("Original"));
    fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            let input = &row["case"];
            let colors = integer(&input["value_count"]);
            let initial: Vec<_> = input["initial"]
                .as_array()
                .unwrap()
                .iter()
                .map(integer)
                .collect();
            let pins = input["pins"].as_array().unwrap();
            assert_eq!(pins.len(), initial.len());
            assert!(pins.iter().all(|pin| pin.as_bool() == Some(false)));
            let weights: Vec<Vec<i64>> = input["weights"]
                .as_array()
                .unwrap()
                .iter()
                .map(|row| {
                    row.as_array()
                        .unwrap()
                        .iter()
                        .map(|v| v.as_i64().unwrap())
                        .collect()
                })
                .collect();
            assert_eq!(weights.len(), initial.len());
            assert!(
                weights
                    .iter()
                    .all(|row| row.len() == colors && row.iter().all(|w| *w >= row[0]))
            );
            assert!(initial.iter().all(|&v| v == 0));
            assert!((2..=6).contains(&initial.len()));
            assert_eq!(colors, 3);
            let edges: Vec<_> = input["edges"]
                .as_array()
                .unwrap()
                .iter()
                .map(|edge| {
                    let endpoints = edge.as_array().unwrap();
                    assert_eq!(endpoints.len(), 2);
                    (integer(&endpoints[0]), integer(&endpoints[1]))
                })
                .collect();
            assert!(edges.iter().all(|&(a, b)| a < b && b < initial.len()));
            for (i, edge) in edges.iter().enumerate() {
                assert!(!edges[..i].contains(edge));
            }
            Case {
                id: input["id"].as_str().unwrap().to_owned(),
                mode: row["mode"].as_str().unwrap().to_owned(),
                colors,
                initial,
                edges,
                weights,
                optimum: stored_score(&row["optimum"]),
                legacy: stored_score(&row["legacy_score"]),
            }
        })
        .collect()
}

impl Case {
    fn group(&self) -> usize {
        match self.mode.as_str() {
            "half" => self.initial.len().div_ceil(2),
            "all" => self.initial.len(),
            other => panic!("unknown group mode {other}"),
        }
    }

    fn problem(&self) -> FiniteProblem<usize> {
        FiniteProblem::new(
            self.initial
                .iter()
                .map(|&initial| Variable::new((0..self.colors).collect()).with_initial(initial))
                .collect(),
        )
        .unwrap()
    }

    // Full final/exhaustive rescoring uses concrete values and explicit loops.
    // It does not call the partial scoring callback or the example adapter.
    fn complete_score(&self, values: &[usize]) -> HardSoftScore {
        assert_eq!(values.len(), self.initial.len());
        let mut hard = 0;
        let mut soft = 0;
        for &(a, b) in &self.edges {
            if values[a] == values[b] {
                hard -= 1;
            }
        }
        for (i, &color) in values.iter().enumerate() {
            assert!(color < self.colors);
            soft += self.weights[i][color];
        }
        HardSoftScore::new(hard, soft)
    }

    // Construction must also score incomplete states: missing slots contribute
    // neither weight nor graph conflicts.
    fn partial_score(&self, values: &[Option<usize>]) -> HardSoftScore {
        assert_eq!(values.len(), self.initial.len());
        let hard = -(self
            .edges
            .iter()
            .filter(|&&(a, b)| matches!((values[a], values[b]), (Some(x), Some(y)) if x == y))
            .count() as i64);
        let soft = values
            .iter()
            .zip(&self.weights)
            .filter_map(|(color, weights)| color.map(|color| weights[color]))
            .sum();
        HardSoftScore::new(hard, soft)
    }

    fn exhaustive_optimum(&self) -> HardSoftScore {
        let state_count = self.colors.pow(u32::try_from(self.initial.len()).unwrap());
        let mut optimum = self.complete_score(&self.initial);
        // Every base-k representation appears exactly once, including all zero.
        for mut encoded in 0..state_count {
            let mut values = vec![0; self.initial.len()];
            for value in &mut values {
                *value = encoded % self.colors;
                encoded /= self.colors;
            }
            optimum = optimum.max(self.complete_score(&values));
        }
        optimum
    }
}

fn budget(case: &Case, placement: ConstructionPlacement) -> (u64, u64) {
    let group = case.group() as u64;
    let colors = case.colors as u64;
    // Sequential scans k candidates per missing slot. Pooled rescans every
    // remaining slot after each retained insertion: k*(g + ... + 1) candidates.
    // Each reconstruction has one initialization score; so does the outer solve.
    let candidates = match placement {
        ConstructionPlacement::Sequential => colors * group,
        ConstructionPlacement::Pooled => colors * group * (group + 1) / 2,
        _ => panic!("quality matrix only supports sequential and pooled"),
    };
    (ATTEMPTS * (candidates + 1) + 2, ATTEMPTS * candidates)
}

fn repair_config(
    case: &Case,
    sampling: RuinSampling,
    placement: ConstructionPlacement,
) -> RuinRecreateConfig<'static, HardSoftScore> {
    let (evaluation_cap, move_cap) = budget(case, placement);
    RuinRecreateConfig {
        seed: SEED,
        minimum_ruined: case.group(),
        maximum_ruined: case.group(),
        max_attempts: ATTEMPTS,
        max_evaluations: evaluation_cap,
        max_move_evaluations: Some(move_cap + 1),
        acceptance: Acceptance::StepCountingHillClimbing {
            period: 9,
            mode: StepCountingMode::Step,
        },
        ruin_sampling: sampling,
        placement,
        ..Default::default()
    }
}

fn run_both(
    case: &Case,
    sampling: RuinSampling,
    placement: ConstructionPlacement,
) -> RuinRecreateResult<usize, HardSoftScore> {
    let problem = case.problem();
    let (evaluation_cap, move_cap) = budget(case, placement);
    let config = repair_config(case, sampling, placement);
    let size = case.initial.len();
    let mut network = ConstraintNetwork::new(size);
    network
        .add_constraint("graph-and-weights", (0..size).collect(), |values| {
            case.partial_score(
                &values
                    .iter()
                    .map(|value| value.copied())
                    .collect::<Vec<_>>(),
            )
        })
        .unwrap();
    let dense = ruin_recreate_search_from(
        &problem,
        &network,
        problem.initial_assignment(),
        config.clone(),
    )
    .unwrap();
    let stream = ruin_recreate_stream_search_from(
        &problem,
        || {
            let mut builder = StreamGraphBuilder::new();
            let source = builder.source::<usize, (usize, Option<usize>)>();
            let all = builder.collect_list(source.port(), |row| *row)?;
            builder.score("graph-and-weights", &all, |rows| {
                let mut values = vec![None; size];
                for &(index, color) in rows.iter() {
                    values[index] = color;
                }
                case.partial_score(&values)
            })?;
            Ok((builder.build(), source))
        },
        problem.initial_assignment(),
        config,
    )
    .unwrap();
    assert_eq!(
        dense.best, stream.best,
        "{} {} {sampling:?} {placement:?}",
        case.id, case.mode
    );
    let logical_work = |result: &RuinRecreateResult<usize, HardSoftScore>| {
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
    };
    assert_eq!(logical_work(&dense), logical_work(&stream));
    // The Stream collect-list terminal scores each planning-row insertion during
    // director initialization; Dense scores the full network once. Preserve
    // actual physical work rather than counting it as additional search trials.
    assert_eq!(dense.constraint_evaluations, u128::from(dense.evaluations));
    assert_eq!(
        stream.constraint_evaluations,
        u128::from(stream.evaluations + (case.initial.len() as u64 - 1) * (ATTEMPTS + 1))
    );
    println!(
        "PHYSICAL {} {} {sampling:?}/{placement:?}: dense_constraints={}, stream_constraints={}",
        case.id, case.mode, dense.constraint_evaluations, stream.constraint_evaluations
    );
    assert_eq!(dense.attempts, ATTEMPTS);
    assert_eq!(dense.complete_reconstructions, ATTEMPTS);
    assert_eq!(dense.termination, RuinRecreateTermination::AttemptLimit);
    assert_eq!(dense.fine_move_attempts, 0);
    assert_eq!(dense.fine_move_evaluations, 0);
    assert!(dense.accepted_moves <= ATTEMPTS);
    assert_eq!(dense.repair_move_evaluations, move_cap);
    assert_eq!(dense.evaluations, move_cap + ATTEMPTS + 1);
    assert!(dense.evaluations < evaluation_cap);
    assert!(dense.repair_move_evaluations < move_cap + 1);
    let best = dense.best.as_ref().expect("complete retained best");
    assert!(problem.is_complete(&best.assignment));
    let concrete: Vec<_> = best
        .assignment
        .values()
        .iter()
        .map(|v| v.expect("required slot"))
        .collect();
    let score = case.complete_score(&concrete);
    assert_eq!(best.score, score);
    assert!(score >= case.complete_score(&case.initial));
    assert!(score <= case.optimum);
    dense
}

#[derive(Debug, Default, Eq, PartialEq)]
struct Quality {
    optimal: usize,
    improved: usize,
    equal: usize,
    regressed: usize,
}

#[test]
fn four_native_repair_policies_retain_real_bounded_quality_and_backend_parity() {
    let cases = fixtures();
    assert_eq!(cases.len(), 64);
    assert_eq!(cases.iter().filter(|case| case.mode == "half").count(), 32);
    assert_eq!(cases.iter().filter(|case| case.mode == "all").count(), 32);
    assert_eq!(
        RuinRecreateConfig::<HardSoftScore>::default().ruin_sampling,
        RuinSampling::Independent
    );
    assert_eq!(
        RuinRecreateConfig::<HardSoftScore>::default().placement,
        ConstructionPlacement::Sequential
    );
    let variants = [
        (RuinSampling::Independent, ConstructionPlacement::Sequential),
        (RuinSampling::Independent, ConstructionPlacement::Pooled),
        (
            RuinSampling::CoverageCycle,
            ConstructionPlacement::Sequential,
        ),
        (RuinSampling::CoverageCycle, ConstructionPlacement::Pooled),
    ];
    let mut qualities = variants.map(|_| Quality::default());
    for case in &cases {
        assert_eq!(
            case.exhaustive_optimum(),
            case.optimum,
            "{} {} stored Python optimum",
            case.id,
            case.mode
        );
        for (index, &(sampling, placement)) in variants.iter().enumerate() {
            let result = run_both(case, sampling, placement);
            let score = result.best.unwrap().score;
            if index == 0 {
                assert_eq!(
                    score, case.legacy,
                    "{} {} exact default legacy score",
                    case.id, case.mode
                );
            }
            qualities[index].optimal += usize::from(score == case.optimum);
            match score.cmp(&case.legacy) {
                std::cmp::Ordering::Greater => qualities[index].improved += 1,
                std::cmp::Ordering::Equal => qualities[index].equal += 1,
                std::cmp::Ordering::Less => qualities[index].regressed += 1,
            }
            println!(
                "QUALITY {} {} {sampling:?}/{placement:?}: score={score}, legacy={}, optimum={}, evaluations={}, accepted={}",
                case.id,
                case.mode,
                case.legacy,
                case.optimum,
                result.evaluations,
                result.accepted_moves
            );
        }
    }
    for (variant, quality) in variants.iter().zip(&qualities) {
        println!("QUALITY SUMMARY {variant:?}: {quality:?}");
    }
    // Qualified to these original fixtures, seed, fixed groups and eight attempts.
    // Pooled placement deliberately retains real regressions as well as gains.
    assert_eq!(
        qualities,
        [
            Quality {
                optimal: 33,
                improved: 0,
                equal: 64,
                regressed: 0
            },
            Quality {
                optimal: 46,
                improved: 21,
                equal: 40,
                regressed: 3
            },
            Quality {
                optimal: 36,
                improved: 5,
                equal: 59,
                regressed: 0
            },
            Quality {
                optimal: 47,
                improved: 22,
                equal: 40,
                regressed: 2
            },
        ]
    );
}

fn logical_work(work: SolverWork) -> SolverWork {
    // Physical graph callbacks are separately asserted for each backend below.
    SolverWork {
        constraint_evaluations: 0,
        ..work
    }
}

fn observe_retained(
    case: &Case,
    update: SolverUpdate<'_, Assignment<usize>, HardSoftScore>,
    updates: &mut Vec<(usize, Assignment<usize>, HardSoftScore)>,
) {
    assert!(update.complete);
    assert!(update.phase < 2);
    assert_eq!(update.kind, SolverPhaseKind::RuinRecreate);
    let values: Vec<_> = update
        .assignment
        .values()
        .iter()
        .map(|v| v.expect("required slot"))
        .collect();
    assert_eq!(update.score, case.complete_score(&values));
    if let Some((phase, _, score)) = updates.last() {
        assert!(update.phase >= *phase);
        assert!(
            update.score > *score,
            "root notifications retain strictly improving best scores"
        );
    }
    updates.push((update.phase, update.assignment.clone(), update.score));
}

fn run_phased_both(case: &Case) -> SolverResult<Assignment<usize>, HardSoftScore> {
    let problem = case.problem();
    let size = case.initial.len();
    let (first_cap, first_candidates) = budget(case, ConstructionPlacement::Sequential);
    let (second_cap, second_candidates) = budget(case, ConstructionPlacement::Pooled);
    let solve_cap = first_cap + second_cap + 2;
    let solve_move_cap = first_candidates + second_candidates + 2;
    let sequence = SolverSequence {
        limits: SolverLimits {
            max_evaluations: solve_cap,
            max_move_evaluations: Some(solve_move_cap),
            ..Default::default()
        },
        phases: vec![
            SolverPhase::RuinRecreate(repair_config(
                case,
                RuinSampling::Independent,
                ConstructionPlacement::Sequential,
            )),
            SolverPhase::RuinRecreate(repair_config(
                case,
                RuinSampling::CoverageCycle,
                ConstructionPlacement::Pooled,
            )),
        ],
    };
    let first_evaluations = first_candidates + ATTEMPTS + 1;
    let first_stream_constraints = first_evaluations + (size as u64 - 1) * (ATTEMPTS + 1);
    let dense_calls = Cell::new(0_u64);
    let dense_entry = RefCell::new(None);
    let mut network = ConstraintNetwork::new(size);
    network
        .add_constraint("graph-and-weights", (0..size).collect(), |rows| {
            let values: Vec<_> = rows.iter().map(|value| value.copied()).collect();
            if dense_calls.get() == first_evaluations {
                *dense_entry.borrow_mut() = Some(values.clone());
            }
            dense_calls.set(dense_calls.get() + 1);
            case.partial_score(&values)
        })
        .unwrap();
    let mut dense_updates = Vec::new();
    let dense = solve_with_observer(&problem, &network, sequence.clone(), |update| {
        observe_retained(case, update, &mut dense_updates);
    })
    .unwrap();
    let stream_calls = Cell::new(0_u64);
    let stream_entry = RefCell::new(None);
    let mut stream_updates = Vec::new();
    let stream = solve_stream_with_observer(
        &problem,
        || {
            let mut builder = StreamGraphBuilder::new();
            let source = builder.source::<usize, (usize, Option<usize>)>();
            let all = builder.collect_list(source.port(), |row| *row)?;
            builder.score("graph-and-weights", &all, |rows| {
                let mut values = vec![None; size];
                for &(index, color) in rows.iter() {
                    values[index] = color;
                }
                // Read the fully populated second-phase initialization after
                // all planning rows have entered its fresh scoring graph.
                if stream_calls.get() == first_stream_constraints + size as u64 - 1 {
                    *stream_entry.borrow_mut() = Some(values.clone());
                }
                stream_calls.set(stream_calls.get() + 1);
                case.partial_score(&values)
            })?;
            Ok((builder.build(), source))
        },
        sequence,
        |update| observe_retained(case, update, &mut stream_updates),
    )
    .unwrap();
    assert_eq!(dense.assignment, stream.assignment);
    assert_eq!(dense.score, stream.score);
    assert_eq!(dense.complete, stream.complete);
    assert_eq!(dense.termination, stream.termination);
    assert_eq!(logical_work(dense.work), logical_work(stream.work));
    assert_eq!(dense_updates, stream_updates);
    let first_best = dense_updates
        .iter()
        .rfind(|(phase, _, _)| *phase == 0)
        .unwrap();
    assert_eq!(first_best.2, case.legacy);
    // These are observations from the scoring callbacks, never scripted inputs.
    // The second phase consumes the actual first-phase retained assignment.
    assert_eq!(dense_entry.borrow().as_deref(), Some(first_best.1.values()));
    assert_eq!(
        stream_entry.borrow().as_deref(),
        Some(first_best.1.values())
    );
    assert_eq!(dense_updates.last().unwrap().2, dense.score.unwrap());
    assert_eq!(dense_updates.last().unwrap().1, dense.assignment);
    assert_eq!(dense.phases.len(), 2);
    assert_eq!(stream.phases.len(), 2);
    for (index, ((a, b), (cap, candidates))) in dense
        .phases
        .iter()
        .zip(&stream.phases)
        .zip([
            (first_cap, first_candidates),
            (second_cap, second_candidates),
        ])
        .enumerate()
    {
        assert_eq!(a.index, index);
        assert_eq!(b.index, index);
        assert_eq!(a.score, b.score);
        assert_eq!(
            a.termination,
            PhaseTermination::RuinRecreate(RuinRecreateTermination::AttemptLimit)
        );
        assert_eq!(a.termination, b.termination);
        assert_eq!(logical_work(a.work), logical_work(b.work));
        assert_eq!(a.work.repair_attempts, ATTEMPTS);
        assert_eq!(a.work.complete_reconstructions, ATTEMPTS);
        assert_eq!(a.work.move_evaluations, candidates);
        assert_eq!(a.work.evaluations, candidates + ATTEMPTS + 1);
        assert!(a.work.evaluations < cap);
        assert_eq!(a.work.fine_move_attempts, 0);
        assert_eq!(a.work.construction_steps, 0);
        assert_eq!(a.work.completed_steps, a.work.accepted_moves);
        assert!(a.work.accepted_moves <= ATTEMPTS);
        assert_eq!(
            a.work.constraint_evaluations,
            u128::from(a.work.evaluations)
        );
        assert_eq!(
            b.work.constraint_evaluations,
            u128::from(b.work.evaluations + (size as u64 - 1) * (ATTEMPTS + 1))
        );
    }
    assert_eq!(dense.phases[0].score, Some(case.legacy));
    assert!(dense.phases[1].score.unwrap() >= case.legacy);
    assert_eq!(dense.score, dense.phases[1].score);
    assert_eq!(dense.work.repair_attempts, 2 * ATTEMPTS);
    assert_eq!(dense.work.complete_reconstructions, 2 * ATTEMPTS);
    assert_eq!(
        dense.work.move_evaluations,
        first_candidates + second_candidates
    );
    assert_eq!(
        dense.work.evaluations,
        first_candidates + second_candidates + 2 * (ATTEMPTS + 1)
    );
    assert!(dense.work.evaluations < solve_cap);
    assert!(dense.work.move_evaluations < solve_move_cap);
    assert_eq!(
        u128::from(dense_calls.get()),
        dense.work.constraint_evaluations
    );
    assert_eq!(
        u128::from(stream_calls.get()),
        stream.work.constraint_evaluations
    );
    assert_eq!(dense.termination, SolverTermination::PlanCompleted);
    assert!(
        !dense.is_optimal(),
        "exhaustive test oracle does not make heuristic solves optimality proofs"
    );
    assert!(dense.complete && problem.is_complete(&dense.assignment));
    let values: Vec<_> = dense
        .assignment
        .values()
        .iter()
        .map(|v| v.expect("required slot"))
        .collect();
    let score = case.complete_score(&values);
    assert_eq!(dense.score, Some(score));
    assert!(score >= case.legacy);
    assert!(score <= case.optimum);
    println!(
        "PHASED {} {}: score={score}, legacy={}, optimum={}, evaluations={}, attempts={}, dense_constraints={}, stream_constraints={}",
        case.id,
        case.mode,
        case.legacy,
        case.optimum,
        dense.work.evaluations,
        dense.work.repair_attempts,
        dense.work.constraint_evaluations,
        stream.work.constraint_evaluations
    );
    dense
}

#[test]
fn two_real_repair_phases_preserve_the_default_best_through_handoff() {
    let cases = fixtures();
    assert_eq!(cases.len(), 64);
    let mut quality = Quality::default();
    for case in &cases {
        assert_eq!(case.exhaustive_optimum(), case.optimum);
        let result = run_phased_both(case);
        let score = result.score.unwrap();
        quality.optimal += usize::from(score == case.optimum);
        match score.cmp(&case.legacy) {
            std::cmp::Ordering::Greater => quality.improved += 1,
            std::cmp::Ordering::Equal => quality.equal += 1,
            std::cmp::Ordering::Less => quality.regressed += 1,
        }
    }
    println!("PHASED SUMMARY {quality:?}");
    // This is a distinct sixteen-attempt profile. Its count is measured on this
    // fixture suite, not inherited from the eight-attempt single-policy result.
    assert_eq!(
        quality,
        Quality {
            optimal: 48,
            improved: 21,
            equal: 43,
            regressed: 0
        }
    );
}
