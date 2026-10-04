//! Original public composition comparison; no generated-case method credit.
use std::{
    collections::BTreeMap,
    io::{self, BufRead},
};
use timetuck::*;
type R = (i64, i64, i64);
type E = Option<Extremum<i64, i64, i64>>;
type Pair = PairCollector<MinByCollector<i64, i64, i64>, MaxByCollector<i64, i64, i64>>;
struct Two;
impl CollectorFinisher<(E, E)> for Two {
    type Output = String;
    fn finish((a, b): (E, E)) -> String {
        format!("{},{}", value(a), value(b))
    }
}
struct Three;
impl CollectorFinisher<((E, E), Option<f64>)> for Three {
    type Output = String;
    fn finish((ab, c): ((E, E), Option<f64>)) -> String {
        format!(
            "{},{}",
            Two::finish(ab),
            c.map_or_else(|| "-".into(), |v| v.to_bits().to_string())
        )
    }
}
struct Four;
impl CollectorFinisher<(usize, ((E, E), Option<f64>))> for Four {
    type Output = String;
    fn finish((n, abc): (usize, ((E, E), Option<f64>))) -> String {
        format!("{n},{}", Three::finish(abc))
    }
}
struct Then;
impl CollectorFinisher<usize> for Then {
    type Output = String;
    fn finish(n: usize) -> String {
        (n * 10).to_string()
    }
}
fn value(v: E) -> String {
    v.map_or_else(|| "-".into(), |v| v.value.to_string())
}
fn pair(v: i64) -> (R, R) {
    ((v, v, v), (v, v, v))
}
fn run<V, C: ReversibleCollector<V, Output = String>>(f: &[&str], map: impl Fn(i64) -> V) {
    let items = f[2]
        .split(',')
        .map(|s| s.parse::<i64>().unwrap())
        .collect::<Vec<_>>();
    let mut c = C::default();
    let mut slots = BTreeMap::<usize, usize>::new();
    let mut state = 0;
    println!("{}\t{state}\t{}", f[0], c.finish());
    for op in f[3].split(';').filter(|s| !s.is_empty()) {
        let v = op.split(':').collect::<Vec<_>>();
        let slot = v[1].parse::<usize>().unwrap();
        let item = v[2].parse::<isize>().unwrap();
        let old = slots.get(&slot).map_or(-1, |v| *v as isize);
        for new in std::iter::once(item).chain((v[0] == "t").then_some(old)) {
            match (slots.get(&slot).copied(), new) {
                (Some(a), b) if b >= 0 => {
                    assert!(c.replace(&map(items[a]), &map(items[b as usize])))
                }
                (Some(a), _) => assert!(c.remove(&map(items[a]))),
                (None, b) if b >= 0 => c.insert(&map(items[b as usize])),
                _ => {}
            }
            if new >= 0 {
                slots.insert(slot, new as usize);
            } else {
                slots.remove(&slot);
            }
            state += 1;
            println!("{}\t{state}\t{}", f[0], c.finish());
        }
    }
}
fn main() {
    for line in io::stdin().lock().lines() {
        let line = line.unwrap();
        let f = line.split('\t').collect::<Vec<_>>();
        match f[1] {
            "two" => run::<_, FinishingCollector<Pair, Two>>(&f, pair),
            "three" => run::<_, FinishingCollector<PairCollector<Pair, AverageCollector>, Three>>(
                &f,
                |v| (pair(v), v),
            ),
            "four" => run::<
                _,
                FinishingCollector<
                    PairCollector<CountCollector, PairCollector<Pair, AverageCollector>>,
                    Four,
                >,
            >(&f, |v| (v, (pair(v), v))),
            "then" => run::<_, FinishingCollector<CountCollector, Then>>(&f, |v| v),
            _ => panic!("unknown mode"),
        }
    }
}
