//! Read actual solution-owned constraint weights before graph initialization.
use crate::{
    ComposedPlanningSolution, ConstraintWeightOverrides, PlanningType, SolutionMemberKind,
};
use std::sync::Arc;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SolutionWeightError {
    SolutionType {
        expected: PlanningType,
        actual: PlanningType,
    },
    MissingDeclaration,
    ScoreType {
        expected: PlanningType,
        actual: PlanningType,
    },
}
impl std::fmt::Display for SolutionWeightError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "solution weights: {self:?}")
    }
}
impl std::error::Error for SolutionWeightError {}
type WeightRead<'a, P, S> = dyn Fn(&P) -> ConstraintWeightOverrides<S> + 'a;
/// Explicit typed getters replace reflection. A read owns its weight map, so
/// later source edits cannot alter an already initialized graph's configuration.
pub struct SolutionWeightAccessor<'a, P, S> {
    layout: Arc<ComposedPlanningSolution>,
    read: Box<WeightRead<'a, P, S>>,
}
pub struct BoundSolutionWeights<S> {
    layout: Arc<ComposedPlanningSolution>,
    weights: ConstraintWeightOverrides<S>,
}
impl<S> BoundSolutionWeights<S> {
    pub fn descriptor(&self) -> &Arc<ComposedPlanningSolution> {
        &self.layout
    }
    pub fn weights(&self) -> &ConstraintWeightOverrides<S> {
        &self.weights
    }
}
impl<'a, P: 'static, S: 'static> SolutionWeightAccessor<'a, P, S> {
    pub fn new(
        layout: Arc<ComposedPlanningSolution>,
        read: impl Fn(&P) -> ConstraintWeightOverrides<S> + 'a,
    ) -> Result<Self, SolutionWeightError> {
        let expected = layout.schema().solution_type();
        let actual = PlanningType::of::<P>();
        if expected != actual {
            return Err(SolutionWeightError::SolutionType { expected, actual });
        }
        let expected = layout
            .members()
            .iter()
            .find_map(|m| match m.kind() {
                SolutionMemberKind::ConstraintWeights(t) => Some(*t),
                _ => None,
            })
            .ok_or(SolutionWeightError::MissingDeclaration)?;
        let actual = PlanningType::of::<S>();
        if expected != actual {
            return Err(SolutionWeightError::ScoreType { expected, actual });
        }
        Ok(Self {
            layout,
            read: Box::new(read),
        })
    }
    /// Read a nullable solution-owned override field. None means no overrides:
    /// declared graph defaults remain effective, including in an empty-fact
    /// constraint assertion. Metadata validation finishes before any getter call.
    /// The owned result captures this read; later solution edits do not change it.
    pub fn new_nullable(
        layout: Arc<ComposedPlanningSolution>,
        read: impl Fn(&P) -> Option<ConstraintWeightOverrides<S>> + 'a,
    ) -> Result<Self, SolutionWeightError> {
        Self::new(layout, move |solution| read(solution).unwrap_or_default())
    }

    pub fn read(&self, solution: &P) -> BoundSolutionWeights<S> {
        BoundSolutionWeights {
            layout: self.layout.clone(),
            weights: (self.read)(solution),
        }
    }
}
