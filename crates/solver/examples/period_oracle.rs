//! Original public period collector comparison. No upstream-method/timing credit.
use std::{
    collections::BTreeMap,
    io::{self, BufRead},
};
use timetuck::*;
fn emit(id: &str, state: usize, c: &PeriodSumCollector) {
    let [y, m, d] = c.finish().components();
    println!("{id}\t{state}\t{y},{m},{d}");
}
fn put(
    c: &mut PeriodSumCollector,
    slots: &mut BTreeMap<usize, usize>,
    items: &[CalendarPeriod],
    slot: usize,
    new: Option<usize>,
) -> Result<(), PeriodOverflow> {
    match (slots.get(&slot).copied(), new) {
        (Some(a), Some(b)) => assert!(c.try_replace(items[a], items[b])?),
        (Some(a), None) => assert!(c.try_remove(items[a])?),
        (None, Some(b)) => c.try_insert(items[b])?,
        (None, None) => {}
    }
    if let Some(i) = new {
        slots.insert(slot, i);
    } else {
        slots.remove(&slot);
    }
    Ok(())
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
                    .map(|s| s.parse::<i32>().unwrap())
                    .collect::<Vec<_>>();
                CalendarPeriod::new(v[0], v[1], v[2])
            })
            .collect::<Vec<_>>();
        let mut c = PeriodSumCollector::default();
        let mut slots = BTreeMap::new();
        let mut state = 0;
        emit(f[0], state, &c);
        state += 1;
        for op in f[2].split(';').filter(|o| !o.is_empty()) {
            let v = op.split(':').collect::<Vec<_>>();
            let slot = v[1].parse::<usize>().unwrap();
            let item = v[2].parse::<isize>().unwrap();
            let old = slots.get(&slot).copied();
            if put(
                &mut c,
                &mut slots,
                &items,
                slot,
                (item >= 0).then_some(item as usize),
            )
            .is_err()
            {
                println!("{}\t{state}\tOVERFLOW", f[0]);
                break;
            }
            emit(f[0], state, &c);
            state += 1;
            if v[0] == "t" {
                if put(&mut c, &mut slots, &items, slot, old).is_err() {
                    println!("{}\t{state}\tOVERFLOW", f[0]);
                    break;
                }
                emit(f[0], state, &c);
                state += 1;
            }
        }
    }
}
