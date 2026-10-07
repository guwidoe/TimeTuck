//! Basic-variable search using application-defined incremental calculators.
//!
//! ```
//! use timetuck::*;
//! #[derive(Default)]
//! struct Sum(i64);
//! impl IncrementalCalculator<Assignment<i64>> for Sum {
//!     type Score = SimpleScore;
//!     type Justification = ();
//!     type Change = Vec<usize>;
//!     fn reset(&mut self, a: &Assignment<i64>, _: &mut MatchRegistry<SimpleScore, ()>)
//!         -> Result<(), MatchRegistryError> {
//!         self.0 = a.values().iter().flatten().sum();
//!         Ok(())
//!     }
//!     fn before_change(&mut self, a: &Assignment<i64>, changed: &Vec<usize>,
//!         _: &mut MatchRegistry<SimpleScore, ()>) -> Result<(), MatchRegistryError> {
//!         for &i in changed { self.0 -= a.values()[i].unwrap_or(0); }
//!         Ok(())
//!     }
//!     fn after_change(&mut self, a: &Assignment<i64>, changed: &Vec<usize>,
//!         _: &mut MatchRegistry<SimpleScore, ()>) -> Result<(), MatchRegistryError> {
//!         for &i in changed { self.0 += a.values()[i].unwrap_or(0); }
//!         Ok(())
//!     }
//!     fn calculate_score(&mut self) -> SimpleScore { SimpleScore::new(self.0) }
//! }
//! let p = FiniteProblem::new(vec![Variable::new(vec![0_i64, 1, 2]); 2])?;
//! let built = calculator_construction(&p, p.initial_assignment(), Sum::default,
//!     ConstructionConfig::default())?;
//! let improved = calculator_local_search_from(&p, built.assignment, Sum::default,
//!     LocalSearchConfig { assert_scores: true, ..Default::default() })?;
//! assert_eq!(improved.best.unwrap().score, SimpleScore::new(4));
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
use crate::*;
use crate::{scoring::Selection, search_scoring::SearchDirector};
use std::panic::{AssertUnwindSafe, catch_unwind};

