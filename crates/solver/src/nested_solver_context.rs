//! Managed calculation counts for a native plan borrowing an outer director.
use crate::*;
use std::{error::Error, fmt};
/// A child scope borrows the actual retained director. Closing this scope releases
/// the borrow, never the outer director or its external resources.
pub struct NestedScoreResource<'d, D> {
    director: &'d mut D,
}
impl<D> NestedScoreResource<'_, D> {
    pub fn director(&self) -> &D {
        self.director
    }
}
impl<D: ContextScoreResource> ContextScoreResource for NestedScoreResource<'_, D> {
    type Score = D::Score;
    type Error = D::Error;
    fn context_score(&self) -> D::Score {
        self.director.context_score()
    }
    fn check_context(&mut self) -> Result<(), D::Error> {
        self.director.check_context()
    }
    fn context_totals(&self) -> Vec<(String, D::Score)> {
        self.director.context_totals()
    }
}
pub type NestedSolverView<'d, D> = ScoringContextHandle<NestedScoreResource<'d, D>>;
#[derive(Debug)]
pub enum NestedSolverError<E, C> {
    Solver(SolverError<E>),
    Context(ScoringContextError<C>),
}
impl<E: fmt::Display, C: fmt::Display> fmt::Display for NestedSolverError<E, C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Solver(e) => e.fmt(f),
            Self::Context(e) => e.fmt(f),
        }
    }
}
impl<E: Error + 'static, C: Error + 'static> Error for NestedSolverError<E, C> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Solver(e) => Some(e),
            Self::Context(e) => Some(e),
        }
    }
}
pub(crate) struct NestedWork<A, S, E>(pub(crate) Result<SolverResult<A, S>, SolverError<E>>);
impl<A, S, E> ContextScoreWork for NestedWork<A, S, E> {
    fn context_calculations(&self) -> u64 {
        self.0.as_ref().map_or(0, |r| r.work.evaluations)
    }
}
pub(crate) fn run<'d, D: ContextScoreResource, W: ContextScoreWork>(
    director: &'d mut D,
    solve: impl FnOnce(&mut D) -> W,
    inspect: impl FnOnce(&NestedSolverView<'d, D>),
) -> Result<W, ScoringContextError<D::Error>> {
    let mut context = ScoringContextManager::new(
        NestedScoreResource { director },
        EnvironmentMode::NoAssert,
        vec![],
        ContextMetrics::default(),
        |_: &mut NestedScoreResource<'d, D>,
         _: ContextSettings|
         -> Result<NestedScoreResource<'d, D>, D::Error> {
            unreachable!("a nested solve scope has no context mode transitions")
        },
    );
    context.start()?;
    // Phase diagnostics remain in the supplied native plan; this context records
    // actual completed work and cannot close/rebuild the parent scoring resource.
    let work = context.run(|resource| Ok::<_, D::Error>(solve(resource.director)))?;
    inspect(&context.handle());
    context.finish()?;
    Ok(work)
}
