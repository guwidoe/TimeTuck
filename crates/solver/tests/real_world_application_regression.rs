//! Original RealWorld application regressions; zero upstream method credit.
#[path = "../examples/support/real_world_model.rs"]
mod model;

use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use timetuck::*;

const GOLFER: &str =
    include_str!("../../../tests/application/real_world/social-golfer-32x8x10-v1/problem.json");
const SAILING: &str =
    include_str!("../../../tests/application/real_world/sailing-trip-v1/problem.json");

fn applies(value: &Value, session: usize) -> bool {
    value
        .get("session_indices")
        .and_then(Value::as_array)
        .is_none_or(|items| {
            items
                .iter()
                .any(|item| item.as_u64() == Some(session as u64))
        })
}
fn weight(value: &Value) -> i64 {
    (value.as_f64().unwrap_or(1000.0) * 100.0).round() as i64
}

/// Independent JSON-level oracle: enumerate unordered people pairs directly.
/// This does not use planning-variable indexes, stream nodes or model internals.
fn recompute(problem: &Value, schedule: &Value) -> (HardSoftScore, usize, usize) {
    let sc = &problem["scenario"];
    let people = sc["people"].as_array().unwrap();
    let groups = sc["groups"].as_array().unwrap();
    let sessions = sc["session_count"].as_u64().unwrap() as usize;
    let mut hard = 0;
    let mut soft = 0;
    let mut encounters = BTreeMap::<(String, String), i64>::new();
    let mut pair_occurrences = 0;
    for session in 0..sessions {
        let row = schedule[format!("session_{session}")].as_object().unwrap();
        assert_eq!(row.len(), groups.len());
        let mut assigned = BTreeMap::<&str, &str>::new();
        for group in groups {
            let gid = group["id"].as_str().unwrap();
            let occupants = row[gid].as_array().unwrap();
            let capacity = group.get("session_capacities").map_or_else(
                || group["capacity"].as_i64().unwrap(),
                |values| values[session].as_i64().unwrap(),
            );
            hard -= (occupants.len() as i64 - capacity).max(0);
            for person in occupants {
                let pid = person.as_str().unwrap();
                assert!(people.iter().any(|p| p["id"] == pid && applies(p, session)));
                assert!(assigned.insert(pid, gid).is_none(), "duplicate attendee");
            }
        }
        for person in people {
            assert_eq!(
                assigned.contains_key(person["id"].as_str().unwrap()),
                applies(person, session)
            );
        }
        let ids = assigned.keys().copied().collect::<Vec<_>>();
        for (i, &left) in ids.iter().enumerate() {
            for &right in &ids[i + 1..] {
                if assigned[left] == assigned[right] {
                    *encounters.entry((left.into(), right.into())).or_default() += 1;
                    pair_occurrences += 1;
                }
            }
        }
        for constraint in problem["constraints"].as_array().unwrap() {
            if !applies(constraint, session) {
                continue;
            }
            let kind = constraint["type"].as_str().unwrap();
            if kind == "attribute_balance" {
                for group in groups {
                    let gid = group["id"].as_str().unwrap();
                    if constraint
                        .get("group_ids")
                        .and_then(Value::as_array)
                        .is_some_and(|ids| !ids.iter().any(|id| id == gid))
                    {
                        continue;
                    }
                    let members = assigned
                        .iter()
                        .filter(|(_, g)| **g == gid)
                        .map(|(p, _)| *p)
                        .collect::<BTreeSet<_>>();
                    // Original stream semantics emit no tally for an empty group.
                    if members.is_empty() {
                        continue;
                    }
                    for (attribute, target) in constraint["target_counts"].as_object().unwrap() {
                        let key = constraint["attribute_key"].as_str().unwrap();
                        let count = people
                            .iter()
                            .filter(|p| {
                                members.contains(p["id"].as_str().unwrap())
                                    && p["attributes"][key].as_str().unwrap_or_default()
                                        == attribute
                            })
                            .count() as i64;
                        let gap = target.as_i64().unwrap() - count;
                        soft -= weight(&constraint["penalty_weight"])
                            * if constraint["target_mode"] == "at_least" {
                                gap.max(0)
                            } else {
                                gap.abs()
                            };
                    }
                }
                continue;
            }
            let requested = constraint["person_ids"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|p| assigned.get(p.as_str().unwrap()).copied())
                .collect::<Vec<_>>();
            if kind == "fixed_placement" {
                assert!(
                    requested
                        .iter()
                        .all(|g| Some(*g) == constraint["group_id"].as_str())
                );
                continue;
            }
            for i in 0..requested.len() {
                for j in i + 1..requested.len() {
                    let same = requested[i] == requested[j];
                    match kind {
                        "keep_together" if !same => hard -= 1,
                        "keep_apart" if same => hard -= 1,
                        "prefer_together" if !same => soft -= weight(&constraint["penalty_weight"]),
                        "prefer_apart" if same => soft -= weight(&constraint["penalty_weight"]),
                        "keep_together" | "keep_apart" | "prefer_together" | "prefer_apart" => (),
                        _ => panic!("unsupported public fixture constraint {kind}"),
                    }
                }
            }
        }
    }
    for objective in problem["objectives"].as_array().unwrap() {
        match objective["type"].as_str().unwrap() {
            "maximize_unique_contacts" => {
                soft += weight(&objective["weight"]) * encounters.len() as i64
            }
            "avoid_repeats" => {
                let maximum = objective["max_encounters_per_pair"].as_i64().unwrap();
                for count in encounters.values() {
                    let excess = (count - maximum).max(0);
                    soft -= weight(&objective["weight"])
                        * if objective["penalty_curve"] == "squared" {
                            excess * excess
                        } else {
                            excess
                        };
                }
            }
            kind => panic!("unsupported public fixture objective {kind}"),
        }
    }
    (
        HardSoftScore::new(hard, soft),
        pair_occurrences,
        encounters.len(),
    )
}

