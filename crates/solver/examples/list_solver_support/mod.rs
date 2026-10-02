//! Original bounded routing protocol for development comparisons; not production API.
use std::error::Error;
use timetuck::{HardSoftScore, ListAssignment, ListEntity, ListProblem};
pub struct Case {
    pub id: String,
    pub problem: ListProblem<usize>,
    pub points: Vec<(i64, i64)>,
    pub depots: Vec<(i64, i64)>,
    pub capacities: Vec<usize>,
}
fn points(text: &str) -> Result<Vec<(i64, i64)>, Box<dyn Error>> {
    if text == "-" {
        return Ok(vec![]);
    }
    text.split(',')
        .map(|s| {
            let (a, b) = s.split_once(':').ok_or("point requires two coordinates")?;
            let (x, y) = (a.parse::<i64>()?, b.parse::<i64>()?);
            if !(-1_000_000..=1_000_000).contains(&x) || !(-1_000_000..=1_000_000).contains(&y) {
                return Err("coordinate bound".into());
            }
            Ok((x, y))
        })
        .collect()
}
impl Case {
    pub fn parse(line: &str) -> Result<Self, Box<dyn Error>> {
        let f: Vec<_> = line.split('\t').collect();
        if f.len() != 7 || f[0].is_empty() {
            return Err("expected seven fields".into());
        }
        let depots = points(f[2])?;
        let points = points(f[1])?;
        let capacities = f[3]
            .split(',')
            .map(str::parse)
            .collect::<Result<Vec<usize>, _>>()?;
        let lists = f[4]
            .split(';')
            .map(|s| {
                if s == "-" {
                    Ok(vec![])
                } else {
                    s.split(',')
                        .map(str::parse)
                        .collect::<Result<Vec<usize>, _>>()
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        let prefixes = f[6]
            .split(',')
            .map(str::parse)
            .collect::<Result<Vec<usize>, _>>()?;
        let m = depots.len();
        if m == 0
            || m > 100
            || points.len() > 1000
            || capacities.iter().any(|&v| v > i32::MAX as usize)
            || capacities.len() != m
            || lists.len() != m
            || prefixes.len() != m
            || f[5].len() != m
            || !f[5].bytes().all(|b| b == b'0' || b == b'1')
        {
            return Err("invalid shape or pins".into());
        }
        let entities = lists
            .into_iter()
            .enumerate()
            .map(|(e, list)| {
                ListEntity::new(list)
                    .with_pinned(f[5].as_bytes()[e] == b'1')
                    .with_pinned_prefix(prefixes[e])
            })
            .collect();
        let problem = ListProblem::new((0..points.len()).collect(), entities)?;
        Ok(Self {
            id: f[0].to_owned(),
            problem,
            points,
            depots,
            capacities,
        })
    }
    pub fn score(&self, a: &ListAssignment<usize>) -> HardSoftScore {
        self.score_lists(a.lists().iter().map(Vec::as_slice))
    }
    pub fn score_lists<'a>(&self, lists: impl Iterator<Item = &'a [usize]>) -> HardSoftScore {
        let mut hard = 0;
        let mut distance = 0;
        for (e, list) in lists.enumerate() {
            hard -= list.len().saturating_sub(self.capacities[e]) as i64;
            let depot = self.depots[e];
            let mut previous = depot;
            for &v in list {
                let point = self.points[v];
                distance += (previous.0 - point.0).abs() + (previous.1 - point.1).abs();
                previous = point;
            }
            distance += (previous.0 - depot.0).abs() + (previous.1 - depot.1).abs();
        }
        HardSoftScore::new(hard, -distance)
    }
    pub fn encode_assignment(a: &ListAssignment<usize>) -> String {
        a.lists()
            .iter()
            .map(|row| {
                if row.is_empty() {
                    "-".to_owned()
                } else {
                    row.iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(",")
                }
            })
            .collect::<Vec<_>>()
            .join(";")
    }
}
