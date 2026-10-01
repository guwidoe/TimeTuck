//! Original matched microbenchmark. See compat/oracle/ScoreBenchmark.java.
use std::{hint::black_box, time::Instant};
use timetuck::{HardMediumSoftScore, HardSoftScore, SimpleScore};

fn run(kind: &str, iterations: u64, seed: i64) -> i64 {
    let mut x = seed;
    macro_rules! workload {
        ($t:ty, $delta:expr) => {{
            let mut score = <$t>::ZERO;
            for _ in 0..iterations {
                x = x
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                let delta = $delta(x);
                score = score + delta;
                if score > <$t>::ZERO {
                    score = score - delta;
                }
            }
            score.levels().into_iter().fold(x, |sum, level| sum ^ level)
        }};
    }
    match kind {
        "simple" => workload!(SimpleScore, |x: i64| SimpleScore::new((x >> 32) % 101)),
        "hard_soft" => workload!(HardSoftScore, |x: i64| HardSoftScore::new(
            (x >> 32) % 101,
            x % 101
        )),
        "hard_medium_soft" => workload!(HardMediumSoftScore, |x: i64| HardMediumSoftScore::new(
            (x >> 32) % 101,
            (x >> 16) % 101,
            x % 101
        )),
        _ => panic!("unknown workload"),
    }
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let iterations = args[2].parse().expect("iterations");
    let seed = args[3].parse().expect("seed");
    let warmup: usize = args[4].parse().expect("warmup");
    let samples: usize = args[5].parse().expect("samples");
    for _ in 0..warmup {
        black_box(run(&args[1], black_box(iterations), black_box(seed)));
    }
    for _ in 0..samples {
        let start = Instant::now();
        let checksum = black_box(run(&args[1], black_box(iterations), black_box(seed)));
        let elapsed = start.elapsed().as_nanos();
        println!("{elapsed}\t{checksum}");
    }
}
