//! Original development protocol for arbitrary-precision collector comparisons.
use std::{
    collections::BTreeMap,
    io::{self, BufRead},
    str::FromStr,
};
use timetuck::*;
fn decimal(text: &str) -> Result<Decimal, Box<dyn std::error::Error>> {
    let (coefficient, scale) = text.split_once(':').ok_or("expected coefficient:scale")?;
    Ok(Decimal::new(BigInt::from_str(coefficient)?, scale.parse()?))
}
fn encode(value: &Decimal) -> String {
    format!("{}:{}", value.coefficient(), value.scale())
}
fn run<V: Clone, C: ReversibleCollector<V>, E>(
    id: &str,
    ops: &str,
    parse: impl Fn(&str) -> Result<V, E>,
    finish: impl Fn(C::Output) -> String,
) -> Result<(), Box<dyn std::error::Error>>
where
    E: Into<Box<dyn std::error::Error>>,
{
    let mut collector = C::default();
    let mut slots = BTreeMap::new();
    println!("{id}\t0\t{}", finish(collector.finish()));
    if ops.is_empty() {
        return Ok(());
    }
    for (i, op) in ops.split(';').enumerate() {
        let f = op.splitn(3, ':').collect::<Vec<_>>();
        let slot: usize = f.get(1).ok_or("missing slot")?.parse()?;
        match f.as_slice() {
            ["p", _, v] => {
                let value = parse(v).map_err(Into::into)?;
                if let Some(old) = slots.get(&slot) {
                    assert!(collector.replace(old, &value));
                } else {
                    collector.insert(&value);
                }
                slots.insert(slot, value);
            }
            ["r", _] => {
                if let Some(old) = slots.remove(&slot) {
                    assert!(collector.remove(&old));
                }
            }
            _ => return Err("invalid operation".into()),
        }
        println!("{id}\t{}\t{}", i + 1, finish(collector.finish()));
    }
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    for line in io::stdin().lock().lines() {
        let line = line?;
        let f = line.split('\t').collect::<Vec<_>>();
        let [id, kind, ops] = f.as_slice() else {
            return Err("expected id, kind, operations".into());
        };
        let average = |v: Option<Decimal>| v.map_or_else(|| "-".into(), |v| encode(&v));
        match *kind {
            "integer_sum" => {
                run::<_, BigIntSumCollector, _>(id, ops, BigInt::from_str, |v| v.to_string())?
            }
            "integer_average" => {
                run::<_, BigIntAverageCollector, _>(id, ops, BigInt::from_str, average)?
            }
            "decimal_sum" => run::<_, DecimalSumCollector, _>(id, ops, decimal, |v| encode(&v))?,
            "decimal_average" => run::<_, DecimalAverageCollector, _>(id, ops, decimal, average)?,
            _ => return Err("unknown kind".into()),
        }
    }
    Ok(())
}
