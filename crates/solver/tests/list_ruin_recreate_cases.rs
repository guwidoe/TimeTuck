// Adapted from pinned ListRuinRecreateMoveSelectorTest (four methods).
// Apache-2.0; see compat/provenance.json for method and fixture attribution.
use timetuck::*;
#[test]
fn ruining_only_one_assigned_value() {
    let p = ListProblem::new(vec![0, 1, 2], vec![ListEntity::new(vec![0, 1, 2])])
        .unwrap()
        .allowing_unassigned();
    let mut n = ListConstraintNetwork::new(1, 3);
    n.add_constraint("Only One Assigned", vec![ListScope::Entity(0)], |v| {
        let ListFact::Entity(list) = v[0] else {
            unreachable!()
        };
        SimpleScore::new(-i64::from(list.len() != 1))
    })
    .unwrap();
    let c = RuinRecreateConfig {
        minimum_ruined: 3,
        max_attempts: 1,
        assert_scores: true,
        ..Default::default()
    };
    let a = ruin_recreate_list_search_from(&p, &n, p.initial_assignment(), c.clone()).unwrap();
    let b = ruin_recreate_list_stream_search_from(
        &p,
        || {
            let mut b = StreamGraphBuilder::new();
            let s = ListStreamSources::new(&mut b);
            b.score("Only One Assigned", s.entities.port(), |(_, list)| {
                SimpleScore::new(-i64::from(list.len() != 1))
            })?;
            Ok((b.build(), s))
        },
        p.initial_assignment(),
        c,
    )
    .unwrap();
    for r in [a, b] {
        assert_eq!(r.best.unwrap().assignment.list(0).unwrap().len(), 1);
    }
}

