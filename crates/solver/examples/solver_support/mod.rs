//! Original protocol adapter shared by development solver comparisons.
use std::error::Error;
use timetuck::{Assignment, FiniteProblem, HardSoftScore, Variable};

pub struct Case {
    pub id: String,
    pub problem: FiniteProblem<usize>,
    edges: Vec<(usize, usize)>,
    weights: Vec<Vec<i64>>,
}
impl Case {
    pub fn parse(line: &str) -> Result<Self, Box<dyn Error>> {
        let fields: Vec<_> = line.split('\t').collect();
        if fields.len() != 6 {
            return Err("expected six tab-separated fields".into());
        }
        let count: usize = fields[1].parse()?;
        let initial: Vec<Option<usize>> = fields[2]
            .split(',')
            .map(|s| {
                if s == "-" {
                    Ok(None)
                } else {
                    s.parse().map(Some)
                }
            })
            .collect::<Result<_, _>>()?;
        if count == 0
            || fields[3].len() != initial.len()
            || !fields[3].bytes().all(|b| b == b'0' || b == b'1')
        {
            return Err("invalid domain size or pins".into());
        }
        let variables = initial
            .iter()
            .zip(fields[3].bytes())
            .map(|(value, pin)| {
                let mut variable = Variable::new((0..count).collect()).with_pinned(pin == b'1');
                if pin == b'1'
                    && let Some(value) = value
                {
                    variable = variable.with_initial(*value);
                }
                variable
            })
            .collect();
        let mut edges = Vec::new();
        if !fields[4].is_empty() {
            for edge in fields[4].split(',') {
                let (a, b) = edge.split_once(':').ok_or("expected edge endpoints")?;
                let pair = (a.parse::<usize>()?, b.parse::<usize>()?);
                if pair.0 >= initial.len() || pair.1 >= initial.len() {
                    return Err("invalid edge".into());
                }
                edges.push(pair);
            }
        }
        let flat: Vec<i64> = fields[5]
            .split(',')
            .map(str::parse)
            .collect::<Result<_, _>>()?;
        if flat.len() != initial.len().checked_mul(count).ok_or("too many weights")? {
            return Err("weight count mismatch".into());
        }
        Ok(Self {
            id: fields[0].to_owned(),
            problem: FiniteProblem::new(variables)?,
            edges,
            weights: flat.chunks_exact(count).map(<[i64]>::to_vec).collect(),
        })
    }
    pub fn score(&self, state: &Assignment<usize>) -> HardSoftScore {
        let hard = -(self
            .edges
            .iter()
            .filter(|&&(a, b)| {
                state.values()[a].is_some() && state.values()[a] == state.values()[b]
            })
            .count() as i64);
        let soft = state
            .values()
            .iter()
            .zip(&self.weights)
            .map(|(value, weights)| value.map_or(0, |value| weights[value]))
            .sum();
        HardSoftScore::new(hard, soft)
    }
}
