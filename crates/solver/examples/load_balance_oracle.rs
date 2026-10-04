//! Original public load balance comparison; no source-method or timing credit.
use std::{
    collections::BTreeMap,
    io::{self, BufRead},
};
use timetuck::*;
type C = LoadBalanceCollector<i32>;
type Node = CollectNode<usize, (), (i32, i64, i64), C>;
fn emit(id: &str, state: usize, n: &Node) {
    let c = n
        .groups()
        .next()
        .map_or_else(|| C::default().finish(), |(_, c)| c);
    let loads = c
        .loads()
        .iter()
        .map(|(k, v)| format!("{k}:{v}"))
        .collect::<Vec<_>>();
    let u = c.unfairness();
    println!(
        "{id}\t{state}\t{}\t{}\t{}",
        if loads.is_empty() {
            "-".into()
        } else {
            loads.join(";")
        },
        u.coefficient(),
        u.scale()
    );
}
fn main() {
    for line in io::stdin().lock().lines() {
        let line = line.unwrap();
        let f = line.split('\t').collect::<Vec<_>>();
        let items = f[1]
            .split(';')
            .map(|r| {
                let v = r
                    .split(',')
                    .map(|v| v.parse::<i64>().unwrap())
                    .collect::<Vec<_>>();
                (i32::try_from(v[0]).unwrap(), v[1], v[2])
            })
            .collect::<Vec<_>>();
        let capture = |i: usize| ((), items[i]);
        let mut n = Node::new();
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
