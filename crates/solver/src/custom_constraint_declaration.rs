//! Executable custom justification configuration retained by a constraint declaration.
use crate::{
    AdditiveScore, ConstraintExplanation, GraphError, StreamConstraint, StreamGraph,
    StreamGraphBuilder, StreamPort,
};
type Factor<'a, T> = dyn Fn(&T) -> i64 + 'a;
type Justify<'a, T, S, J> = dyn Fn(&T, &S) -> J + 'a;
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConstraintDeclarationError {
    MultipleJustifications,
    MissingJustification,
    Graph(GraphError),
}
impl std::fmt::Display for ConstraintDeclarationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MultipleJustifications => {
                f.write_str("Maybe the constraint calls justifyWith() twice?")
            }
            Self::MissingJustification => {
                f.write_str("custom constraint declaration needs a justification callback")
            }
            Self::Graph(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for ConstraintDeclarationError {}
/// One declaration owns its actual input port, weight, match-factor callback and
/// custom justification callback. Registration does not evaluate any callback.
/// A failed duplicate preserves the first callback. This configures an independent
/// declaration; ordinary per-request graph explanation APIs remain available.
pub struct CustomConstraintDeclaration<'a, I, T, S, J> {
    input: StreamPort<I, T>,
    name: String,
    weight: S,
    factor: Box<Factor<'a, T>>,
    justification: Option<Box<Justify<'a, T, S, J>>>,
}
pub struct DeclaredCustomConstraint<'a, I, T, S, J> {
    constraint: StreamConstraint<I, T>,
    justification: Box<Justify<'a, T, S, J>>,
}
impl<'a, I, T, S, J> CustomConstraintDeclaration<'a, I, T, S, J> {
    pub fn new(
        input: &StreamPort<I, T>,
        name: impl Into<String>,
        weight: S,
        factor: impl Fn(&T) -> i64 + 'a,
    ) -> Self {
        Self {
            input: input.clone(),
            name: name.into(),
            weight,
            factor: Box::new(factor),
            justification: None,
        }
    }
    pub fn justify_with(
        &mut self,
        callback: impl Fn(&T, &S) -> J + 'a,
    ) -> Result<(), ConstraintDeclarationError> {
        if self.justification.is_some() {
            return Err(ConstraintDeclarationError::MultipleJustifications);
        }
        self.justification = Some(Box::new(callback));
        Ok(())
    }
}
impl<'a, I: Ord + Clone + 'static, T: Clone + 'static, S: AdditiveScore + 'static, J>
    CustomConstraintDeclaration<'a, I, T, S, J>
{
    pub fn build(
        self,
        builder: &mut StreamGraphBuilder<'a, S>,
    ) -> Result<DeclaredCustomConstraint<'a, I, T, S, J>, ConstraintDeclarationError> {
        let justification = self
            .justification
            .ok_or(ConstraintDeclarationError::MissingJustification)?;
        let factor = self.factor;
        let constraint = builder
            .score_weighted(self.name, &self.input, self.weight, move |row| factor(row))
            .map_err(ConstraintDeclarationError::Graph)?;
        Ok(DeclaredCustomConstraint {
            constraint,
            justification,
        })
    }
}
impl<I, T, S, J> DeclaredCustomConstraint<'_, I, T, S, J> {
    pub fn constraint(&self) -> &StreamConstraint<I, T> {
        &self.constraint
    }
}
impl<I: Ord + Clone + 'static, T: 'static, S: AdditiveScore + 'static, J>
    DeclaredCustomConstraint<'_, I, T, S, J>
{
    /// Invoke the configured callback on actual retained tuples and scores,
    /// including zero matches. No scoring callback runs during explanation.
    /// Indictment facts are independently selected and deduplicated by the graph.
    pub fn explain<F: Ord + Clone>(
        &self,
        graph: &StreamGraph<'_, S>,
        mut indict: impl FnMut(&T) -> Vec<F>,
    ) -> Result<ConstraintExplanation<I, S, J, F>, GraphError> {
        graph.explain_constraint(&self.constraint, |row, score| {
            ((self.justification)(row, score), indict(row))
        })
    }
}
