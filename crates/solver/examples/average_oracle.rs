//! Original development protocol; no upstream test-method credit.
use std::{
    collections::BTreeMap,
    io::{self, BufRead},
};
use timetuck::{AverageCollector, ReversibleCollector};
fn emit(id: &str, state: usize, c: &AverageCollector) {
    let value = c
        .finish()
        .map_or_else(|| "-".into(), |v| format!("{:016x}", v.to_bits()));
    println!("{id}\t{state}\t{value}");
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    for line in io::stdin().lock().lines() {
        let line = line?;
        let (id, operations) = line
            .split_once('\t')
            .ok_or("expected case and operations")?;
        let mut c = AverageCollector::default();
        let mut slots = BTreeMap::new();
        emit(id, 0, &c);
        if operations.is_empty() {
            continue;
        }
        for (step, operation) in operations.split(';').enumerate() {
            let fields = operation.split(':').collect::<Vec<_>>();
            let slot: usize = fields.get(1).ok_or("missing slot")?.parse()?;
            let result = match fields.as_slice() {
                ["p", _, value] => {
                    let value = value.parse()?;
                    let result = if let Some(&old) = slots.get(&slot) {
                        c.try_replace(old, value).map(|found| assert!(found))
                    } else {
                        c.try_insert(value)
                    };
                    if result.is_ok() {
                        slots.insert(slot, value);
                    }
                    result
                }
                ["r", _] => {
                    let result = if let Some(&old) = slots.get(&slot) {
                        c.try_remove(old).map(|found| assert!(found))
                    } else {
                        Ok(())
                    };
                    if result.is_ok() {
                        slots.remove(&slot);
                    }
                    result
                }
                _ => return Err("invalid average operation".into()),
            };
            if result.is_err() {
                println!("{id}\t{}\tOVERFLOW", step + 1);
                break;
            }
            emit(id, step + 1, &c);
        }
    }
    Ok(())
}
