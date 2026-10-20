//! Direct callbacks remain concrete through the private terminal's Node impl.
//! Storage, transactional state, arithmetic, owner publication and callback
//! destruction order are shared with the original boxed/weighted evaluator.
use super::{GraphError, NodeArithmetic, TerminalEvaluation};

pub(super) trait TerminalEvaluator<T, S> {
    fn evaluate(&self, value: &T, name: &str, arithmetic: NodeArithmetic) -> Result<S, GraphError>;
    fn constraint_weight(&self) -> Option<&S> {
        None
    }
    fn replace_weight(&mut self, _new: S) {
        unreachable!("unweighted terminal")
    }
}

pub(super) struct DirectTerminal<F>(pub(super) F);
impl<T, S, F: Fn(&T) -> S> TerminalEvaluator<T, S> for DirectTerminal<F> {
    #[inline]
    fn evaluate(
        &self,
        value: &T,
        _name: &str,
        _arithmetic: NodeArithmetic,
    ) -> Result<S, GraphError> {
        Ok((self.0)(value))
    }
}
impl<T, S> TerminalEvaluator<T, S> for TerminalEvaluation<'_, T, S> {
    fn evaluate(&self, value: &T, name: &str, arithmetic: NodeArithmetic) -> Result<S, GraphError> {
        TerminalEvaluation::evaluate(self, value, name, arithmetic)
    }
    fn constraint_weight(&self) -> Option<&S> {
        match self {
            Self::Weighted { weight, .. } => Some(weight),
            _ => None,
        }
    }
    fn replace_weight(&mut self, new: S) {
        match self {
            Self::Weighted { weight, .. } => *weight = new,
            _ => unreachable!("unweighted terminal"),
        }
    }
}

#[cfg(test)]
mod tests;
