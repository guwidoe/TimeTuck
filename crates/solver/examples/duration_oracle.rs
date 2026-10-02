//! Original public duration-collector protocol; no upstream-method credit.
use std::{
    collections::BTreeMap,
    io::{self, BufRead},
};
use timetuck::*;
enum Collector {
    Sum(DurationSumCollector),
    Average(DurationAverageCollector),
}
impl Collector {
    fn update(
        &mut self,
        old: Option<SignedDuration>,
        new: Option<SignedDuration>,
    ) -> Result<(), DurationOverflow> {
        macro_rules! apply {
            ($c:expr) => {
                match (old, new) {
                    (Some(a), Some(b)) => $c.try_replace(a, b).map(|found| assert!(found)),
                    (None, Some(b)) => $c.try_insert(b),
                    (Some(a), None) => $c.try_remove(a).map(|found| assert!(found)),
                    (None, None) => Ok(()),
                }
            };
        }
        match self {
            Self::Sum(c) => apply!(c),
            Self::Average(c) => apply!(c),
        }
    }
    fn finish(&self) -> Result<Option<SignedDuration>, DurationOverflow> {
        match self {
            Self::Sum(c) => Ok(Some(c.finish())),
            Self::Average(c) => c.try_finish(),
        }
    }
}
fn emit(id: &str, state: usize, value: Option<SignedDuration>) {
    println!(
        "{id}\t{state}\t{}",
        value.map_or_else(|| "-".into(), |v| v.as_nanos().to_string())
    );
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    for line in io::stdin().lock().lines() {
        let line = line?;
        let fields = line.split('\t').collect::<Vec<_>>();
        let [id, kind, operations] = fields.as_slice() else {
            return Err("expected id, kind, operations".into());
        };
        let mut c = match *kind {
            "sum" => Collector::Sum(Default::default()),
            "average" => Collector::Average(Default::default()),
            _ => return Err("unknown kind".into()),
        };
        let mut slots = BTreeMap::new();
        emit(id, 0, c.finish()?);
        if operations.is_empty() {
            continue;
        }
        for (i, op) in operations.split(';').enumerate() {
            let fields = op.split(':').collect::<Vec<_>>();
            let slot: usize = fields.get(1).ok_or("missing slot")?.parse()?;
            let value = match fields.as_slice() {
                ["p", _, value] => Some(SignedDuration::from_nanos(value.parse()?)?),
                ["r", _] => None,
                _ => return Err("invalid operation".into()),
            };
            if c.update(slots.get(&slot).copied(), value).is_err() {
                println!("{id}\t{}\tOVERFLOW:update", i + 1);
                break;
            }
            if let Some(v) = value {
                slots.insert(slot, v);
            } else {
                slots.remove(&slot);
            }
            match c.finish() {
                Ok(value) => emit(id, i + 1, value),
                Err(_) => {
                    println!("{id}\t{}\tOVERFLOW:finish", i + 1);
                    break;
                }
            }
        }
    }
    Ok(())
}
