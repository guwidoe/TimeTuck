//! Original public collector comparison; no source-method or timing credit.
use std::{
    collections::BTreeMap,
    io::{self, BufRead},
};
use timetuck::*;
type C<P, M> = ConnectedRangeCollector<usize, P, usize, M>;
type Node<P, M> = CollectNode<usize, (), (usize, P, P, usize), C<P, M>>;
fn emit<P: Copy + Ord + std::fmt::Display, M: RangeMetric<P>>(
    id: &str,
    state: usize,
    n: &Node<P, M>,
) where
    M::Distance: std::fmt::Display,
{
    let c = n
        .groups()
        .next()
        .map_or_else(|| C::<P, M>::default().finish(), |(_, c)| c);
    let optional = |v: Option<usize>| v.map_or_else(|| "-".into(), |v| v.to_string());
    let ranges = c
        .ranges()
        .iter()
        .map(|r| {
            format!(
                "{},{},{},{},{},{},{}:{}",
                r.start(),
                r.end(),
                r.length(),
                r.count(),
                optional(r.minimum_overlap()),
                optional(r.maximum_overlap()),
                u8::from(r.has_overlap()),
                r.items()
                    .iter()
                    .map(|v| v.id.to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            )
        })
        .collect::<Vec<_>>();
    let gaps = c
        .gaps()
        .iter()
        .map(|g| {
            format!(
                "{},{},{}",
                g.previous_range_end(),
                g.next_range_start(),
                g.length()
            )
        })
        .collect::<Vec<_>>();
    println!(
        "{id}\t{state}\t{}\t{}",
        if ranges.is_empty() {
            "-".into()
        } else {
            ranges.join(";")
        },
        if gaps.is_empty() {
            "-".into()
        } else {
            gaps.join(";")
        }
    );
}
fn run<P: Copy + Ord + std::fmt::Display + std::str::FromStr, M: RangeMetric<P>>()
where
    M::Distance: std::fmt::Display,
{
    for line in io::stdin().lock().lines() {
        let line = line.unwrap();
        let f = line.split('\t').collect::<Vec<_>>();
        let intervals = f[1]
            .split(';')
            .map(|r| {
                let v = r
                    .split(',')
                    .map(|v| {
                        v.parse::<P>()
                            .ok()
                            .expect("endpoint outside selected integer domain")
                    })
                    .collect::<Vec<_>>();
                (v[0], v[1])
            })
            .collect::<Vec<_>>();
        let capture = |i: usize| ((), (i, intervals[i].0, intervals[i].1, i));
        let mut n = Node::<P, M>::new();
        let mut slots = BTreeMap::new();
        let mut state = 0;
        emit(f[0], state, &n);
        state += 1;
        for op in f[2].split(';').filter(|s| !s.is_empty()) {
            let a = op.split(':').collect::<Vec<_>>();
            let slot = a[1].parse::<usize>().unwrap();
            let item = a[2].parse::<isize>().unwrap();
            let old = slots.get(&slot).copied();
            if item >= 0 {
                slots.insert(slot, item as usize);
            } else {
                slots.remove(&slot);
            }
            n.apply(TupleChange {
                id: slot,
                value: (item >= 0).then(|| capture(item as usize)),
            });
            emit(f[0], state, &n);
            state += 1;
            if a[0] == "t" {
                n.apply(TupleChange {
                    id: slot,
                    value: old.map(capture),
                });
                if let Some(i) = old {
                    slots.insert(slot, i);
                } else {
                    slots.remove(&slot);
                }
                emit(f[0], state, &n);
                state += 1;
            }
        }
    }
}

fn main() {
    match std::env::args().skip(1).collect::<Vec<_>>().as_slice() {
        [] => run::<i64, IntegerRangeMetric>(),
        [mode] if mode == "--wrapping32" => run::<i32, WrappingIntegerRangeMetric>(),
        _ => panic!("usage: range_oracle [--wrapping32]"),
    }
}
