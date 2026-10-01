//! Original protocol adapter shared by development solver comparisons.
use std::error::Error;
use timetuck::{Assignment, FiniteProblem, HardSoftScore, Variable};

pub struct Case {
    pub id: String,
    pub problem: FiniteProblem<usize>,
    edges: Vec<(usize, usize)>,
    weights: Vec<Vec<i64>>,
    unassigned_weights: Vec<i64>,
}
impl Case {
    pub fn parse(line: &str, retain_initial: bool) -> Result<Self, Box<dyn Error>> {
        let fields: Vec<_> = line.split('\t').collect();
        if fields.len() != 6 && fields.len() != 9 {
            return Err("expected six or nine tab-separated fields".into());
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
        let extended = fields.len() == 9;
        let domains: Vec<Vec<usize>> = if extended {
            fields[6]
                .split(';')
                .map(|s| {
                    if s.is_empty() {
                        Ok(Vec::new())
                    } else {
                        s.split(':').map(str::parse).collect::<Result<Vec<_>, _>>()
                    }
                })
                .collect::<Result<_, _>>()?
        } else {
            vec![(0..count).collect(); initial.len()]
        };
        let optional = if extended {
            fields[7].as_bytes().to_vec()
        } else {
            vec![b'0'; initial.len()]
        };
        let unassigned_weights: Vec<i64> = if extended {
            fields[8]
                .split(',')
                .map(str::parse)
                .collect::<Result<_, _>>()?
        } else {
            vec![0; initial.len()]
        };
        if domains.len() != initial.len()
            || optional.len() != initial.len()
            || unassigned_weights.len() != initial.len()
            || !optional.iter().all(|b| *b == b'0' || *b == b'1')
            || domains.iter().flatten().any(|&v| v >= count)
        {
            return Err("invalid extended domain metadata".into());
        }
        let mut variables = Vec::with_capacity(initial.len());
        for (i, domain) in domains.into_iter().enumerate() {
            if initial[i].is_some_and(|v| !domain.contains(&v)) {
                return Err("initial value outside domain".into());
            }
            let pin = fields[3].as_bytes()[i] == b'1';
            let mut variable = Variable::new(domain).with_pinned(pin);
            if optional[i] == b'1' {
                variable = variable.allowing_unassigned();
            }
            if (retain_initial || pin)
                && let Some(value) = initial[i]
            {
                variable = variable.with_initial(value);
            }
            variables.push(variable);
        }
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
            unassigned_weights,
            weights: flat.chunks_exact(count).map(<[i64]>::to_vec).collect(),
        })
    }
    pub fn score(&self, state: &Assignment<usize>) -> HardSoftScore {
        self.score_by(|i| state.values()[i])
    }
    pub fn score_by(&self, value: impl Fn(usize) -> Option<usize>) -> HardSoftScore {
        let hard = -(self
            .edges
            .iter()
            .filter(|&&(a, b)| value(a).is_some() && value(a) == value(b))
            .count() as i64);
        let soft = self
            .weights
            .iter()
            .enumerate()
            .map(|(i, weights)| value(i).map_or(self.unassigned_weights[i], |value| weights[value]))
            .sum();
        HardSoftScore::new(hard, soft)
    }
}
