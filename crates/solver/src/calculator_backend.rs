//! Shared transaction implementation for the three native calculator backends.
pub(crate) struct CalculatorSnapshot<A, C: crate::IncrementalCalculator<A>> {
    pub(crate) session: crate::CalculatorSession<A, C>,
    pub(crate) score: C::Score,
}

/// Structural-delta hook for backends without a cheap delta: always rescan.
pub(crate) fn full_structure_scan<P, A, C>(_: &P, _: &A, _: &A, _: &C) -> Option<i64> {
    None
}

macro_rules! calculator_backend {
    ($director:ident, $guard:ident, [$($v:ident),+], $assignment:ty, $problem:ty,
     $movement:ty, $change:ty, $error:ident, $describe:path, $structure:path,
     $structure_delta:path, $apply:ident) => {
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
/// it twice by default. NonIntrusive reserves/counts a selected replay score.
/// Calculators and factories must be deterministic and keep captured
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
    undo_diagnostics: crate::CalculatorUndoDiagnostics,
    metric_capture: Option<crate::solver_metrics::ConstraintMetricCapture<C::Score>>,
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
        let undo_diagnostics = session.calculator().undo_diagnostics();
        Ok(Self {
            problem,
            factory,
            session,
            score,
            failure: None,
            undo_diagnostics,
            metric_capture: None,
        })
    }
    /// Configure actual per-trial undo checks for subsequent searches through
    /// this retained director. The default preserves legacy assert_scores checks.
    /// Tracked requires IncrementalCalculator::capture_undo_state support.
    pub fn with_undo_diagnostics(mut self, policy: crate::CalculatorUndoDiagnostics) -> Self {
        self.undo_diagnostics = policy;
        self
    }
    fn capture_undo(&self) -> Result<Option<crate::CalculatorStateSnapshot>, $error> {
        if self.undo_diagnostics != crate::CalculatorUndoDiagnostics::Tracked { return Ok(None); }
        self.healthy()?;
        self.session.calculator().capture_undo_state().map_err($error::Calculator)?
            .map(Some).ok_or_else(|| $error::Calculator(MatchRegistryError::Calculator("tracked calculator undo state is unsupported".into())))
    }
    fn verify_undo(&mut self, before: Option<crate::CalculatorStateSnapshot>, assert_scores: bool) -> Result<(), $error> {
        use crate::CalculatorUndoDiagnostics as Policy;
        match self.undo_diagnostics {
            Policy::Disabled | Policy::NonIntrusive => return Ok(()),
            Policy::Default => return if assert_scores { self.assert_consistent() } else { Ok(()) },
            Policy::Score | Policy::Tracked => {}
        }
        let fields = match before {
            Some(before) => {
                let after = self.capture_undo()?.expect("tracked state remains supported");
                before.differences(&after)
            }
            None => Vec::new(),
        };
        let score = match self.assert_consistent() {
            Ok(()) => None,
            Err($error::ScoreCorruption(report)) => Some(*report),
            Err(error) => return Err(error),
        };
        if fields.is_empty() && score.is_none() { return Ok(()); }
        Err($error::UndoCorruption(Box::new(crate::CalculatorUndoCorruption { score, fields })))
    }
    pub(crate) fn with_metric_capture(mut self, capture: Option<crate::solver_metrics::ConstraintMetricCapture<C::Score>>) -> Self {
        self.metric_capture = capture;
        self
    }
    pub(crate) fn capture_metrics(&self) {
        if let Some(capture) = &self.metric_capture { capture.capture(self.registry()); }
    }
    pub(crate) fn track_metrics(&mut self, capture: Option<crate::solver_metrics::ConstraintMetricCapture<C::Score>>) -> crate::calculator_backend::CalculatorMetricScope<'_, Self> {
        assert!(self.metric_capture.is_none(), "nested calculator metric scope");
        self.metric_capture = capture;
        crate::calculator_backend::CalculatorMetricScope { director: self, clear: |d| d.metric_capture = None }
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
    /// Score after a move whose structural change is known, avoiding a scan of
    /// every slot; `None` falls back to the full structural calculation.
    fn calculate_moved(
        problem: &$problem,
        session: &mut CalculatorSession<$assignment, C>,
        structural: Option<i64>,
    ) -> Result<C::Score, $error> {
        let Some(structural) = structural else {
            return Self::calculate(problem, session);
        };
        let score = session.score().map_err($error::Calculator)?;
        if score.structural() != 0 {
            return Err($error::Calculator(
                MatchRegistryError::StructuralContribution,
            ));
        }
        Ok(score.with_structural(structural))
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
    /// Inspect the actual ready calculator, including its owned shadow fields.
    /// Read adapters must not mutate scoring dependencies through interior state.
    pub fn calculator(&self) -> Result<&C, $error> {
        self.healthy()?;
        Ok(self.session.calculator())
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
    /// Compare cached/live scores with independent recomputation, using a fresh
    /// reset by default or the calculator's explicit projection-scoring hook.
    /// This does not repair live state or invalidate retained match handles.
    pub fn assert_consistent(&mut self) -> Result<(), $error> {
        self.healthy()?;
        let live = Self::calculate(self.problem, &mut self.session)?;
        let mut fresh = (self.factory)();
        let recomputed = if let Some(score) = fresh.recompute_score_from(self.session.calculator()).map_err($error::Calculator)? {
            if score.structural() != 0 { return Err($error::Calculator(MatchRegistryError::StructuralContribution)); }
            $structure(score, self.problem, self.assignment())?
        } else {
            let mut fresh = CalculatorSession::new(self.assignment().clone(), fresh, false)
                .map_err($error::Calculator)?;
            Self::calculate(self.problem, &mut fresh)?
        };
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
    pub(crate) fn capture_retained(&self) -> crate::calculator_backend::CalculatorSnapshot<$assignment, C>
    where C: Clone,
    {
        crate::calculator_backend::CalculatorSnapshot {
            session: self.session.snapshot(),
            score: self.score.clone(),
        }
    }
    pub(crate) fn install_retained_state(&mut self, snapshot: crate::calculator_backend::CalculatorSnapshot<$assignment, C>) {
        self.session.restore_snapshot(snapshot.session);
        self.score = snapshot.score;
        self.failure = None;
    }
    pub(crate) fn prepare_retained_phase(&mut self, initial: $assignment) -> Result<u128, $error> {
        self.prepare_retained_assignment(initial, false)
    }
    // Eq may compare planning identities rather than scoring payloads. Reuse an
    // equal assignment only when the caller preserves its payload representatives.
    pub(crate) fn prepare_retained_assignment(&mut self, initial: $assignment, refresh_payloads: bool) -> Result<u128, $error> {
        self.healthy()?;
        if !refresh_payloads && self.assignment() == &initial {
            return Ok(0);
        }
        self.reset(initial)?;
        Ok(1)
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
        let structural = $structure_delta(self.problem, self.assignment(), &candidate, &variables)
            .map(|delta| previous_score.structural() + delta);
        let mut guard = $guard {
            director: self,
            prior: None,
            variables,
            previous_score,
        };
        // The replaced assignment becomes the rollback state, avoiding a second
        // whole-assignment copy. A callback failure before the swap leaves the
        // solution untouched and the session awaiting reset.
        let prior = &mut guard.prior;
        guard
            .director
            .session
            .change(&guard.variables, |a| *prior = Some(std::mem::replace(a, candidate)))
            .map_err($error::Calculator)?;
        guard.director.score = match known_score {
            Some(score) => score,
            None => Self::calculate_moved(
                guard.director.problem,
                &mut guard.director.session,
                structural,
            )?,
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

pub(crate) struct CalculatorMetricScope<'a, D> {
    pub(crate) director: &'a mut D,
    pub(crate) clear: fn(&mut D),
}
impl<D> std::ops::Deref for CalculatorMetricScope<'_, D> {
    type Target = D;
    fn deref(&self) -> &D {
        self.director
    }
}
impl<D> std::ops::DerefMut for CalculatorMetricScope<'_, D> {
    fn deref_mut(&mut self) -> &mut D {
        self.director
    }
}
impl<D> Drop for CalculatorMetricScope<'_, D> {
    fn drop(&mut self) {
        (self.clear)(self.director);
    }
}