// Adapted test scoring, generated names and 10-value/3-entity fixture.
fn java_string_hash(s: &str) -> i32 {
    s.encode_utf16()
        .fold(0_i32, |h, c| h.wrapping_mul(31).wrapping_add(c as i32))
}
fn hash_penalty(owner: usize, index: usize, value: usize) -> i64 {
    let fields = [
        java_string_hash(&format!("Generated Entity {owner}")),
        index as i32,
        java_string_hash(&format!("Generated Value {value}")),
    ];
    fields
        .into_iter()
        .fold(1_i32, |h, f| h.wrapping_mul(31).wrapping_add(f))
        .wrapping_abs()
        .wrapping_shr(8) as i64
}
fn fixture_score(lists: &[Vec<usize>], optional: bool) -> SimpleScore {
    if !optional {
        return SimpleScore::new(
            -lists
                .iter()
                .enumerate()
                .map(|(e, row)| {
                    row.iter()
                        .enumerate()
                        .map(|(i, &v)| hash_penalty(e, i, v))
                        .sum::<i64>()
                })
                .sum::<i64>(),
        );
    }
    let sizes = lists
        .iter()
        .map(|l| (l.len() * l.len()) as i64)
        .sum::<i64>();
    let unassigned = 10 - lists.iter().map(Vec::len).sum::<usize>();
    let shared_indices = (0..10)
        .map(|i| {
            let n = lists.iter().filter(|l| l.len() > i).count() as i64;
            n * (n - 1) / 2
        })
        .sum::<i64>();
    SimpleScore::new(-sizes - 5 * unassigned as i64 - shared_indices)
}
fn fixture_solve(
    optional: bool,
    stream: bool,
    mut observe: impl FnMut(SimpleScore),
) -> SolverResult<ListAssignment<usize>, SimpleScore> {
    let p = ListProblem::new((0..10).collect(), vec![ListEntity::new(vec![]); 3]).unwrap();
    let p = if optional { p.allowing_unassigned() } else { p };
    let c = SolverSequence {
        phases: vec![
            SolverPhase::Construction(ConstructionConfig {
                max_evaluations: u64::MAX,
                assert_scores: true,
                ..Default::default()
            }),
            SolverPhase::RuinRecreate(RuinRecreateConfig {
                seed: 20261224,
                max_attempts: if optional { 100 } else { u64::MAX },
                move_weights: if optional {
                    RepairMoveWeights {
                        change: 2,
                        swap: 2,
                        repair: 1,
                    }
                } else {
                    RepairMoveWeights::default()
                },
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
        solve_list_stream_with_observer(
            &p,
            || {
                let mut b = StreamGraphBuilder::new();
                let s = ListStreamSources::<usize>::new(&mut b);
                if optional {
                    b.score("Minimize entity list size", s.entities.port(), |(_, l)| {
                        SimpleScore::new(-((l.len() * l.len()) as i64))
                    })?;
                    b.score("Minimize unassigned values", s.values.port(), |v| {
                        SimpleScore::new(if v.position.is_none() { -5 } else { 0 })
                    })?;
                    let assigned = b.filter(s.values.port(), |v| v.position.is_some())?;
                    let pairs = b.join_equal(
                        &assigned,
                        &assigned,
                        |v| v.position.unwrap().index,
                        |v| v.position.unwrap().index,
                    )?;
                    let unique = b.filter(&pairs, |(a, b)| a.value < b.value)?;
                    b.score("Maximize different indices", &unique, |_| {
                        SimpleScore::new(-1)
                    })?;
                } else {
                    b.score("Hash constraint", s.values.port(), |v| {
                        SimpleScore::new(
                            v.position
                                .map_or(0, |p| -hash_penalty(p.entity, p.index, v.value)),
                        )
                    })?;
                }
                Ok((b.build(), s))
            },
            c,
            |u| observe(u.score),
        )
        .unwrap()
    } else {
        let mut n = ListConstraintNetwork::new(3, 10);
        n.add_constraint(
            "Fixture",
            (0..3).map(ListScope::Entity).collect(),
            |facts| {
                let lists: Vec<_> = facts
                    .iter()
                    .map(|f| {
                        let ListFact::Entity(l) = f else {
                            unreachable!()
                        };
                        l.to_vec()
                    })
                    .collect();
                fixture_score(&lists, optional)
            },
        )
        .unwrap();
        solve_list_with_observer(&p, &n, c, |u| observe(u.score)).unwrap()
    };
    p.validate(&result.assignment).unwrap();
    assert!(result.complete);
    assert_eq!(
        result.score,
        Some(fixture_score(result.assignment.lists(), optional))
    );
    result
}
#[test]
fn ruining() {
    for stream in [false, true] {
        let result = fixture_solve(false, stream, |_| {});
        assert_eq!(result.termination, SolverTermination::UnimprovedTimeLimit);
    }
}
#[test]
fn ruining_with_metric() {
    for stream in [false, true] {
        let mut events = Vec::new();
        let result = fixture_solve(false, stream, |s| events.push(s));
        assert!(!events.is_empty());
        let scores = result.work.evaluations;
        let moves = result.work.repair_attempts;
        assert!(scores > 0);
        assert!(moves > 0);
        assert!(scores > moves);
    }
}
#[test]
fn ruining_allows_unassigned_values() {
    for stream in [false, true] {
        let result = fixture_solve(true, stream, |_| {});
        assert_eq!(
            result.work.repair_attempts + result.work.fine_move_attempts,
            100
        );
        assert!(result.work.repair_attempts > 0 && result.work.fine_move_attempts > 0);
    }
}

#[test]
fn hash_fixture_matches_public_java_oracle_for_every_position() {
    let rows: Vec<_> = include_str!("fixtures/list-repair-hashes.csv")
        .lines()
        .collect();
    assert_eq!(rows.len(), 300);
    for (row, text) in rows.iter().enumerate() {
        let fields: Vec<i64> = text.split(',').map(|v| v.parse().unwrap()).collect();
        let (owner, index, value) = (row / 100, row / 10 % 10, row % 10);
        assert_eq!(fields[..3], [owner as i64, index as i64, value as i64]);
        assert_eq!(hash_penalty(owner, index, value), fields[3]);
    }
}
