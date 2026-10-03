//! Original explicit-group transaction oracle and benchmark; no search or grouping.
use std::{
    io::{self, BufRead},
    time::Instant,
};
use timetuck::*;
fn value(s: &str) -> Option<i32> {
    if s == "-" {
        None
    } else {
        Some(s.parse().expect("integer"))
    }
}
fn indices(s: &str) -> Vec<usize> {
    if s == "-" {
        vec![]
    } else {
        s.split(',').map(|v| v.parse().expect("index")).collect()
    }
}
fn encode(a: &[Option<i32>]) -> String {
    a.iter()
        .map(|v| v.map_or_else(|| "-".to_owned(), |v| v.to_string()))
        .collect::<Vec<_>>()
        .join(",")
}
fn checksum(a: &[Option<i32>]) -> i64 {
    a.iter()
        .enumerate()
        .map(|(i, v)| (i as i64 + 1) * i64::from(v.unwrap_or(7)))
        .sum()
}
fn movement(
    kind: &str,
    columns: &[usize],
    left: &[usize],
    right: &[usize],
    target: Option<i32>,
    a: &Assignment<i32>,
) -> Move<i32> {
    let pillar = |members: &[usize]| {
        Pillar::from_rows(
            members
                .iter()
                .map(|r| columns.iter().map(|c| 3 * r + c).collect())
                .collect(),
        )
        .expect("pillar")
    };
    let l = pillar(left);
    if kind == "c" {
        l.change_move(a, &[target]).expect("change")
    } else {
        l.swap_move(&pillar(right), a).expect("swap")
    }
}
fn main() {
    let args: Vec<usize> = std::env::args()
        .skip(1)
        .map(|s| s.parse().expect("count"))
        .collect();
    assert_eq!(args.len(), 3);
    let (iterations, warmup, samples) = (args[0], args[1], args[2]);
    assert!((1..=1_000_000).contains(&iterations) && samples > 0);
    for line in io::stdin().lock().lines() {
        let line = line.expect("input");
        let f: Vec<_> = line.split('\t').collect();
        assert_eq!(f.len(), 7);
        assert!(matches!(f[1], "c" | "s"));
        let columns = indices(f[2]);
        let left = indices(f[3]);
        let right = indices(f[4]);
        let target = value(f[5]);
        let initial: Vec<_> = f[6]
            .split(';')
            .flat_map(|row| {
                let row: Vec<_> = row.split(',').map(value).collect();
                assert_eq!(row.len(), 3);
                row
            })
            .collect();
        let p = FiniteProblem::new(
            initial
                .iter()
                .map(|v| {
                    let variable = Variable::new((-2..=3).collect()).allowing_unassigned();
                    v.map_or(variable.clone(), |v| variable.with_initial(v))
                })
                .collect(),
        )
        .expect("model");
        let mut expected = initial.clone();
        for &col in &columns {
            for &row in &left {
                expected[3 * row + col] = if f[1] == "c" {
                    target
                } else {
                    initial[3 * right[0] + col]
                };
            }
            if f[1] == "s" {
                for &row in &right {
                    expected[3 * row + col] = initial[3 * left[0] + col];
                }
            }
        }
        for sample in 0..warmup + samples {
            let mut a = p.initial_assignment();
            let mut trial = vec![];
            let mut restored = vec![];
            let mut sum = 0;
            let start = Instant::now();
            for _ in 0..iterations {
                let m = movement(f[1], &columns, &left, &right, target, &a);
                {
                    let guard = p.apply(&mut a, &m).expect("apply");
                    trial = guard.values().to_vec();
                    assert_eq!(trial, expected);
                }
                restored = a.values().to_vec();
                assert_eq!(restored, initial);
                sum += 31 * checksum(&trial) + checksum(&restored);
            }
            let elapsed = start.elapsed().as_nanos();
            let m = movement(f[1], &columns, &left, &right, target, &a);
            p.apply(&mut a, &m).expect("commit").commit();
            assert_eq!(a.values(), expected);
            if sample >= warmup {
                println!(
                    "{}\t{elapsed}\t{sum}\t{}\t{}\t{}",
                    f[0],
                    encode(&trial),
                    encode(&restored),
                    encode(a.values())
                );
            }
        }
    }
}
