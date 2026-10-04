//! Original bounded forager diagnostic, not a performance workload.
use std::{
    error::Error,
    io::{self, BufRead},
};
use timetuck::*;
fn main() -> Result<(), Box<dyn Error>> {
    for line in io::stdin().lock().lines() {
        let line = line?;
        let f: Vec<_> = line.split('\t').collect();
        if f.len() != 4 {
            return Err("expected four fields".into());
        }
        let ranking = match f[1] {
            "highest" => FinalistRanking::HighestScore,
            "last" => FinalistRanking::StrategicOscillationLastStep,
            "best" => FinalistRanking::StrategicOscillationBest,
            _ => return Err("unknown ranking".into()),
        };
        let steps: u64 = f[2].parse()?;
        if !(1..=4).contains(&steps) {
            return Err("bounded steps required".into());
        }
        let mut scores = vec![];
        for row in f[3].split(';') {
            let a: Vec<i64> = row.split(',').map(str::parse).collect::<Result<_, _>>()?;
            if a.len() != 3 || a.iter().any(|v| !(-8..=8).contains(v)) {
                return Err("bounded triple required".into());
            }
            scores.push(HardMediumSoftScore::new(a[0], a[1], a[2]));
        }
        if !(2..=16).contains(&scores.len()) {
            return Err("bounded value count required".into());
        }
        let (mut current, mut best) = (0, 0);
        for _ in 0..steps {
            let mut forager = MoveForager::new(ForagerConfig {
                ranking,
                current_score: scores[current],
                best_score: scores[best],
                ..Default::default()
            });
            for (i, s) in scores.iter().enumerate() {
                if i != current {
                    forager.add(*s, true, i);
                }
            }
            current = forager.pick().ok_or("missing selected step")?;
            if scores[current] > scores[best] {
                best = current;
            }
        }
        let a = scores[current];
        let b = scores[best];
        println!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            f[0],
            a.hard(),
            a.medium(),
            a.soft(),
            b.hard(),
            b.medium(),
            b.soft(),
            best,
            scores.len() - 1,
            scores.len() - 1
        );
    }
    Ok(())
}
