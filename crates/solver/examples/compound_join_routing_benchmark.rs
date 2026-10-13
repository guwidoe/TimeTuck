//! Original native before/after workload, compatible with the prior conjunction API.
use std::{hint::black_box, time::Instant};
use timetuck::*;
type Row = (usize, i64, i64);
fn matches(mode: &str, left: &Row, right: &Row) -> bool {
    match mode {
        "empty" => true,
        "ordered" => left.1 < right.1,
        "double" => left.1 < right.1 && left.2 > right.2,
        "equality" => left.0 == right.0 && left.1 < right.1,
        _ => panic!("unknown mode"),
    }
}
fn solve(mode: &str, rows: usize, rounds: usize) -> (i64, u128) {
    let group = JoinProjection::new(|r: &Row| r.0);
    let age = JoinProjection::new(|r: &Row| r.1);
    let suffix = JoinProjection::new(|r: &Row| r.2);
    let clauses = match mode {
        "empty" => vec![],
        "ordered" => vec![JoinCondition::new(JoinComparison::LessThan, &age, &age)],
        "double" => vec![
            JoinCondition::new(JoinComparison::LessThan, &age, &age),
            JoinCondition::new(JoinComparison::GreaterThan, &suffix, &suffix),
        ],
        "equality" => vec![
            JoinCondition::new(JoinComparison::Equal, &group, &group),
            JoinCondition::new(JoinComparison::LessThan, &age, &age),
        ],
        _ => panic!("unknown mode"),
    };
    let conditions = JoinConditions::new(clauses);
    let mut builder = StreamGraphBuilder::<SimpleScore>::new();
    let left = builder.source::<usize, Row>();
    let right = builder.source::<usize, Row>();
    let joined = conditions
        .apply(&mut builder, left.port(), right.port())
        .unwrap();
    builder
        .score("count", &joined, |_| SimpleScore::new(1))
        .unwrap();
    let mut graph = builder.build();
    let mut l = vec![None; rows];
    let mut r = vec![None; rows];
    let mut tx = graph.transaction();
    for id in 0..rows {
        let a = (id % 8, id as i64, (rows - id) as i64);
        let b = (id % 8, id as i64 + 1, (rows - id + 1) as i64);
        l[id] = Some(a);
        r[id] = Some(b);
        tx.apply(&left, TupleChange::put(id, a)).unwrap();
        tx.apply(&right, TupleChange::put(id, b)).unwrap();
    }
    tx.commit().unwrap();
    let mut checksum = 0_i64;
    let mut evaluations = 0_u128;
    for round in 0..rounds {
        let id = round % rows;
        let value = (!round.is_multiple_of(17)).then_some((
            round % 8,
            (round % (rows * 2)) as i64,
            ((round * 7) % (rows * 2)) as i64,
        ));
        let (source, values) = if round % 2 == 0 {
            (&left, &mut l)
        } else {
            (&right, &mut r)
        };
        values[id] = value;
        let mut tx = graph.transaction();
        tx.apply(source, TupleChange { id, value }).unwrap();
        checksum += tx.score().unwrap().score();
        evaluations += tx.score_evaluations();
        tx.commit().unwrap();
    }
    let expected = l
        .iter()
        .flatten()
        .flat_map(|a| r.iter().flatten().map(move |b| (a, b)))
        .filter(|(a, b)| matches(mode, a, b))
        .count();
    assert_eq!(graph.score(), SimpleScore::new(expected as i64));
    (checksum, evaluations)
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let mode = &args[1];
    let rows = args[2].parse().unwrap();
    let rounds = args[3].parse().unwrap();
    let warmups = args[4].parse().unwrap();
    let samples = args[5].parse().unwrap();
    for _ in 0..warmups {
        black_box(solve(mode, rows, rounds));
    }
    for _ in 0..samples {
        let start = Instant::now();
        let (checksum, evaluations) = black_box(solve(mode, rows, rounds));
        println!("{},{checksum},{evaluations}", start.elapsed().as_nanos());
    }
}
