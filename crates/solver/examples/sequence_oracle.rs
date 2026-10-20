//! Original standalone collector protocol; no upstream method credit or timing claim.
use std::{
    fmt::{Debug, Display},
    io::{self, BufRead},
    str::FromStr,
};
use timetuck::*;
type C<P, M> = SequenceCollector<usize, P, usize, M>;
type Node<P, M> = CollectNode<usize, (), (usize, P, usize), C<P, M>>;
fn emit<P: Ord + Clone, M: SequenceMetric<P>>(id: &str, state: usize, n: &Node<P, M>)
where
    M::Distance: Display,
{
    let c = n
        .groups()
        .next()
        .map_or_else(|| C::<P, M>::default().finish(), |(_, c)| c);
    let optional =
        |i: Option<usize>| i.map_or_else(|| "-".into(), |i| c.breaks()[i].length().to_string());
    let ss = c
        .sequences()
        .iter()
        .map(|s| {
            format!(
                "{},{},{},{},{},{},{},{}:{}",
                s.count(),
                s.length(),
                u8::from(s.is_first()),
                u8::from(s.is_last()),
                optional(s.previous_break_index()),
                optional(s.next_break_index()),
                s.first_item().id,
                s.last_item().id,
                s.items()
                    .iter()
                    .map(|x| x.id.to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            )
        })
        .collect::<Vec<_>>();
    let bs = c
        .breaks()
        .iter()
        .map(|b| {
            format!(
                "{},{},{},{},{}",
                b.length(),
                u8::from(b.is_first()),
                u8::from(b.is_last()),
                b.previous_sequence_end().id,
                b.next_sequence_start().id
            )
        })
        .collect::<Vec<_>>();
    println!(
        "{}\t{}\t{}\t{}",
        id,
        state,
        if ss.is_empty() {
            "-".into()
        } else {
            ss.join(";")
        },
        if bs.is_empty() {
            "-".into()
        } else {
            bs.join(";")
        }
    );
}
fn run<P, M>()
where
    P: Ord + Clone + FromStr,
    P::Err: Debug,
    M: SequenceMetric<P>,
    M::Distance: Display,
{
    for line in io::stdin().lock().lines() {
        let line = line.unwrap();
        let f = line.split('\t').collect::<Vec<_>>();
        let points = f[1]
            .split(',')
            .map(|x| x.parse::<P>().unwrap())
            .collect::<Vec<_>>();
        let mut n = Node::<P, M>::new();
        let mut slots = std::collections::BTreeMap::new();
        let mut state = 0;
        emit(f[0], state, &n);
        state += 1;
        for op in f[2].split(';').filter(|x| !x.is_empty()) {
            let a = op.split(':').collect::<Vec<_>>();
            let slot = a[1].parse::<usize>().unwrap();
            let item = usize::try_from(a[2].parse::<isize>().unwrap()).ok();
            let saved = slots.get(&slot).copied();
            if let Some(item) = item {
                slots.insert(slot, item);
            } else {
                slots.remove(&slot);
            }
            n.apply(TupleChange {
                id: slot,
                value: item.map(|item| ((), (item, points[item].clone(), item))),
            });
            emit(f[0], state, &n);
            state += 1;
            if a[0] == "t" {
                n.apply(TupleChange {
                    id: slot,
                    value: saved.map(|i| ((), (i, points[i].clone(), i))),
                });
                if let Some(i) = saved {
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
    let mut args = std::env::args().skip(1);
    match (args.next().as_deref(), args.next()) {
        (None, None) => run::<i64, IntegerSequenceMetric>(),
        (Some("--wrapping32"), None) => run::<i32, WrappingIntegerSequenceMetric>(),
        _ => panic!("usage: sequence_oracle [--wrapping32]"),
    }
}
