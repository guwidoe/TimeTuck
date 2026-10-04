//! Atomic named weight configuration for scoped networks.
use crate::{AdditiveScore, ConstraintWeightOverrides};
use std::{collections::BTreeMap, error::Error, fmt};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConstraintWeightError {
    UnknownConstraint(String),
    UnweightedConstraint(String),
    StructuralWeight(String),
}
impl fmt::Display for ConstraintWeightError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownConstraint(name) => write!(f, "unknown constraint {name:?}"),
            Self::UnweightedConstraint(name) => {
                write!(f, "constraint {name:?} has no configurable weight")
            }
            Self::StructuralWeight(name) => {
                write!(f, "constraint {name:?} has a structural weight component")
            }
        }
    }
}
impl Error for ConstraintWeightError {}

pub(crate) fn resolve_overrides<'a, S: AdditiveScore + 'a>(
    constraints: impl Iterator<Item = (&'a str, Option<&'a S>)>,
    overrides: &ConstraintWeightOverrides<S>,
) -> Result<Vec<(usize, S)>, ConstraintWeightError> {
    let names: BTreeMap<_, _> = constraints
        .enumerate()
        .map(|(i, (name, weight))| (name, (i, weight)))
        .collect();
    overrides
        .iter()
        .map(|(name, weight)| {
            let &(index, current) = names
                .get(name)
                .ok_or_else(|| ConstraintWeightError::UnknownConstraint(name.to_owned()))?;
            if current.is_none() {
                return Err(ConstraintWeightError::UnweightedConstraint(name.to_owned()));
            }
            if weight.structural() != 0 {
                return Err(ConstraintWeightError::StructuralWeight(name.to_owned()));
            }
            Ok((index, weight.clone()))
        })
        .collect()
}
