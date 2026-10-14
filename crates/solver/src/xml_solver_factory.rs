//! Owned XML declarations compiled within each reusable factory job.
use crate::{
    AdditiveScore, ConfiguredSolverManager, PhaseTerminationBinding, SolverFactory,
    SolverFactoryError, SolverProgress, SolverSequence, StatefulSearchTermination,
    XmlTerminationError, XmlTerminationPlan,
};
use std::{error::Error, fmt, io, str::FromStr, sync::Arc};

#[derive(Debug)]
pub enum XmlSolverFactoryBuildError {
    Binding(XmlTerminationError),
    Factory(SolverFactoryError),
}
impl fmt::Display for XmlSolverFactoryBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Binding(e) => e.fmt(f),
            Self::Factory(e) => e.fmt(f),
        }
    }
}
impl Error for XmlSolverFactoryBuildError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(match self {
            Self::Binding(e) => e,
            Self::Factory(e) => e,
        })
    }
}
#[derive(Debug)]
pub enum XmlSolverRunError<E> {
    Binding(XmlTerminationError),
    Solve(E),
}
impl<E: fmt::Display> fmt::Display for XmlSolverRunError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Binding(e) => write!(f, "XML binding failed: {e}"),
            Self::Solve(e) => e.fmt(f),
        }
    }
}
impl<E: Error + 'static> Error for XmlSolverRunError<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(match self {
            Self::Binding(e) => e,
            Self::Solve(e) => e,
        })
    }
}
/// Retains owned declarations, while each job borrows its own compiled expressions.
/// The executor installs the supplied bindings in `SolverExtensions` during its
/// solve. Inputs, scoring state and phase histories must be independent per job.
/// Native template limits and per-job overrides remain independent hard caps;
/// they do not replace conditions within the owned XML expression.
///
/// `factory().configuration()` exposes the native template. `termination_plan()`
/// exposes the additional XML conditions. No temporary expression references are
/// stored in the template or returned job configuration.
pub struct XmlSolverFactory<P, R, E, S: 'static = crate::SimpleScore> {
    plan: Arc<XmlTerminationPlan>,
    factory: SolverFactory<P, R, XmlSolverRunError<E>, S>,
}
impl<P, R, E, S: 'static> Clone for XmlSolverFactory<P, R, E, S> {
    fn clone(&self) -> Self {
        Self {
            plan: self.plan.clone(),
            factory: self.factory.clone(),
        }
    }
}
impl<P, R, E, S> XmlSolverFactory<P, R, E, S>
where
    P: Send + 'static,
    R: Send + Sync + 'static,
    E: Send + Sync + 'static,
    S: AdditiveScore + FromStr + Send + Sync + 'static,
{
    pub fn new(
        template: SolverSequence<'static, S>,
        plan: XmlTerminationPlan,
        execute: impl for<'a, 'b> Fn(
            P,
            SolverSequence<'a, S>,
            &[PhaseTerminationBinding<'b, S>],
            SolverProgress<R>,
        ) -> Result<R, E>
        + Send
        + Sync
        + 'static,
    ) -> Result<Self, XmlSolverFactoryBuildError> {
        // Validate all score texts, phase shapes and supported expression scopes
        // before publishing a factory or invoking any caller execution callback.
        plan.with_sequence_extensions(template.clone(), &[], |_, _| ())
            .map_err(XmlSolverFactoryBuildError::Binding)?;
        let plan = Arc::new(plan);
        let captured = plan.clone();
        let factory = SolverFactory::new(template, move |input, effective, progress| {
            captured
                .with_sequence_extensions(effective, &[], |sequence, bindings| {
                    execute(input, sequence, bindings, progress)
                })
                .map_err(XmlSolverRunError::Binding)?
                .map_err(XmlSolverRunError::Solve)
        })
        .map_err(XmlSolverFactoryBuildError::Factory)?;
        Ok(Self { plan, factory })
    }
    /// Compiles fresh solve-wide and phase-local controls for each owned job.
    /// The executor must install both lent outputs in `SolverExtensions`.
    pub fn new_with_controls(
        template: SolverSequence<'static, S>,
        plan: XmlTerminationPlan,
        execute: impl for<'a, 'b, 'c> Fn(
            P,
            SolverSequence<'a, S>,
            &[PhaseTerminationBinding<'b, S>],
            Option<&StatefulSearchTermination<'c, S>>,
            SolverProgress<R>,
        ) -> Result<R, E>
        + Send
        + Sync
        + 'static,
    ) -> Result<Self, XmlSolverFactoryBuildError> {
        plan.with_sequence_controls(template.clone(), &[], None, |_, _, _| ())
            .map_err(XmlSolverFactoryBuildError::Binding)?;
        let plan = Arc::new(plan);
        let captured = plan.clone();
        let factory = SolverFactory::new(template, move |input, effective, progress| {
            captured
                .with_sequence_controls(effective, &[], None, |sequence, bindings, stateful| {
                    execute(input, sequence, bindings, stateful, progress)
                })
                .map_err(XmlSolverRunError::Binding)?
                .map_err(XmlSolverRunError::Solve)
        })
        .map_err(XmlSolverFactoryBuildError::Factory)?;
        Ok(Self { plan, factory })
    }
    pub fn termination_plan(&self) -> &XmlTerminationPlan {
        &self.plan
    }
    pub fn factory(&self) -> &SolverFactory<P, R, XmlSolverRunError<E>, S> {
        &self.factory
    }
    pub fn build_manager(
        &self,
        workers: usize,
    ) -> io::Result<ConfiguredSolverManager<P, R, XmlSolverRunError<E>, S>> {
        self.factory.build_manager(workers)
    }
}
