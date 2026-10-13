//! Resolve an ordered list of configured inner stopping conditions once.
use crate::{
    SearchLimit, SearchTermination, SolverLimits, TerminationComposition, TerminationProfileError,
    TimeLimitSpec,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InnerTerminationCondition {
    SpentTime(TimeLimitSpec),
    Native(SearchLimit),
}

/// Native configuration for the inner list of a stopping policy. Unit-based
/// durations resolve before runtime; empty unit groups supply no condition.
/// Explicit zero durations remain active. Other native leaves retain their
/// exact scope. Deeper expressions use the existing borrowed expression API.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct InnerTerminationConfig {
    pub children: Vec<InnerTerminationCondition>,
    pub composition: TerminationComposition,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuiltInnerTermination {
    children: Vec<SearchTermination<'static>>,
    composition: TerminationComposition,
}

impl InnerTerminationConfig {
    /// Resolve every child before publishing the built policy. Resolution failure
    /// leaves the input configuration untouched and binds no partial policy.
    pub fn build(&self) -> Result<BuiltInnerTermination, TerminationProfileError> {
        let mut children = Vec::with_capacity(self.children.len());
        for child in &self.children {
            let condition = match *child {
                InnerTerminationCondition::SpentTime(spec) => {
                    spec.resolve()?.map(SearchLimit::Time)
                }
                InnerTerminationCondition::Native(condition) => Some(condition),
            };
            if let Some(condition) = condition {
                children.push(SearchTermination::Condition(condition));
            }
        }
        Ok(BuiltInnerTermination {
            children,
            composition: self.composition,
        })
    }
}

impl BuiltInnerTermination {
    pub fn children(&self) -> &[SearchTermination<'static>] {
        &self.children
    }

    /// Bind the configured child composition alongside any inherited condition.
    /// The inherited condition remains an independent OR safeguard. Other hard
    /// limits/cancellation stay on SolverLimits. Empty configuration preserves
    /// the inherited expression; no allocations or unit conversions occur here.
    pub fn with_limits<S, R>(
        &self,
        limits: SolverLimits<'_, S>,
        run: impl FnOnce(SolverLimits<'_, S>) -> R,
    ) -> R {
        let Some(configured) = SearchTermination::compose(&self.children, self.composition) else {
            return run(limits);
        };
        let inherited = limits
            .termination
            .copied()
            .unwrap_or(SearchTermination::Any(&[]));
        let children = [inherited, *configured];
        let combined = SearchTermination::Any(&children);
        run(SolverLimits {
            termination: Some(&combined),
            ..limits
        })
    }
}
