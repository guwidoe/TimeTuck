//! Original public load balance comparison; no source-method or timing credit.
use std::{
    collections::BTreeMap,
    io::{self, BufRead},
};
use timetuck::*;
type Node<C> = CollectNode<usize, (), (i32, i64, i64), C>;
trait Observation {
    fn emit(&self, id: &str, state: usize);
}
impl Observation for LoadBalance<i32> {
    fn emit(&self, id: &str, state: usize) {
        emit_values(id, state, self.loads(), self.unfairness());
    }
}
impl Observation for Result<WrappingLoadBalance<i32>, WrappingLoadBalanceError> {
    fn emit(&self, id: &str, state: usize) {
        match self {
            Ok(value) => emit_values(id, state, value.loads(), value.unfairness()),
            Err(WrappingLoadBalanceError::NegativeRadicand) => {
                println!("{id}\t{state}\tERROR\tNegativeRadicand");
            }
        }
    }
}
fn emit<C>(id: &str, state: usize, n: &Node<C>)
where
    C: ReversibleCollector<(i32, i64, i64)> + Default,
    C::Output: Observation,
{
    let c = n
        .groups()
        .next()
        .map_or_else(|| C::default().finish(), |(_, c)| c);
    c.emit(id, state);
}
fn emit_values<V: std::fmt::Display>(
    id: &str,
    state: usize,
    loads: &SnapshotMap<i32, V>,
    u: &Decimal,
) {
    let loads = loads
        .iter()
        .map(|(k, v)| format!("{k}:{v}"))
        .collect::<Vec<_>>();
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
fn run<C>()
where
    C: ReversibleCollector<(i32, i64, i64)> + Default,
    C::Output: Observation,
{
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
        let mut n = Node::<C>::new();
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
        [] => run::<LoadBalanceCollector<i32>>(),
        [mode] if mode == "--wrapping64-moments" => run::<WrappingLoadBalanceCollector<i32>>(),
        _ => panic!("expected no argument or --wrapping64-moments"),
    }
}
