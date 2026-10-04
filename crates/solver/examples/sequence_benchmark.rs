//! Original public collector update/full-observation benchmark, without solver timing.
use std::{
    io::{self, BufRead},
    time::Instant,
};
use timetuck::*;
type C = SequenceCollector<usize, i64, usize>;
fn numbers(s: &str) -> Vec<i64> {
    s.split(',').map(|v| v.parse().unwrap()).collect()
}
fn encode(s: &[i64]) -> String {
    s.iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(",")
}
fn checksum(s: &[i64]) -> i64 {
    s.iter().enumerate().map(|(i, v)| (i as i64 + 1) * v).sum()
}
fn put(c: &mut C, points: &[i64], old: i64, new: i64) {
    let item = |i: i64| (i as usize, points[i as usize], i as usize);
    match (old >= 0, new >= 0) {
        (true, true) => assert!(c.replace(&item(old), &item(new))),
        (true, false) => assert!(c.remove(&item(old))),
        (false, true) => c.insert(&item(new)),
        (false, false) => {}
    }
}
fn observe(c: &C, pool: usize) -> Vec<i64> {
    let chain = c.finish();
    let mut out =
        Vec::with_capacity(2 + 3 * pool + 8 * chain.sequences().len() + 5 * chain.breaks().len());
    out.extend([chain.sequences().len() as i64, chain.breaks().len() as i64]);
    out.resize(2 + 3 * pool, -1);
    for (q, s) in chain.sequences().iter().enumerate() {
        for item in s.items() {
            let offset = 2 + 3 * item.id;
            assert_eq!(out[offset], -1, "duplicate output identity");
            out[offset..offset + 3].copy_from_slice(&[1, item.index, q as i64]);
        }
        out.extend([
            s.count() as i64,
            i64::try_from(*s.length()).unwrap(),
            s.first_item().index,
            s.last_item().index,
            i64::from(s.is_first()),
            i64::from(s.is_last()),
            s.previous_break_index().map_or(-1, |i| i as i64),
            s.next_break_index().map_or(-1, |i| i as i64),
        ]);
    }
    for b in chain.breaks() {
        out.extend([
            i64::try_from(*b.length()).unwrap(),
            b.previous_sequence_end().index,
            b.next_sequence_start().index,
            i64::from(b.is_first()),
            i64::from(b.is_last()),
        ]);
    }
    out
}
fn main() {
    let args = std::env::args()
        .skip(1)
        .map(|s| s.parse::<usize>().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(args.len(), 3);
    let (iterations, warmups, samples) = (args[0], args[1], args[2]);
    assert!((1..=10000).contains(&iterations) && samples > 0);
    for line in io::stdin().lock().lines() {
        let line = line.unwrap();
        let f = line.split('\t').collect::<Vec<_>>();
        assert_eq!(f.len(), 7);
        let points = numbers(f[1]);
        let slots = numbers(f[2]);
        let slot = f[3].parse::<usize>().unwrap();
        let new = f[4].parse::<i64>().unwrap();
        let old = slots[slot];
        let initial = numbers(f[5]);
        let expected = numbers(f[6]);
        for sample in 0..warmups + samples {
            let mut c = C::default();
            for &i in &slots {
                put(&mut c, &points, -1, i);
            }
            assert_eq!(observe(&c, points.len()), initial);
            let mut trial = vec![];
            let mut restored = vec![];
            let mut sum = 0;
            let start = Instant::now();
            for _ in 0..iterations {
                put(&mut c, &points, old, new);
                trial = observe(&c, points.len());
                assert_eq!(trial, expected);
                put(&mut c, &points, new, old);
                restored = observe(&c, points.len());
                assert_eq!(restored, initial);
                sum += 31 * checksum(&trial) + checksum(&restored);
            }
            let elapsed = start.elapsed().as_nanos();
            put(&mut c, &points, old, new);
            let committed = observe(&c, points.len());
            assert_eq!(committed, expected);
            if sample >= warmups {
                println!(
                    "{}\t{elapsed}\t{sum}\t{}\t{}\t{}",
                    f[0],
                    encode(&trial),
                    encode(&restored),
                    encode(&committed)
                );
            }
        }
    }
}
