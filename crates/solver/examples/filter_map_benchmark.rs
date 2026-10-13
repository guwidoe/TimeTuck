//! Original native comparison of fused projection and separate filter/map nodes.
use std::{hint::black_box, time::Instant};
use timetuck::{SimpleScore, StreamBatch, StreamGraphBuilder, TupleChange};
type Row = [i64; 64];
fn accepts(row: &Row, accept_mod: i64) -> bool {
    row[0] % 4 < accept_mod
}
fn project(row: &Row) -> i64 {
    row[0] * 2 + row[63]
}
fn solve(rows: usize, rounds: usize, accept_mod: i64, fused: bool, batch: bool) -> (i64, u128) {
    let mut builder = StreamGraphBuilder::<SimpleScore>::new();
    let source = builder.source::<usize, Row>();
    let output = if fused {
        builder
            .filter_map(source.port(), move |row| {
                accepts(row, accept_mod).then(|| project(row))
            })
            .unwrap()
    } else {
        let filtered = builder
            .filter(source.port(), move |row| accepts(row, accept_mod))
            .unwrap();
        builder.map(&filtered, project).unwrap()
    };
    builder
        .score("sum", &output, |value| SimpleScore::new(*value))
        .unwrap();
    let mut graph = builder.build();
    let mut values = vec![None; rows];
    let mut tx = graph.transaction();
    for (id, value) in values.iter_mut().enumerate() {
        let row = [id as i64; 64];
        *value = Some(row);
        tx.apply(&source, TupleChange::put(id, row)).unwrap();
    }
    tx.commit().unwrap();
    let mut checksum = 0_i64;
    let mut evaluations = 0_u128;
    for round in 0..rounds {
        let mut tx = graph.transaction();
        let mut changes = StreamBatch::new();
        for offset in 0..4 {
            let id = (round + offset * 7) % rows;
            let value =
                (!(round + offset).is_multiple_of(13)).then_some([(round * 3 + offset) as i64; 64]);
            values[id] = value;
            let change = TupleChange { id, value };
            if batch {
                changes.push(&source, change).unwrap();
            } else {
                tx.apply(&source, change).unwrap();
            }
        }
        if batch {
            tx.apply_batch(changes).unwrap();
        }
        checksum = checksum.wrapping_add(tx.score().unwrap().score());
        evaluations += tx.score_evaluations();
        tx.commit().unwrap();
    }
    let expected: i64 = values
        .iter()
        .flatten()
        .filter(|row| accepts(row, accept_mod))
        .map(project)
        .sum();
    assert_eq!(graph.score(), SimpleScore::new(expected));
    (checksum, evaluations)
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let fused = args[1] == "fused";
    let batch = args[2] == "batch";
    let rows = args[3].parse().unwrap();
    let accept_mod = args[4].parse().unwrap();
    let rounds = args[5].parse().unwrap();
    let warmups = args[6].parse().unwrap();
    let samples = args[7].parse().unwrap();
    for _ in 0..warmups {
        black_box(solve(rows, rounds, accept_mod, fused, batch));
    }
    for _ in 0..samples {
        let start = Instant::now();
        let (checksum, evaluations) = black_box(solve(rows, rounds, accept_mod, fused, batch));
        println!("{},{checksum},{evaluations}", start.elapsed().as_nanos());
    }
}
