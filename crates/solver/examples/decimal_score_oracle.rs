//! Original development adapter for public Java/Python decimal score comparisons.
use std::io::{self, BufRead};
use timetuck::*;
fn run(line: &str) -> Option<String> {
    let p: Vec<_> = line.split('\t').collect();
    if p.len() != 4 {
        return None;
    }
    macro_rules! score {
        ($t:ty) => {{
            let a = p[2].parse::<$t>().ok()?;
            if p[1] == "cmp" {
                let b = p[3].parse::<$t>().ok()?;
                let cmp = match a.cmp(&b) {
                    std::cmp::Ordering::Less => -1,
                    std::cmp::Ordering::Equal => 0,
                    std::cmp::Ordering::Greater => 1,
                };
                return Some(format!("{cmp}|{}", a == b));
            }
            let r = match p[1] {
                "parse" => a,
                "add" => a + p[3].parse::<$t>().ok()?,
                "sub" => a - p[3].parse::<$t>().ok()?,
                "neg" => -a,
                "abs" => a.abs(),
                "mul" => a.multiply(&p[3].parse::<Decimal>().ok()?).ok()?,
                "div" => a.divide(&p[3].parse::<Decimal>().ok()?).ok()?,
                "pow" => a.power(p[3].parse::<u32>().ok()?).ok()?,
                _ => return None,
            };
            let levels = r
                .levels()
                .iter()
                .map(|d| format!("{}@{}", d.coefficient(), d.scale()))
                .collect::<Vec<_>>()
                .join(",");
            Some(format!(
                "{}|{}|{}|{}|{}",
                r.structural(),
                r,
                r.to_short_string(),
                r.is_feasible(),
                levels
            ))
        }};
    }
    match p[0] {
        "simple" => score!(SimpleDecimalScore),
        "hard_soft" => score!(HardSoftDecimalScore),
        "hard_medium_soft" => score!(HardMediumSoftDecimalScore),
        _ => None,
    }
}
fn main() {
    for line in io::stdin().lock().lines() {
        println!("{}", run(&line.unwrap()).unwrap_or_else(|| "ERROR".into()));
    }
}
