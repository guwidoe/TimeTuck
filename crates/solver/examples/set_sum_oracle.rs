//! Original public set/map/checked-sum comparison.
use std::{
    collections::BTreeMap,
    io::{self, BufRead},
};
use timetuck::*;
struct Wrap32;
impl CollectorFinisher<IntSummary> for Wrap32 {
    type Output = i32;
    fn finish(s: IntSummary) -> i32 {
        s.sum as i32
    }
}
fn ordinary<V, C: ReversibleCollector<V>>(
    c: &mut C,
    old: Option<V>,
    new: Option<V>,
) -> Result<(), SumOverflow> {
    match (old, new) {
        (Some(a), Some(b)) => assert!(c.replace(&a, &b)),
        (Some(a), None) => assert!(c.remove(&a)),
        (None, Some(b)) => c.insert(&b),
        _ => {}
    }
    Ok(())
}
fn sum(c: &mut SumCollector, old: Option<i64>, new: Option<i64>) -> Result<(), SumOverflow> {
    match (old, new) {
        (Some(a), Some(b)) => assert!(c.try_replace(a, b)?),
        (Some(a), None) => assert!(c.try_remove(a)?),
        (None, Some(b)) => c.try_insert(b)?,
        _ => {}
    }
    Ok(())
}
fn run<V, C: ReversibleCollector<V>>(
    f: &[&str],
    map: impl Fn((i64, i64)) -> V,
    put: impl Fn(&mut C, Option<V>, Option<V>) -> Result<(), SumOverflow>,
    format: impl Fn(C::Output) -> String,
) {
    let items = f[2]
        .split(';')
        .map(|s| {
            let v = s
                .split(',')
                .map(|s| s.parse::<i64>().unwrap())
                .collect::<Vec<_>>();
            (v[0], v[1])
        })
        .collect::<Vec<_>>();
    let mut c = C::default();
    let mut slots = BTreeMap::<usize, usize>::new();
    let mut state = 0;
    println!("{}\t{state}\t{}", f[0], format(c.finish()));
    for op in f[3].split(';').filter(|s| !s.is_empty()) {
        let v = op.split(':').collect::<Vec<_>>();
        let slot = v[1].parse::<usize>().unwrap();
        let item = v[2].parse::<isize>().unwrap();
        let old = slots.get(&slot).map_or(-1, |v| *v as isize);
        for new in std::iter::once(item).chain((v[0] == "t").then_some(old)) {
            state += 1;
            let a = slots.get(&slot).map(|i| map(items[*i]));
            let b = (new >= 0).then(|| map(items[new as usize]));
            if put(&mut c, a, b).is_err() {
                println!("{}\t{state}\tOVERFLOW", f[0]);
                return;
            }
            if new >= 0 {
                slots.insert(slot, new as usize);
            } else {
                slots.remove(&slot);
            }
            println!("{}\t{state}\t{}", f[0], format(c.finish()));
        }
    }
}
fn main() {
    for line in io::stdin().lock().lines() {
        let line = line.unwrap();
        let f = line.split('\t').collect::<Vec<_>>();
        match f[1] {
            "set" | "sorted" => run::<_, SetCollector<i64>>(
                &f,
                |r| r.1,
                ordinary,
                |s| {
                    if s.is_empty() {
                        "-".into()
                    } else {
                        s.keys()
                            .map(ToString::to_string)
                            .collect::<Vec<_>>()
                            .join(",")
                    }
                },
            ),
            "map" => run::<_, MapCollector<i64, i64, FinishingCollector<IntCollector, Wrap32>>>(
                &f,
                |r| r,
                ordinary,
                |m| {
                    if m.is_empty() {
                        "-".into()
                    } else {
                        m.iter()
                            .map(|(k, v)| format!("{k}:{v}"))
                            .collect::<Vec<_>>()
                            .join(",")
                    }
                },
            ),
            "sum" => run::<_, SumCollector>(&f, |r| r.1, sum, |v| v.to_string()),
            _ => panic!("unknown mode"),
        }
    }
}
