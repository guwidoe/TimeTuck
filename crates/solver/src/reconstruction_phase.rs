//! Owned reconstruction contexts and fresh provider sessions for nested construction.
use crate::*;
use std::{cell::RefCell, error::Error, fmt, rc::Rc};
#[derive(Debug)]
enum ContextOrigin {
    Enclosing,
    Worker {
        parent: Rc<ContextOrigin>,
        index: usize,
    },
}
/// Actual working problem/assignment ownership for a nested reconstruction.
/// Derived workers clone the current model and assignment, retaining their real
/// parent session identity. No worker threads are spawned by this optional API.
pub struct ReconstructionContext<V> {
    problem: FiniteProblem<V>,
    assignment: Assignment<V>,
    global: EnvironmentMode,
    enclosing: EnvironmentMode,
    origin: Rc<ContextOrigin>,
}
impl<V: Eq + Clone> ReconstructionContext<V> {
    pub fn new(
        problem: FiniteProblem<V>,
        assignment: Assignment<V>,
        global: EnvironmentMode,
        enclosing: EnvironmentMode,
    ) -> Result<Self, ReconstructionContextError> {
        problem
            .validate(&assignment)
            .map_err(ReconstructionContextError::Model)?;
        EnvironmentProfile::new(Some(global), vec![Some(enclosing)])
            .validate()
            .map_err(ReconstructionContextError::Environment)?;
        Ok(Self {
            problem,
            assignment,
            global,
            enclosing,
            origin: Rc::new(ContextOrigin::Enclosing),
        })
    }
    pub fn derive_worker(&self, index: usize) -> Self {
        Self {
            problem: self.problem.clone(),
            assignment: self.assignment.clone(),
            global: self.global,
            enclosing: self.enclosing,
            origin: Rc::new(ContextOrigin::Worker {
                parent: self.origin.clone(),
                index,
            }),
        }
    }
    pub fn is_derived(&self) -> bool {
        matches!(*self.origin, ContextOrigin::Worker { .. })
    }
    pub fn worker_index(&self) -> Option<usize> {
        match self.origin.as_ref() {
            ContextOrigin::Enclosing => None,
            ContextOrigin::Worker { index, .. } => Some(*index),
        }
    }
    pub fn assignment(&self) -> &Assignment<V> {
        &self.assignment
    }
    pub fn replace_assignment(&mut self, assignment: Assignment<V>) -> Result<(), ModelError> {
        self.problem.validate(&assignment)?;
        self.assignment = assignment;
        Ok(())
    }
    pub fn environment_mode(&self) -> EnvironmentMode {
        self.enclosing
    }
    fn root_origin(&self) -> Rc<ContextOrigin> {
        let mut origin = self.origin.clone();
        while let ContextOrigin::Worker { parent, .. } = origin.as_ref() {
            origin = parent.clone();
        }
        origin
    }
}
#[derive(Debug)]
pub enum ReconstructionContextError {
    Model(ModelError),
    Environment(EnvironmentProfileError),
}
impl fmt::Display for ReconstructionContextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "reconstruction context: {self:?}")
    }
}
impl Error for ReconstructionContextError {}
#[derive(Debug)]
pub enum ReconstructionBuildError<E> {
    NoWorkers,
    ForeignContext,
    WorkerIndex { index: usize, count: usize },
    Copy(PlacerCopyError<E>),
}
impl<E: fmt::Display + fmt::Debug> fmt::Display for ReconstructionBuildError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "reconstruction phase: {self:?}")
    }
}
impl<E: Error + 'static> Error for ReconstructionBuildError<E> {}
type SharedPlacer<P, C, E> = Rc<RefCell<RebuildablePlacer<P, C, E>>>;
/// A nested phase recipe anchored to the enclosing live context's environment
/// and actual reusable construction provider. Derived contexts use factory copies
/// when more than one worker is configured; single-worker builds share the placer.
/// This chooses isolated sessions, not a parallel executor or thread-safe sharing.
pub struct ReconstructionPhaseBuilder<P, C, E> {
    placer: SharedPlacer<P, C, E>,
    mode: EnvironmentMode,
    workers: usize,
    root: Rc<ContextOrigin>,
}
pub struct ReconstructionPhase<P, C, E> {
    placer: SharedPlacer<P, C, E>,
    mode: EnvironmentMode,
    origin: Rc<ContextOrigin>,
}
impl<P, C, E> ReconstructionPhaseBuilder<P, C, E> {
    pub fn new<V: Eq + Clone>(
        context: &ReconstructionContext<V>,
        placer: RebuildablePlacer<P, C, E>,
        workers: usize,
    ) -> Result<Self, ReconstructionBuildError<E>> {
        if workers == 0 {
            return Err(ReconstructionBuildError::NoWorkers);
        }
        Ok(Self {
            placer: Rc::new(RefCell::new(placer)),
            mode: context.environment_mode(),
            workers,
            root: context.root_origin(),
        })
    }
    pub fn placer(&self) -> &SharedPlacer<P, C, E> {
        &self.placer
    }
    pub fn build_for<V: Eq + Clone>(
        &self,
        context: &ReconstructionContext<V>,
    ) -> Result<ReconstructionPhase<P, C, E>, ReconstructionBuildError<E>> {
        if !Rc::ptr_eq(&self.root, &context.root_origin()) {
            return Err(ReconstructionBuildError::ForeignContext);
        }
        if let Some(index) = context.worker_index()
            && index >= self.workers
        {
            return Err(ReconstructionBuildError::WorkerIndex {
                index,
                count: self.workers,
            });
        }
        let placer = if self.workers > 1 && context.is_derived() {
            Rc::new(RefCell::new(
                self.placer
                    .borrow()
                    .copy()
                    .map_err(ReconstructionBuildError::Copy)?,
            ))
        } else {
            self.placer.clone()
        };
        Ok(ReconstructionPhase {
            placer,
            mode: self.mode,
            origin: context.origin.clone(),
        })
    }
}
#[derive(Debug)]
pub enum ReconstructionRunError {
    ForeignContext,
    Model(ModelError),
    Solve(Box<SolverError<LocalSearchError>>),
}
impl fmt::Display for ReconstructionRunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "reconstruction run: {self:?}")
    }
}
impl Error for ReconstructionRunError {}
impl<P, C, E> ReconstructionPhase<P, C, E> {
    pub fn placer(&self) -> &SharedPlacer<P, C, E> {
        &self.placer
    }
    pub fn environment_mode(&self) -> EnvironmentMode {
        self.mode
    }
    /// Execute actual construction from this context's current partial assignment.
    /// Inherited phase/full modes install independent phase checks; step/full modes
    /// additionally enable the native construction scoring/undo consistency checks.
    /// Success installs the retained reconstruction; failure preserves the context's
    /// assignment while provider lifecycle/queue work follows normal native rules.
    pub fn reconstruct<V: Eq + Clone, S: AdditiveScore>(
        &mut self,
        context: &mut ReconstructionContext<V>,
        network: &ConstraintNetwork<'_, V, S>,
        mut construction: ConstructionConfig<'_>,
        mut fresh: impl FnMut(&Assignment<V>) -> Result<S, String>,
    ) -> Result<SolverResult<Assignment<V>, S>, ReconstructionRunError>
    where
        P: MoveProvider<FiniteProblem<V>, Assignment<V>, Move<V>>,
    {
        if !Rc::ptr_eq(&self.origin, &context.origin) {
            return Err(ReconstructionRunError::ForeignContext);
        }
        let problem = context
            .problem
            .with_initial_assignment(&context.assignment)
            .map_err(ReconstructionRunError::Model)?;
        construction.assert_scores |= matches!(
            self.mode,
            EnvironmentMode::FullAssert | EnvironmentMode::TrackedFullAssert
        );
        construction.assert_step_scores |= self.mode == EnvironmentMode::StepAssert;
        let mode = self.mode;
        let mut check = move |_: usize, _: SolverPhaseKind, a: &Assignment<V>| {
            if mode >= EnvironmentMode::PhaseAssert {
                fresh(a).map(Some)
            } else {
                Ok(None)
            }
        };
        let mut placer = self.placer.borrow_mut();
        let mut extensions = SolverExtensions::default();
        extensions.move_providers.bind(0, placer.session_mut());
        extensions.score_check = Some(&mut check);
        let result = solve_with_extensions(
            &problem,
            network,
            SolverSequence {
                phases: vec![SolverPhase::Construction(construction)],
                ..Default::default()
            },
            extensions,
        )
        .map_err(|e| ReconstructionRunError::Solve(Box::new(e)))?;
        context.assignment = result.assignment.clone();
        Ok(result)
    }
}
