//! Shared transaction implementation for the three native calculator backends.
macro_rules! calculator_backend {
    ($director:ident, $guard:ident, [$($v:ident),+], $assignment:ty, $problem:ty,
     $movement:ty, $change:ty, $error:ident, $describe:path, $structure:path, $apply:ident) => {
/// A calculator backend for native moves. Change descriptors identify the
/// slots touched by the whole move. Callbacks see complete
/// pre/post assignments, including for composites and their inverse updates.
///
/// The factory must create independent calculator state with the same objective.
/// It is called at initialization and by explicit consistency checks. Calculators
/// return ordinary scores; this director supplies required-variable structure.
/// One calculator invocation is counted as one opaque constraint evaluation;
/// callback work, inverse notifications and diagnostic recomputation are not
/// separately counted. Scans replay the winner's notifications without scoring
/// it twice. Calculators and factories must be deterministic and keep captured
/// problem facts immutable throughout the search.
///
/// Candidate preparation copies the assignment, but never the calculator cache.
/// A rollback callback failure restores assignment data and invalidates the
/// director until reset. External callback side effects cannot be rolled back.
pub struct $director<'p, $($v,)+ C, F>
where
    C: IncrementalCalculator<$assignment, Change = $change>,
{
    problem: &'p $problem,
    factory: F,
    session: CalculatorSession<$assignment, C>,
    score: C::Score,
    failure: Option<MatchRegistryError>,
}
impl<'p, $($v,)+ C, F> $director<'p, $($v,)+ C, F>
where
    $($v: Eq + Clone,)+
    C: IncrementalCalculator<$assignment, Change = $change>,
    F: Fn() -> C,
{
    pub fn new(
        problem: &'p $problem,
        initial: $assignment,
        factory: F,
        request_matches: bool,
    ) -> Result<Self, $error> {
        problem.validate(&initial)?;
        let mut session = CalculatorSession::new(initial, factory(), request_matches)
            .map_err($error::Calculator)?;
        let score = Self::calculate(problem, &mut session)?;
        Ok(Self {
            problem,
            factory,
            session,
            score,
            failure: None,
        })
    }
    fn calculate(
        problem: &$problem,
        session: &mut CalculatorSession<$assignment, C>,
    ) -> Result<C::Score, $error> {
        let score = session.score().map_err($error::Calculator)?;
        if score.structural() != 0 {
            return Err($error::Calculator(
                MatchRegistryError::StructuralContribution,
            ));
        }
        $structure(score, problem, session.solution())
    }
    fn healthy(&self) -> Result<(), $error> {
        if let Some(error) = &self.failure {
            return Err($error::Calculator(error.clone()));
        }
        self.session
            .registry()
            .map(|_| ())
            .map_err($error::Calculator)
    }
    pub fn assignment(&self) -> &$assignment {
        self.session.solution()
    }
    pub fn score(&self) -> Result<C::Score, $error> {
        self.healthy()?;
        Ok(self.score.clone())
    }
    pub fn registry(&self) -> Result<&MatchRegistry<C::Score, C::Justification>, $error> {
        self.healthy()?;
        self.session.registry().map_err($error::Calculator)
    }
    /// Recover by rebuilding the calculator on a validated assignment. Invalid
    /// model input leaves the previous director untouched; callback errors do not.
    pub fn reset(&mut self, initial: $assignment) -> Result<(), $error> {
        self.problem.validate(&initial)?;
        self.failure = Some(MatchRegistryError::NeedsReset);
        self.session
            .reset(initial)
            .map_err($error::Calculator)?;
        self.score = Self::calculate(self.problem, &mut self.session)?;
        self.failure = None;
        Ok(())
    }
    /// Compare cached, live calculator and fresh-reset scores without repairing
    /// the live calculator or invalidating its retained match handles.
    pub fn assert_consistent(&mut self) -> Result<(), $error> {
        self.healthy()?;
        let live = Self::calculate(self.problem, &mut self.session)?;
        let mut fresh = CalculatorSession::new(self.assignment().clone(), (self.factory)(), false)
            .map_err($error::Calculator)?;
        let recomputed = Self::calculate(self.problem, &mut fresh)?;
        for working in [&self.score, &live] {
            if *working != recomputed {
                return Err($error::ScoreCorruption(Box::new(
                    ScoreConsistencyReport {
                        working_score: working.clone(),
                        recomputed_score: recomputed,
                        constraints: vec![],
                    }
                    .diagnostic(),
                )));
            }
        }
        Ok(())
    }
    pub fn apply(
        &mut self,
        movement: &$movement,
    ) -> Result<$guard<'_, 'p, $($v,)+ C, F>, $error> {
        self.apply_with_score(movement, None)
    }
    fn apply_with_score(
        &mut self,
        movement: &$movement,
        known_score: Option<C::Score>,
    ) -> Result<$guard<'_, 'p, $($v,)+ C, F>, $error> {
        self.healthy()?;
        let mut candidate = self.assignment().clone();
        self.problem.$apply(&mut candidate, movement)?.commit();
        let variables = $describe(movement);
        let previous_score = self.score.clone();
        let prior = Some(self.assignment().clone());
        let guard = $guard {
            director: self,
            prior,
            variables,
            previous_score,
        };
        guard
            .director
            .session
            .change(&guard.variables, |a| *a = candidate)
            .map_err($error::Calculator)?;
        guard.director.score = match known_score {
            Some(score) => score,
            None => Self::calculate(guard.director.problem, &mut guard.director.session)?,
        };
        Ok(guard)
    }
}

/// Drop sends inverse notifications and restores the original assignment/score.
/// Rollback panics are contained, recorded as a calculator error and make the
/// director unavailable until reset; an already-unwinding visitor panic continues.
/// Normal guard limits (forget, abort, panicking value destructors) apply.
#[must_use = "dropping the move rolls it back; commit to retain it"]
pub struct $guard<'d, 'p, $($v,)+ C, F>
where
    C: IncrementalCalculator<$assignment, Change = $change>,
{
    director: &'d mut $director<'p, $($v,)+ C, F>,
    prior: Option<$assignment>,
    variables: $change,
    previous_score: C::Score,
}
impl<$($v,)+ C, F> $guard<'_, '_, $($v,)+ C, F>
where
    C: IncrementalCalculator<$assignment, Change = $change>,
{
    pub fn assignment(&self) -> &$assignment {
        self.director.session.solution()
    }
    pub fn score(&self) -> C::Score {
        self.director.score.clone()
    }
    pub fn commit(mut self) {
        self.prior = None;
    }
}
impl<$($v,)+ C, F> Drop for $guard<'_, '_, $($v,)+ C, F>
where
    C: IncrementalCalculator<$assignment, Change = $change>,
{
    fn drop(&mut self) {
        if self.prior.is_none() {
            return;
        }
        let result = catch_unwind(AssertUnwindSafe(|| {
            self.director.session.change(&self.variables, |a| {
                *a = self.prior.take().expect("pending rollback");
            })
        }));
        let failure = match result {
            Ok(Ok(())) => None,
            Ok(Err(error)) => Some(error),
            Err(_) => Some(MatchRegistryError::Calculator(
                "calculator rollback callback panicked".into(),
            )),
        };
        if let Some(prior) = self.prior.take() {
            self.director.session.restore_failed(prior);
        }
        // Errors/panics after the rollback mutation already poison the session.
        self.director.failure = failure;
        self.director.score = self.previous_score.clone();
    }
}


    };
}
pub(crate) use calculator_backend;
