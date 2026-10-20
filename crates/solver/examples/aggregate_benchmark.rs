//! Original matched collector update/restore and full-observation benchmark.
use std::{
    io::{self, BufRead},
    time::Instant,
};
use timetuck::*;
struct Wrap32;
impl CollectorFinisher<IntSummary> for Wrap32 {
    type Output = i32;
    fn finish(s: IntSummary) -> i32 {
        s.sum as i32
    }
}
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
fn run<V, C: ReversibleCollector<V>>(
    f: &[&str],
    args: &[usize],
    map: impl Fn((i64, i64)) -> V,
    observe: impl Fn(C::Output) -> Vec<i64>,
) {
    let items = f[2]
        .split(';')
        .map(|r| {
            let v = numbers(r);
            (v[0], v[1])
        })
        .collect::<Vec<_>>();
    let slots = numbers(f[3]);
    let selected = f[4].parse::<usize>().unwrap();
    let new = f[5].parse::<i64>().unwrap();
    let old = slots[selected];
    let initial = numbers(f[6]);
    let expected = numbers(f[7]);
    let put = |c: &mut C, old: i64, new: i64| match (old >= 0, new >= 0) {
        (true, true) => assert!(c.replace(&map(items[old as usize]), &map(items[new as usize]))),
        (true, false) => assert!(c.remove(&map(items[old as usize]))),
        (false, true) => c.insert(&map(items[new as usize])),
        _ => {}
    };
    let (iterations, warmups, samples) = (args[0], args[1], args[2]);
    for sample in 0..warmups + samples {
        let mut c = C::default();
        for &i in &slots {
            put(&mut c, -1, i);
        }
        assert_eq!(observe(c.finish()), initial);
        let mut trial = vec![];
        let mut restored = vec![];
        let mut total = 0;
        let start = Instant::now();
        for _ in 0..iterations {
            put(&mut c, old, new);
            trial = observe(c.finish());
            assert_eq!(trial, expected);
            put(&mut c, new, old);
            restored = observe(c.finish());
            assert_eq!(restored, initial);
            total += 31 * checksum(&trial) + checksum(&restored);
        }
        let elapsed = start.elapsed().as_nanos();
        put(&mut c, old, new);
        let committed = observe(c.finish());
        assert_eq!(committed, expected);
        if sample >= warmups {
            println!(
                "{}\t{elapsed}\t{total}\t{}\t{}\t{}",
                f[0],
                encode(&trial),
                encode(&restored),
                encode(&committed)
            );
        }
    }
}
fn main() {
    let args = std::env::args()
        .skip(1)
        .map(|s| s.parse::<usize>().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(args.len(), 3);
    assert!((1..=10000).contains(&args[0]) && args[2] > 0);
    for line in io::stdin().lock().lines() {
        let line = line.unwrap();
        let f = line.split('\t').collect::<Vec<_>>();
        assert_eq!(f.len(), 8);
        match f[1] {
            "sum" => run::<_, SumCollector>(&f, &args, |r| r.1, |v| vec![v]),
            "set" => run::<_, SortedSetCollector<i64>>(
                &f,
                &args,
                |r| r.1,
                |s| {
                    std::iter::once(s.len() as i64)
                        .chain(s.iter().copied())
                        .collect()
                },
            ),
            "map" => run::<_, MapCollector<i64, i64, FinishingCollector<IntCollector, Wrap32>>>(
                &f,
                &args,
                |r| r,
                |m| {
                    std::iter::once(m.len() as i64)
                        .chain(m.iter().flat_map(|(&k, &v)| [k, i64::from(v)]))
                        .collect()
                },
            ),
            _ => panic!("unknown mode"),
        }
    }
}
