//! Line protocol adapter for development-only differential tests.
use std::{
    fmt::Display,
    io::{self, BufRead},
    ops::{Add, Neg, Sub},
    str::FromStr,
};
use timetuck::{HardMediumSoftScore, HardSoftScore, SimpleScore};

trait Oracle:
    Sized + Ord + Display + FromStr + Add<Output = Self> + Sub<Output = Self> + Neg<Output = Self>
{
    fn multiply(self, x: f64) -> Self;
    fn divide(self, x: f64) -> Self;
    fn power(self, x: f64) -> Self;
    fn abs(self) -> Self;
    fn describe(&self) -> String;
}
macro_rules! oracle {
    ($t:ty) => {
        impl Oracle for $t {
            fn multiply(self, x: f64) -> Self {
                self.multiply(x)
            }
            fn divide(self, x: f64) -> Self {
                self.divide(x)
            }
            fn power(self, x: f64) -> Self {
                self.power(x)
            }
            fn abs(self) -> Self {
                self.abs()
            }
            fn describe(&self) -> String {
                format!(
                    "{}\t{}\t{}\t{}",
                    self,
                    self.to_short_string(),
                    self.is_feasible(),
                    self.structural()
                )
            }
        }
    };
}
oracle!(SimpleScore);
oracle!(HardSoftScore);
oracle!(HardMediumSoftScore);

fn evaluate<T: Oracle>(fields: &[&str]) -> Result<String, ()> {
    let a: T = fields[2].parse().map_err(|_| ())?;
    let result = match fields[1] {
        "parse" => a,
        "add" => a + fields[3].parse().map_err(|_| ())?,
        "sub" => a - fields[3].parse().map_err(|_| ())?,
        "neg" => -a,
        "abs" => a.abs(),
        "mul" => a.multiply(fields[3].parse().map_err(|_| ())?),
        "div" => a.divide(fields[3].parse().map_err(|_| ())?),
        "pow" => a.power(fields[3].parse().map_err(|_| ())?),
        "cmp" => {
            return Ok(format!(
                "{}",
                match a.cmp(&fields[3].parse().map_err(|_| ())?) {
                    std::cmp::Ordering::Less => -1,
                    std::cmp::Ordering::Equal => 0,
                    std::cmp::Ordering::Greater => 1,
                }
            ));
        }
        _ => return Err(()),
    };
    Ok(result.describe())
}
fn main() {
    for line in io::stdin().lock().lines() {
        let line = line.expect("read protocol line");
        let fields: Vec<_> = line.split('\t').collect();
        let result = if fields.len() < 4 {
            Err(())
        } else {
            match fields[0] {
                "simple" => evaluate::<SimpleScore>(&fields),
                "hard_soft" => evaluate::<HardSoftScore>(&fields),
                "hard_medium_soft" => evaluate::<HardMediumSoftScore>(&fields),
                _ => Err(()),
            }
        };
        println!("{}", result.unwrap_or_else(|_| "ERROR".to_owned()));
    }
}
