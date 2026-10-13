//! Original native sampling workload, compatible with the prior cursor factories.
use std::{hint::black_box, time::Instant};
use timetuck::*;
fn sample<I: Iterator<Item = usize>>(cursor: I, limit: usize) -> (u64, u64) {
    cursor
        .take(limit)
        .fold((0_u64, 0_u64), |(hash, sum), value| {
            (
                hash.wrapping_mul(6364136223846793005) ^ value as u64,
                sum.wrapping_add(value as u64),
            )
        })
}
fn solve(mode: &str, length: usize, draws: usize) -> u64 {
    let values: Vec<_> = (0..length).collect();
    let mut checksum = 0_u64;
    match mode {
        "range-repeat" => {
            let range = EnumeratedValueRange::new(values);
            sample(range.random_iter(123), draws).0
        }
        "bucket-repeat" | "bucket-unique" => {
            let mut index = CandidateIndex::new();
            for value in values {
                index.insert(value, (), value).unwrap();
            }
            let bucket = index.bucket(&());
            if mode == "bucket-repeat" {
                return sample(bucket.repeating(123).map(|v| v.value), draws).0;
            }
            for seed in 0..draws.div_ceil(length) {
                let (hash, sum) = sample(bucket.unique(seed as u64).map(|v| v.value), length);
                assert_eq!(sum, length as u64 * (length - 1) as u64 / 2);
                checksum = checksum.wrapping_add(hash);
            }
            checksum
        }
        "concat-repeat" | "concat-unique" => {
            let mut query = LazyConcat::new();
            query.push_slice(&values).unwrap();
            if mode == "concat-repeat" {
                return sample(
                    repeating_candidates(&query, 123).map(|v| *v.unwrap()),
                    draws,
                )
                .0;
            }
            for seed in 0..draws.div_ceil(length) {
                let (hash, sum) = sample(query.samples(seed as u64).map(|v| *v.unwrap()), length);
                assert_eq!(sum, length as u64 * (length - 1) as u64 / 2);
                checksum = checksum.wrapping_add(hash);
            }
            checksum
        }
        _ => panic!("unknown sampling mode"),
    }
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let mode = &args[1];
    let length = args[2].parse().unwrap();
    let draws = args[3].parse().unwrap();
    let warmups = args[4].parse().unwrap();
    let samples = args[5].parse().unwrap();
    for _ in 0..warmups {
        black_box(solve(mode, length, draws));
    }
    for _ in 0..samples {
        let start = Instant::now();
        let checksum = black_box(solve(mode, length, draws));
        println!("{},{checksum}", start.elapsed().as_nanos());
    }
}
