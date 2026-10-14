//! Original required/optional list repair comparison using full score callbacks.
use std::{
    error::Error,
    hint::black_box,
    io::{self, BufRead},
    time::Instant,
};
use timetuck::*;
struct Case {
    id: String,
    problem: ListProblem<usize>,
    caps: Vec<usize>,
    costs: Vec<Vec<i64>>,
    missing: Vec<i64>,
}
impl Case {
    fn parse(line: &str) -> Result<Self, Box<dyn Error>> {
        let fields: Vec<_> = line.split('\t').collect();
        if fields.len() != 5 || fields[0].is_empty() || !["0", "1"].contains(&fields[1]) {
            return Err("invalid shape".into());
        }
        fn numbers(s: &str) -> Result<Vec<i64>, Box<dyn Error>> {
            Ok(s.split(',').map(str::parse).collect::<Result<_, _>>()?)
        }
        let caps = numbers(fields[2])?;
        let costs = fields[3]
            .split(';')
            .map(numbers)
            .collect::<Result<Vec<_>, _>>()?;
        let missing = numbers(fields[4])?;
        if !(2..=4).contains(&caps.len())
            || !(2..=96).contains(&costs.len())
            || missing.len() != costs.len()
            || caps[0] != 0
            || caps.iter().any(|c| !(0..=96).contains(c))
        {
            return Err("invalid sizes".into());
        }
        for (row, null) in costs.iter().zip(&missing) {
            if row.len() != caps.len()
                || *null < 0
                || *null > row[0]
                || row.iter().any(|v| !(0..=10000).contains(v) || *v > row[0])
            {
                return Err("initial costs must be worst".into());
            }
        }
        let mut lists = vec![ListEntity::new(vec![]); caps.len()];
        lists[0] = ListEntity::new((0..costs.len()).collect());
        let p = ListProblem::new((0..costs.len()).collect(), lists)?;
        Ok(Self {
            id: fields[0].into(),
            problem: if fields[1] == "1" {
                p.allowing_unassigned()
            } else {
                p
            },
            caps: caps.into_iter().map(|c| c as usize).collect(),
            costs,
            missing,
        })
    }
    fn score<'a>(&self, lists: impl IntoIterator<Item = &'a [usize]>) -> HardSoftScore {
        let mut cost = 0;
        let mut seen = vec![false; self.costs.len()];
        for (e, row) in lists.into_iter().enumerate() {
            let excess = row.len().saturating_sub(self.caps[e]) as i64;
            cost += excess * excess;
            for &v in row {
                seen[v] = true;
                cost += self.costs[v][e];
            }
            cost += 2 * row.windows(2).filter(|w| w[0] < w[1]).count() as i64;
        }
        cost += seen
            .iter()
            .zip(&self.missing)
            .filter(|(seen, _)| !**seen)
            .map(|(_, cost)| cost)
            .sum::<i64>();
        HardSoftScore::new(0, -cost)
    }
}
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().collect();
    if !(5..=7).contains(&args.len()) {
        return Err("group steps warmup samples [minimum [independent|coverage-cycle]]".into());
    }
    let group: usize = args[1].parse()?;
    let minimum: usize = if args.len() >= 6 { args[5].parse()? } else { 1 };
    let ruin_sampling = match args.get(6).map(String::as_str) {
        None | Some("independent") => RuinSampling::Independent,
        Some("coverage-cycle") => RuinSampling::CoverageCycle,
        _ => return Err("unknown repair sampling policy".into()),
    };
    let steps: u64 = args[2].parse()?;
    let warmup: usize = args[3].parse()?;
    let samples: usize = args[4].parse()?;
    if minimum == 0
        || minimum > group
        || group == 0
        || steps == 0
        || steps >= i32::MAX as u64
        || samples == 0
        || warmup
            .checked_add(samples)
            .is_none_or(|n| n > i32::MAX as usize)
    {
        return Err("invalid counts".into());
    }
    for line in io::stdin().lock().lines() {
        let case = Case::parse(&line?)?;
        if group > case.costs.len() {
            return Err("group too large".into());
        }
        let mut network = ListConstraintNetwork::new(case.caps.len(), case.costs.len());
        network.add_constraint(
            "full score",
            (0..case.caps.len()).map(ListScope::Entity).collect(),
            |facts| {
                case.score(facts.iter().map(|f| match f {
                    ListFact::Entity(row) => *row,
                    _ => unreachable!(),
                }))
            },
        )?;
        let initial = case.problem.initial_assignment();
        let initial_score = case.score(initial.lists().iter().map(Vec::as_slice));
        let config = RuinRecreateConfig {
            seed: 20261225,
            ruin_sampling,
            minimum_ruined: minimum,
            maximum_ruined: group,
            max_attempts: steps,
            max_evaluations: u64::MAX,
            acceptance: Acceptance::StepCountingHillClimbing {
                period: steps + 1,
                mode: StepCountingMode::Step,
            },
            ..Default::default()
        };
        for run in 0..warmup + samples {
            let start = Instant::now();
            let r = ruin_recreate_list_search_from(
                black_box(&case.problem),
                &network,
                initial.clone(),
                config.clone(),
            )?;
            if r.attempts > steps || r.complete_reconstructions != r.attempts {
                return Err("incomplete repair count".into());
            }
            let best = r.best.ok_or("missing best")?;
            case.problem.validate(&best.assignment)?;
            if !case.problem.is_complete(&best.assignment)
                || case.score(best.assignment.lists().iter().map(Vec::as_slice)) != best.score
                || best.score < initial_score
            {
                return Err("invalid returned solution".into());
            }
            for (owner, row) in best.assignment.lists().iter().enumerate() {
                for (index, value) in row.iter().enumerate() {
                    if best.assignment.element(value).map(|v| v.position)
                        != Some(ListPosition {
                            entity: owner,
                            index,
                        })
                    {
                        return Err("invalid returned metadata".into());
                    }
                }
            }
            let elapsed = start.elapsed().as_nanos();
            if run >= warmup {
                let rows = best
                    .assignment
                    .lists()
                    .iter()
                    .map(|row| {
                        if row.is_empty() {
                            "-".into()
                        } else {
                            row.iter()
                                .map(usize::to_string)
                                .collect::<Vec<_>>()
                                .join(",")
                        }
                    })
                    .collect::<Vec<String>>()
                    .join(";");
                println!(
                    "{}\t{elapsed}\t{}\t{}\t{rows}\t{}\t{}",
                    case.id,
                    best.score.hard(),
                    best.score.soft(),
                    r.constraint_evaluations,
                    r.attempts
                );
            }
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn optional_score_retains_missing_costs_ordering_and_overflow() {
        let case = Case::parse("check\t1\t0,1\t20,0;20,9;20,1\t5,3,5").unwrap();
        assert_eq!(
            case.score([&[][..], &[2, 0][..]]),
            HardSoftScore::new(0, -5)
        );
        assert_eq!(
            case.score([&[][..], &[0, 2][..]]),
            HardSoftScore::new(0, -7)
        );
        assert_eq!(case.score([&[][..], &[][..]]), HardSoftScore::new(0, -13));
        assert_eq!(
            case.score([&[0, 1, 2][..], &[][..]]),
            HardSoftScore::new(0, -73)
        );
    }
    #[test]
    fn protocol_rejects_invalid_dimensions_costs_and_optionality() {
        for line in [
            "x\t2\t0,1\t20,0;20,1\t5,3",
            "x\t1\t1,1\t20,0;20,1\t5,3",
            "x\t1\t0,1\t20;20,1\t5,3",
            "x\t1\t0,1\t20,0;20,21\t5,3",
            "x\t1\t0,1\t20,0;20,1\t5,21",
        ] {
            assert!(Case::parse(line).is_err(), "{line}");
        }
    }
}
