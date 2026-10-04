//! Original standalone collector protocol; no upstream method credit or timing claim.
use std::io::{self, BufRead};
use timetuck::*;
type C = SequenceCollector<usize, i64, usize>;
type Node = CollectNode<usize, (), (usize, i64, usize), C>;
fn emit(id: &str, state: usize, n: &Node) {
    let c = n
        .groups()
        .next()
        .map_or_else(|| C::default().finish(), |(_, c)| c);
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
fn main() {
    for line in io::stdin().lock().lines() {
        let line = line.unwrap();
        let f = line.split('\t').collect::<Vec<_>>();
        let points = f[1]
            .split(',')
            .map(|x| x.parse::<i64>().unwrap())
            .collect::<Vec<_>>();
        let mut n = Node::new();
        let mut slots = std::collections::BTreeMap::new();
        let mut state = 0;
        emit(f[0], state, &n);
        state += 1;
        for op in f[2].split(';').filter(|x| !x.is_empty()) {
            let a = op.split(':').collect::<Vec<_>>();
            let slot = a[1].parse::<usize>().unwrap();
            let item = a[2].parse::<isize>().unwrap();
            let saved = slots.get(&slot).copied();
            if item >= 0 {
                slots.insert(slot, item as usize);
            } else {
                slots.remove(&slot);
            }
            n.apply(TupleChange {
                id: slot,
                value: (item >= 0)
                    .then(|| ((), (item as usize, points[item as usize], item as usize))),
            });
            emit(f[0], state, &n);
            state += 1;
            if a[0] == "t" {
                n.apply(TupleChange {
                    id: slot,
                    value: saved.map(|i| ((), (i, points[i], i))),
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