/// A calculator backend for native basic moves. Change descriptors contain the
/// sorted unique variables touched by the whole move. Callbacks see complete
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
pub struct CalculatorScoreDirector<'p, V, C, F>
where
    C: IncrementalCalculator<Assignment<V>, Change = Vec<usize>>,
{
    problem: &'p FiniteProblem<V>,
    factory: F,
    session: CalculatorSession<Assignment<V>, C>,
    score: C::Score,
    failure: Option<MatchRegistryError>,
}
impl<'p, V, C, F> CalculatorScoreDirector<'p, V, C, F>
where
    V: Eq + Clone,
    C: IncrementalCalculator<Assignment<V>, Change = Vec<usize>>,
    F: Fn() -> C,
{
    pub fn new(
        problem: &'p FiniteProblem<V>,
        initial: Assignment<V>,
        factory: F,
        request_matches: bool,
    ) -> Result<Self, ScoringError> {
        problem.validate(&initial)?;
        let mut session = CalculatorSession::new(initial, factory(), request_matches)
            .map_err(ScoringError::Calculator)?;
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
        problem: &FiniteProblem<V>,
        session: &mut CalculatorSession<Assignment<V>, C>,
    ) -> Result<C::Score, ScoringError> {
        let score = session.score().map_err(ScoringError::Calculator)?;
        if score.structural() != 0 {
            return Err(ScoringError::Calculator(
                MatchRegistryError::StructuralContribution,
            ));
        }
        Ok(crate::scoring::with_structure(
            score,
            problem,
            session.solution(),
        ))
    }
    fn healthy(&self) -> Result<(), ScoringError> {
        if let Some(error) = &self.failure {
            return Err(ScoringError::Calculator(error.clone()));
        }
        self.session
            .registry()
            .map(|_| ())
            .map_err(ScoringError::Calculator)
    }
    pub fn assignment(&self) -> &Assignment<V> {
        self.session.solution()
    }
    pub fn score(&self) -> Result<C::Score, ScoringError> {
        self.healthy()?;
        Ok(self.score.clone())
    }
    pub fn registry(&self) -> Result<&MatchRegistry<C::Score, C::Justification>, ScoringError> {
        self.healthy()?;
        self.session.registry().map_err(ScoringError::Calculator)
    }
    /// Recover by rebuilding the calculator on a validated assignment. Invalid
    /// model input leaves the previous director untouched; callback errors do not.
    pub fn reset(&mut self, initial: Assignment<V>) -> Result<(), ScoringError> {
        self.problem.validate(&initial)?;
        self.failure = Some(MatchRegistryError::NeedsReset);
        self.session
            .reset(initial)
            .map_err(ScoringError::Calculator)?;
        self.score = Self::calculate(self.problem, &mut self.session)?;
        self.failure = None;
        Ok(())
    }
    /// Compare cached, live calculator and fresh-reset scores without repairing
    /// the live calculator or invalidating its retained match handles.
    pub fn assert_consistent(&mut self) -> Result<(), ScoringError> {
        self.healthy()?;
        let live = Self::calculate(self.problem, &mut self.session)?;
        let mut fresh = CalculatorSession::new(self.assignment().clone(), (self.factory)(), false)
            .map_err(ScoringError::Calculator)?;
        let recomputed = Self::calculate(self.problem, &mut fresh)?;
        for working in [&self.score, &live] {
            if *working != recomputed {
                return Err(ScoringError::ScoreCorruption(Box::new(
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
        movement: &Move<V>,
    ) -> Result<CalculatorMove<'_, 'p, V, C, F>, ScoringError> {
        self.apply_with_score(movement, None)
    }
    fn apply_with_score(
        &mut self,
        movement: &Move<V>,
        known_score: Option<C::Score>,
    ) -> Result<CalculatorMove<'_, 'p, V, C, F>, ScoringError> {
        self.healthy()?;
        let mut candidate = self.assignment().clone();
        self.problem.apply(&mut candidate, movement)?.commit();
        let variables = movement.touched_variables();
        let previous_score = self.score.clone();
        let prior = Some(self.assignment().clone());
        let guard = CalculatorMove {
            director: self,
            prior,
            variables,
            previous_score,
        };
        guard
            .director
            .session
            .change(&guard.variables, |a| *a = candidate)
            .map_err(ScoringError::Calculator)?;
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
pub struct CalculatorMove<'d, 'p, V, C, F>
where
    C: IncrementalCalculator<Assignment<V>, Change = Vec<usize>>,
{
    director: &'d mut CalculatorScoreDirector<'p, V, C, F>,
    prior: Option<Assignment<V>>,
    variables: Vec<usize>,
    previous_score: C::Score,
}
impl<V, C, F> CalculatorMove<'_, '_, V, C, F>
where
    C: IncrementalCalculator<Assignment<V>, Change = Vec<usize>>,
{
    pub fn assignment(&self) -> &Assignment<V> {
        self.director.session.solution()
    }
    pub fn score(&self) -> C::Score {
        self.director.score.clone()
    }
    pub fn commit(mut self) {
        self.prior = None;
    }
}
impl<V, C, F> Drop for CalculatorMove<'_, '_, V, C, F>
where
    C: IncrementalCalculator<Assignment<V>, Change = Vec<usize>>,
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

impl<V, C, F> SearchDirector<V, C::Score> for CalculatorScoreDirector<'_, V, C, F>
where
    V: Eq + Clone,
    C: IncrementalCalculator<Assignment<V>, Change = Vec<usize>>,
    F: Fn() -> C,
{
    type Error = ScoringError;
    fn assignment(&self) -> &Assignment<V> {
        self.assignment()
    }
    fn score(&self) -> C::Score {
        self.score.clone()
    }
    fn initial_constraint_evaluations(&self) -> u128 {
        1
    }
    fn assert_consistent(&mut self) -> Result<(), Self::Error> {
        self.assert_consistent()
    }
    fn select_best(
        &mut self,
        moves: impl IntoIterator<Item = Move<V>>,
        max_evaluations: u64,
        forager: impl Into<ForagerConfig<C::Score>>,
        mut accepts: impl FnMut(&Move<V>, C::Score) -> bool,
        quit_early: impl crate::construction::SelectionPolicy<C::Score>,
        assert_scores: bool,
    ) -> Result<Selection<V>, ScoringError> {
        self.healthy()?;
        let mut result = Selection {
            selected_accepted: false,
            selected: None,
            evaluations: 0,
            constraint_evaluations: 0,
            evaluation_limit: false,
        };
        let mut forager = MoveForager::new(forager.into());
        for movement in moves {
            if result.evaluations == max_evaluations {
                result.evaluation_limit = true;
                break;
            }
            let candidate = self.apply(&movement)?;
            result.evaluations += 1;
            result.constraint_evaluations += 1;
            if assert_scores {
                candidate.director.assert_consistent()?;
            }
            let score = candidate.score();
            let accepted = accepts(&movement, score.clone());
            forager.try_add_with(score.clone(), accepted, || -> Result<_, ScoringError> {
                Ok((movement.clone(), score.clone(), accepted))
            })?;
            let stop = forager.is_quit_early() || (accepted && quit_early.should_pick(score));
            drop(candidate);
            self.healthy()?;
            if stop {
                break;
            }
        }
        if !quit_early.can_commit(result.evaluation_limit) {
            return Ok(result);
        }
        if let Some((movement, score, accepted)) = forager.pick() {
            self.apply_with_score(&movement, Some(score))?.commit();
            if assert_scores {
                self.assert_consistent()?;
            }
            result.selected = Some(movement);
            result.selected_accepted = accepted;
        }
        Ok(result)
    }
}

impl<V, C, F> crate::custom_phase::CommandDirector<Assignment<V>, Move<V>, C::Score>
    for CalculatorScoreDirector<'_, V, C, F>
where
    V: Eq + Clone,
    C: IncrementalCalculator<Assignment<V>, Change = Vec<usize>>,
    F: Fn() -> C,
{
    type Error = ScoringError;
    fn assignment(&self) -> &Assignment<V> {
        self.assignment()
    }
    fn score(&self) -> C::Score {
        self.score.clone()
    }
    fn initial_evaluations(&self) -> u128 {
        1
    }
    fn assert_consistent(&mut self) -> Result<(), ScoringError> {
        self.assert_consistent()
    }
    fn trial(
        &mut self,
        movement: &Move<V>,
        visit: &mut dyn FnMut(&Assignment<V>, C::Score, u128) -> bool,
    ) -> Result<(), ScoringError> {
        let candidate = self.apply(movement)?;
        if visit(candidate.assignment(), candidate.score(), 1) {
            candidate.commit();
        } else {
            drop(candidate);
        }
        self.healthy()
    }
}

/// Greedy construction using the same placement, interruption and budget policies
/// as scoped scoring. Explanations are disabled during standalone search.
pub fn calculator_construction<V, C>(
    problem: &FiniteProblem<V>,
    initial: Assignment<V>,
    factory: impl Fn() -> C,
    config: ConstructionConfig<'_>,
) -> Result<ConstructionResult<V, C::Score>, ScoringError>
where
    V: Eq + Clone,
    C: IncrementalCalculator<Assignment<V>, Change = Vec<usize>>,
{
    problem.validate(&initial)?;
    crate::construction::construction_impl_monitored(None, problem, initial, config, |a| {
        CalculatorScoreDirector::new(problem, a, factory, false)
    })
}
/// Basic local search with existing acceptance, forager, neighborhood and limit
/// semantics. Use explicit construction first for an incomplete starting state.
pub fn calculator_local_search_from<V, C>(
    problem: &FiniteProblem<V>,
    initial: Assignment<V>,
    factory: impl Fn() -> C,
    config: LocalSearchConfig<'_, C::Score>,
) -> Result<LocalSearchResult<V, C::Score>, LocalSearchError>
where
    V: Eq + Clone,
    C: IncrementalCalculator<Assignment<V>, Change = Vec<usize>>,
{
    crate::local_search::search_impl_monitored(None, problem, Some(initial), config, |a| {
        CalculatorScoreDirector::new(problem, a, factory, false)
    })
}
/// Local search over a caller-defined lazy neighborhood.
pub fn calculator_local_search_with_provider<V, C>(
    problem: &FiniteProblem<V>,
    initial: Assignment<V>,
    factory: impl Fn() -> C,
    config: LocalSearchConfig<'_, C::Score>,
    provider: &mut dyn MoveProvider<FiniteProblem<V>, Assignment<V>, Move<V>>,
) -> Result<LocalSearchResult<V, C::Score>, LocalSearchError>
where
    V: Eq + Clone,
    C: IncrementalCalculator<Assignment<V>, Change = Vec<usize>>,
{
    let clock = crate::time_limit::monotonic_clock(
        config.max_time.is_some()
            || config.max_unimproved_time.is_some()
            || config.diminished_returns.is_some()
            || config.significant_improvement.is_some(),
    );
    crate::local_search::search_impl_with_provider(
        None,
        problem,
        Some(initial),
        config,
        |a| CalculatorScoreDirector::new(problem, a, factory, false),
        clock,
        Some(provider),
    )
}
/// Run bounded custom commands and publish initial/best updates using the shared
/// custom-phase engine. Trial moves use inverse calculator notifications.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn calculator_custom_phase<V, C>(
    problem: &FiniteProblem<V>,
    initial: Assignment<V>,
    factory: impl Fn() -> C,
    config: CustomPhaseConfig<'_>,
    command: impl FnOnce(
        &mut CustomPhaseContext<'_, Assignment<V>, Move<V>, C::Score, ScoringError>,
    ) -> Result<(), CustomPhaseError<ScoringError>>,
    observer: impl FnMut(SolutionUpdate<'_, Assignment<V>, C::Score>),
) -> Result<CustomPhaseResult<Assignment<V>, C::Score>, CustomPhaseError<ScoringError>>
where
    V: Eq + Clone,
    C: IncrementalCalculator<Assignment<V>, Change = Vec<usize>>,
{
    problem
        .validate(&initial)
        .map_err(|e| CustomPhaseError::Scoring(ScoringError::Model(e)))?;
    crate::custom_phase::run_monitored(
        initial,
        config,
        |a| problem.is_complete(a),
        |a| CalculatorScoreDirector::new(problem, a, factory, false),
        command,
        observer,
        None,
        None,
        false,
    )
}
