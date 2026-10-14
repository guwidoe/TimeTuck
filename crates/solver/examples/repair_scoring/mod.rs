//! Original native repair workload representations; no Java architecture parity.
//! Incremental mode is for the six-field homogeneous required-value protocol:
//! temporarily unassigned variables contribute zero weight and no conflicts.
use super::solver_support::Case;
use timetuck::{ConstraintNetwork, HardSoftScore, ScoringError};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ScoringModel {
    #[default]
    Full,
    Incremental,
    IncrementalCached,
}

impl ScoringModel {
    pub fn parse(value: Option<&str>) -> Result<Self, &'static str> {
        match value {
            None | Some("full") => Ok(Self::Full),
            Some("incremental") => Ok(Self::Incremental),
            Some("incremental-cached") => Ok(Self::IncrementalCached),
            _ => Err("unknown repair scoring model"),
        }
    }
}

pub fn network(
    case: &Case,
    model: ScoringModel,
) -> Result<ConstraintNetwork<'_, usize, HardSoftScore>, ScoringError> {
    let size = case.problem.variables().len();
    let mut network = ConstraintNetwork::new(size);
    match model {
        ScoringModel::Full => {
            network.add_constraint("full-score", (0..size).collect(), |values| {
                case.score_by(|i| values[i].copied())
            })?;
        }
        ScoringModel::Incremental | ScoringModel::IncrementalCached => {
            // Keep every edge occurrence, including duplicate and reversed edges.
            // Self edges have a one-variable scope and conflict iff assigned.
            for (index, &(left, right)) in case.edges.iter().enumerate() {
                if left == right {
                    network.add_constraint(format!("edge-{index}"), vec![left], |values| {
                        HardSoftScore::new(-i64::from(values[0].is_some()), 0)
                    })?;
                } else {
                    network.add_constraint(
                        format!("edge-{index}"),
                        vec![left, right],
                        |values| {
                            HardSoftScore::new(
                                -i64::from(values[0].is_some() && values[0] == values[1]),
                                0,
                            )
                        },
                    )?;
                }
            }
            for (index, weights) in case.weights.iter().enumerate() {
                network.add_constraint(format!("weight-{index}"), vec![index], move |values| {
                    HardSoftScore::new(0, values[0].map_or(0, |&color| weights[color]))
                })?;
            }
        }
    }
    if model == ScoringModel::IncrementalCached {
        // All callbacks above are pure. Cache capacity counts one contribution
        // per (variable, domain occurrence, dependent constraint). Each weight
        // contributes one dependency; every edge occurrence contributes its
        // distinct endpoints, preserving duplicates/reversals and self edges.
        // Only required-basic Dense/Pooled reconstruction consumes this opt-in.
        // Overflow disables the optimization (zero cap), preserving ordinary
        // scoring rather than misreporting a score arithmetic error.
        let mut dependencies = vec![1usize; size];
        let capacity = (|| {
            for &(left, right) in &case.edges {
                dependencies[left] = dependencies[left].checked_add(1)?;
                if left != right {
                    dependencies[right] = dependencies[right].checked_add(1)?;
                }
            }
            case.problem.variables().iter().zip(dependencies).try_fold(
                0usize,
                |total, (variable, count)| {
                    total.checked_add(variable.domain().len().checked_mul(count)?)
                },
            )
        })()
        .unwrap_or(0);
        network = network.with_pooled_contribution_cache(capacity);
    }
    Ok(network)
}
