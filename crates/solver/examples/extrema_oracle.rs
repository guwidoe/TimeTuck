//! Original standalone key-projected extrema comparison.
use std::{
    collections::BTreeMap,
    io::{self, BufRead},
};
use timetuck::*;
fn run<const MAX: bool>(f: &[&str]) {
    let items = f[2]
        .split(';')
        .enumerate()
        .map(|(i, r)| {
            let v = r
                .split(',')
                .map(|s| s.parse::<i64>().unwrap())
                .collect::<Vec<_>>();
            (i, v[0], v[1])
        })
        .collect::<Vec<_>>();
    let mut c = ExtremumCollector::<usize, i64, i64, MAX>::default();
    let mut slots = BTreeMap::new();
    let mut state = 0;
    let emit = |state, c: &ExtremumCollector<usize, i64, i64, MAX>| {
        let value = c.finish().map_or_else(
            || "-".to_owned(),
            |v| format!("{},{},{}", v.id, v.key, v.value),
        );
        println!("{}\t{state}\t{value}", f[0]);
    };
    emit(state, &c);
    for op in f[3].split(';').filter(|s| !s.is_empty()) {
        let v = op.split(':').collect::<Vec<_>>();
        let slot = v[1].parse::<usize>().unwrap();
        let item = v[2].parse::<isize>().unwrap();
        let old = slots.get(&slot).copied().unwrap_or(-1);
        for new in std::iter::once(item).chain((v[0] == "t").then_some(old)) {
            match (slots.get(&slot).copied(), new) {
                (Some(a), b) if b >= 0 => {
                    assert!(c.replace(&items[a as usize], &items[b as usize]))
                }
                (Some(a), _) => assert!(c.remove(&items[a as usize])),
                (None, b) if b >= 0 => c.insert(&items[b as usize]),
                _ => {}
            }
            if new >= 0 {
                slots.insert(slot, new);
            } else {
                slots.remove(&slot);
            }
            state += 1;
            emit(state, &c);
        }
    }
}
fn main() {
    for line in io::stdin().lock().lines() {
        let line = line.unwrap();
        let f = line.split('\t').collect::<Vec<_>>();
        match f[1] {
            "min" => run::<false>(&f),
            "max" => run::<true>(&f),
            _ => panic!("unknown extremum"),
        }
    }
}