fn full_score_and_rollback(input: &str, dimensions: (usize, usize, usize, usize)) {
    let problem: Value = serde_json::from_str(input).unwrap();
    let (m, fp, initial) = model::load(&problem);
    assert_eq!(m.dimensions(), dimensions);
    let initial = Assignment::new(initial);
    let (graph, source) = model::graph(&m);
    let mut director = StreamScoreDirector::new(&fp, graph, source, initial.clone()).unwrap();
    let expected = recompute(&problem, &m.schedule(&initial));
    assert_eq!(director.score(), expected.0);
    // Actual multi-session repeated encounters and large pair joins are required.
    assert!(expected.1 >= 480);
    assert!(expected.2 >= 48);
    if dimensions.0 == 32 {
        assert_eq!(expected, (HardSoftScore::new(0, -3_883_200), 480, 48));
    }
    let mut provider = model::GroupMoves::new(&m, 19);
    for step in 0..24 {
        let before = director.assignment().clone();
        let before_score = director.score();
        let movement = provider.moves(&fp, &before, step).next().unwrap();
        let trial = director.apply(&movement).unwrap();
        assert_eq!(
            trial.score(),
            recompute(&problem, &m.schedule(trial.assignment())).0
        );
        if step % 3 == 0 {
            trial.commit();
        } else {
            drop(trial);
            assert_eq!(director.assignment(), &before);
            assert_eq!(director.score(), before_score);
        }
        director.assert_consistent().unwrap();
    }
}

fn bounded_search(input: &str) {
    let problem: Value = serde_json::from_str(input).unwrap();
    let (m, fp, initial) = model::load(&problem);
    let expected_initial = recompute(&problem, &m.schedule(&Assignment::new(initial.clone()))).0;
    let first = model::solve(&m, &fp, &initial, 31, None, 64);
    let second = model::solve(&m, &fp, &initial, 31, None, 64);
    assert_eq!(first.initial_score, expected_initial);
    assert_eq!(first.result.evaluations, 64);
    assert_eq!(first.result.evaluations, second.result.evaluations);
    assert_eq!(first.result.completed_steps, second.result.completed_steps);
    assert_eq!(first.result.accepted_moves, second.result.accepted_moves);
    assert_eq!(first.result.termination, second.result.termination);
    assert!(first.elapsed_ns > 0);
    let best = first.result.best.unwrap();
    let replay = second.result.best.unwrap();
    assert_eq!(best.assignment, replay.assignment);
    assert_eq!(best.score, replay.score);
    assert!(best.score >= expected_initial);
    fp.validate(&best.assignment).unwrap();
    assert_eq!(
        best.score,
        recompute(&problem, &m.schedule(&best.assignment)).0
    );
    let (graph, source) = model::graph(&m);
    let mut fresh = StreamScoreDirector::new(&fp, graph, source, best.assignment).unwrap();
    assert_eq!(fresh.score(), best.score);
    fresh.assert_consistent().unwrap();
}

#[test]
fn social_golfer_full_score_and_rollback() {
    full_score_and_rollback(GOLFER, (32, 8, 10, 320));
}
#[test]
fn sailing_full_score_and_rollback() {
    full_score_and_rollback(SAILING, (115, 11, 5, 573));
}
#[test]
fn social_golfer_bounded_search() {
    bounded_search(GOLFER);
}
#[test]
fn sailing_bounded_search() {
    bounded_search(SAILING);
}
