//! Original shadow-transaction measurements using attributed concurrent fixtures.
// Only the cached evaluator and Visit are needed by this driver.
#[allow(dead_code)]
mod concurrent_support;
use concurrent_support::{Visit, cache::CachedSchedule};
use std::{
    io::{self, BufRead},
    time::Instant,
};
use timetuck::*;
fn ids(s: &str) -> Vec<usize> {
    if s == "_" {
        vec![]
    } else {
        s.split(',').map(|v| v.parse().unwrap()).collect()
    }
}
fn state(rows: &[Visit]) -> Vec<Option<i64>> {
    rows.iter()
        .flat_map(|v| {
            [
                v.owner.map(|x| x as i64),
                v.index.map(|x| x as i64),
                v.previous.map(|x| x as i64),
                v.next.map(|x| x as i64),
                v.cascading,
                v.ready,
                v.start,
                v.finish,
                Some(i64::from(v.inconsistent)),
            ]
        })
        .collect()
}
fn parse_state(s: &str) -> Vec<Option<i64>> {
    if s == "_" {
        vec![]
    } else {
        s.split(',')
            .map(|v| {
                if v == "-" {
                    None
                } else {
                    Some(v.parse().unwrap())
                }
            })
            .collect()
    }
}
fn encode(s: &[Option<i64>]) -> String {
    s.iter()
        .map(|v| v.map_or_else(|| "-".into(), |v| v.to_string()))
        .collect::<Vec<_>>()
        .join(",")
}
fn checksum(s: &[Option<i64>]) -> i64 {
    s.iter()
        .enumerate()
        .map(|(i, v)| (i as i64 + 1) * v.unwrap_or(-1))
        .sum()
}
fn movement(s: &str) -> ListMove<usize> {
    let f: Vec<_> = s.split(',').collect();
    assert_eq!(f.len(), 5);
    let a = ListPosition {
        entity: f[1].parse().unwrap(),
        index: f[2].parse().unwrap(),
    };
    let b = ListPosition {
        entity: f[3].parse().unwrap(),
        index: f[4].parse().unwrap(),
    };
    match f[0] {
        "c" => ListMove::Change {
            source: a,
            destination: b,
        },
        "s" => ListMove::Swap { left: a, right: b },
        _ => panic!("move"),
    }
}
fn main() {
    let args: Vec<usize> = std::env::args()
        .skip(1)
        .map(|s| s.parse().unwrap())
        .collect();
    assert_eq!(args.len(), 3);
    let (iterations, warmup, samples) = (args[0], args[1], args[2]);
    assert!((1..=10_000).contains(&iterations) && samples > 0);
    for line in io::stdin().lock().lines() {
        let line = line.unwrap();
        let f: Vec<_> = line.split('\t').collect();
        assert_eq!(f.len(), 7);
        let n: usize = f[1].parse().unwrap();
        assert!((2..=64).contains(&n));
        let groups = if f[2] == "_" {
            vec![]
        } else {
            f[2].split(';').map(ids).collect()
        };
        let lists: Vec<_> = f[3].split(';').map(ids).collect();
        let p = ListProblem::new(
            (0..n).collect(),
            lists.into_iter().map(ListEntity::new).collect(),
        )
        .unwrap();
        let expected_initial = parse_state(f[5]);
        let expected_trial = parse_state(f[6]);
        assert_eq!(expected_initial.len(), 9 * n);
        assert_eq!(expected_trial.len(), 9 * n);
        for sample in 0..warmup + samples {
            let mut a = p.initial_assignment();
            let mut cache = CachedSchedule::new(&groups, n);
            let mut rows = cache.project(a.lists()).0;
            assert_eq!(state(&rows), expected_initial);
            let mut trial = vec![];
            let mut restored = vec![];
            let mut sum = 0;
            let start = Instant::now();
            for _ in 0..iterations {
                let m = movement(f[4]);
                let saved = cache.clone();
                let saved_rows = rows.clone();
                {
                    let t = p.apply(&mut a, &m).unwrap();
                    rows = cache.project(t.lists()).0;
                    trial = state(&rows);
                    assert_eq!(trial, expected_trial);
                }
                cache = saved;
                rows = saved_rows;
                restored = state(&rows);
                assert_eq!(restored, expected_initial);
                // Validate restored ownership, not only restored cached fields.
                for (e, list) in a.lists().iter().enumerate() {
                    for (i, &v) in list.iter().enumerate() {
                        assert_eq!((rows[v].owner, rows[v].index), (Some(e), Some(i)));
                    }
                }
                sum += 31 * checksum(&trial) + checksum(&restored);
            }
            let elapsed = start.elapsed().as_nanos();
            p.apply(&mut a, &movement(f[4])).unwrap().commit();
            let committed = state(&cache.project(a.lists()).0);
            assert_eq!(committed, expected_trial);
            if sample >= warmup {
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
