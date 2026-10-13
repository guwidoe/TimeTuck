//! Original default-path performance workload; compatible with the prior node API.
use std::{hint::black_box, time::Instant};
use timetuck::{JoinComparison, PartitionedJoinNode, TupleChange};
fn solve(rounds: usize, batch: bool) -> u64 {
    let mut node = PartitionedJoinNode::new(
        JoinComparison::LessThan,
        |r: &(usize, usize)| *r,
        |r: &(usize, usize)| *r,
    );
    for id in 0..256 {
        node.apply_left(TupleChange::put(id, (id % 32, id % 17)));
        node.apply_right(TupleChange::put(id, (id % 32, id % 17 + 1)));
    }
    let mut checksum = 0_u64;
    for round in 0..rounds {
        let id = round % 256;
        let equal = (round / 7) % 32;
        let key = round % 17;
        let left = TupleChange::put(id, (equal, key));
        let right = TupleChange::put(id, (equal, key + 1));
        if batch {
            checksum += node.apply_batch([left], [right]).len() as u64;
        } else {
            checksum += node.apply_left(left).len() as u64;
            checksum += node.apply_right(right).len() as u64;
        }
        checksum += node.matching_right(&equal, &key).count() as u64;
    }
    checksum + node.matches().count() as u64
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let rounds = args[1].parse().unwrap();
    let warmups = args[2].parse().unwrap();
    let samples = args[3].parse().unwrap();
    let batch = args[4] == "batch";
    for _ in 0..warmups {
        black_box(solve(rounds, batch));
    }
    for _ in 0..samples {
        let start = Instant::now();
        let checksum = black_box(solve(rounds, batch));
        println!("{},{}", start.elapsed().as_nanos(), checksum);
    }
}
