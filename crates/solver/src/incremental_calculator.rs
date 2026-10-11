//! Application-defined incremental scoring, with optional retained explanations.
//!
//! A session owns its working solution and brackets each caller-supplied change
//! with calculator callbacks. Change descriptors must describe every modified
//! scoring dependency. This API does not infer dependencies or automatically plug
//! calculators into the built-in scoped/stream search engines.
use crate::AdditiveScore;
use std::{collections::BTreeMap, error::Error, fmt, sync::Arc};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MatchRegistryError {
    Disabled,
    ForeignOrExpiredRegistration,
    AlreadyCanceled,
    ScoreOverflow,
    StructuralContribution,
    IdentityExhausted,
    NeedsReset,
    Calculator(String),
}
impl fmt::Display for MatchRegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Disabled => f.write_str("constraint matching is DISABLED"),
            Self::ForeignOrExpiredRegistration => {
                f.write_str("registration belongs to another registry or a previous reset")
            }
            Self::AlreadyCanceled => f.write_str("registration can only be canceled once"),
            Self::ScoreOverflow => f.write_str("constraint match score overflow"),
            Self::StructuralContribution => {
                f.write_str("constraint matches must have zero structural score")
            }
            Self::IdentityExhausted => f.write_str("constraint match identity space exhausted"),
            Self::NeedsReset => f.write_str("calculator session needs a successful reset"),
            Self::Calculator(message) => f.write_str(message),
        }
    }
}
impl Error for MatchRegistryError {}

/// Immutable occurrence payload. Equal payloads remain distinct registrations.
#[derive(Debug)]
pub struct RegisteredMatch<S, J> {
    pub constraint: String,
    pub score: S,
    pub justification: J,
}

/// Explicit cancellation handle; dropping it does not retract the match.
/// Cloned handles refer to the same occurrence, so cancellation is allowed once
/// across all copies. Retained metadata remains readable after reset/cancellation.
#[derive(Debug)]
pub struct MatchRegistration<S, J> {
    owner: Arc<()>,
    id: usize,
    record: Arc<RegisteredMatch<S, J>>,
}
impl<S, J> Clone for MatchRegistration<S, J> {
    fn clone(&self) -> Self {
        Self {
            owner: self.owner.clone(),
            id: self.id,
            record: self.record.clone(),
        }
    }
}
impl<S, J> MatchRegistration<S, J> {
    pub fn record(&self) -> &RegisteredMatch<S, J> {
        &self.record
    }
    pub fn id(&self) -> usize {
        self.id
    }
}
impl<S: AdditiveScore, J> MatchRegistration<S, J> {
    pub fn cancel(&self, registry: &mut MatchRegistry<S, J>) -> Result<(), MatchRegistryError> {
        registry.cancel(self)
    }
}

#[derive(Debug)]
pub struct RegisteredConstraint<S, J> {
    score: S,
    matches: BTreeMap<usize, Arc<RegisteredMatch<S, J>>>,
}
impl<S, J> RegisteredConstraint<S, J> {
    pub fn score(&self) -> &S {
        &self.score
    }
    pub fn matches(&self) -> &BTreeMap<usize, Arc<RegisteredMatch<S, J>>> {
        &self.matches
    }
}

/// Checked ordinary score totals; empty constraints survive cancellation, but not
/// reset. Reset expires all old handles, including handles whose IDs are reused.
#[derive(Debug)]
pub struct MatchRegistry<S, J> {
    enabled: bool,
    owner: Arc<()>,
    next_id: usize,
    high_water: usize,
    total: S,
    constraints: BTreeMap<String, RegisteredConstraint<S, J>>,
}
impl<S: AdditiveScore, J> MatchRegistry<S, J> {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            owner: Arc::new(()),
            next_id: 0,
            high_water: 0,
            total: S::ZERO,
            constraints: BTreeMap::new(),
        }
    }
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
    fn require_enabled(&self) -> Result<(), MatchRegistryError> {
        if self.enabled {
            Ok(())
        } else {
            Err(MatchRegistryError::Disabled)
        }
    }
    pub fn total(&self) -> Result<&S, MatchRegistryError> {
        self.require_enabled()?;
        Ok(&self.total)
    }
    pub fn constraints(
        &self,
    ) -> Result<&BTreeMap<String, RegisteredConstraint<S, J>>, MatchRegistryError> {
        self.require_enabled()?;
        Ok(&self.constraints)
    }
    pub fn reset(&mut self) {
        self.owner = Arc::new(());
        self.next_id = 0;
        self.total = S::ZERO;
        self.constraints.clear();
    }
    pub fn register(
        &mut self,
        constraint: impl Into<String>,
        score: S,
        justification: J,
    ) -> Result<MatchRegistration<S, J>, MatchRegistryError> {
        self.require_enabled()?;
        if score.structural() != 0 {
            return Err(MatchRegistryError::StructuralContribution);
        }
        let next_id = self
            .next_id
            .checked_add(1)
            .ok_or(MatchRegistryError::IdentityExhausted)?;
        let name = constraint.into();
        let total =
            S::checked_sum([&self.total, &score]).ok_or(MatchRegistryError::ScoreOverflow)?;
        let zero = S::ZERO;
        let old = self.constraints.get(&name).map_or(&zero, |c| &c.score);
        let subtotal = S::checked_sum([old, &score]).ok_or(MatchRegistryError::ScoreOverflow)?;
        let record = Arc::new(RegisteredMatch {
            constraint: name.clone(),
            score,
            justification,
        });
        let entry = self
            .constraints
            .entry(name)
            .or_insert_with(|| RegisteredConstraint {
                score: S::ZERO,
                matches: BTreeMap::new(),
            });
        entry.score = subtotal;
        entry.matches.insert(self.next_id, record.clone());
        let handle = MatchRegistration {
            owner: self.owner.clone(),
            id: self.next_id,
            record,
        };
        self.next_id = next_id;
        self.high_water = self.high_water.max(next_id);
        self.total = total;
        Ok(handle)
    }
    pub fn cancel(&mut self, handle: &MatchRegistration<S, J>) -> Result<(), MatchRegistryError> {
        self.require_enabled()?;
        if !Arc::ptr_eq(&self.owner, &handle.owner) {
            return Err(MatchRegistryError::ForeignOrExpiredRegistration);
        }
        let entry = self
            .constraints
            .get(&handle.record.constraint)
            .ok_or(MatchRegistryError::AlreadyCanceled)?;
        if !entry.matches.contains_key(&handle.id) {
            return Err(MatchRegistryError::AlreadyCanceled);
        }
        let total = self
            .total
            .checked_adjust([(&handle.record.score, &S::ZERO)])
            .ok_or(MatchRegistryError::ScoreOverflow)?;
        let subtotal = entry
            .score
            .checked_adjust([(&handle.record.score, &S::ZERO)])
            .ok_or(MatchRegistryError::ScoreOverflow)?;
        let entry = self
            .constraints
            .get_mut(&handle.record.constraint)
            .expect("validated constraint");
        entry.matches.remove(&handle.id);
        entry.score = subtotal;
        self.total = total;
        Ok(())
    }
}

/// Calculator callbacks receive a registry even when matching is disabled; test
/// `is_enabled()` before registering optional explanations. The calculated score
/// is independent of the registry total; applications may validate agreement.
/// Callbacks must not mutate dependencies through shared interior state outside
/// the session. Return an error for a failed update; recovery requires reset.
pub trait IncrementalCalculator<P> {
    type Score: AdditiveScore;
    type Justification;
    type Change;
    fn supports_matches(&self) -> bool {
        false
    }
    fn reset(
        &mut self,
        solution: &P,
        registry: &mut MatchRegistry<Self::Score, Self::Justification>,
    ) -> Result<(), MatchRegistryError>;
    fn before_change(
        &mut self,
        _solution: &P,
        _change: &Self::Change,
        _registry: &mut MatchRegistry<Self::Score, Self::Justification>,
    ) -> Result<(), MatchRegistryError> {
        Ok(())
    }
    fn after_change(
        &mut self,
        _solution: &P,
        _change: &Self::Change,
        _registry: &mut MatchRegistry<Self::Score, Self::Justification>,
    ) -> Result<(), MatchRegistryError> {
        Ok(())
    }
    /// Propagate deferred derived state after a problem-edit batch that did not
    /// change entity membership. The default is a no-op: calculators with deferred
    /// work must opt in. Update only derived state, not planning/fact inputs.
    /// Membership edits instead call `reset` with the complete current solution.
    /// This hook must not calculate a score implicitly. Failure requires reset.
    fn flush_changes(
        &mut self,
        _solution: &mut P,
        _registry: &mut MatchRegistry<Self::Score, Self::Justification>,
    ) -> Result<(), MatchRegistryError> {
        Ok(())
    }

    fn calculate_score(&mut self) -> Self::Score;

    /// Fallible scoring with access to optional explanation registration. Override
    /// this when scoring itself discovers matches or can fail. The default keeps
    /// existing calculators unchanged by calling `calculate_score`.
    ///
    /// Called for every session score calculation, including when matching is
    /// disabled. Check `registry.is_enabled()` before registering explanations.
    /// The calculator owns cancellation of earlier matches: scoring does not
    /// clear the registry automatically. Errors and panics invalidate the session
    /// until reset, just like failed change callbacks. Native search directors
    /// also use this hook; cached score reads do not invoke it again.
    fn calculate_score_with_matches(
        &mut self,
        _registry: &mut MatchRegistry<Self::Score, Self::Justification>,
    ) -> Result<Self::Score, MatchRegistryError> {
        Ok(self.calculate_score())
    }
}

/// Owns working data so ordinary mutations cannot bypass calculator callbacks.
/// A callback error or an unwind during mutation leaves the session unavailable
/// until reset. Changes are not rolled back on failure. A calculator that panics
/// in score calculation is likewise unavailable if the caller catches the panic.
pub struct CalculatorSession<P, C: IncrementalCalculator<P>> {
    solution: P,
    calculator: C,
    registry: MatchRegistry<C::Score, C::Justification>,
    ready: bool,
}
impl<P, C: IncrementalCalculator<P>> CalculatorSession<P, C> {
    pub fn new(
        solution: P,
        calculator: C,
        request_matches: bool,
    ) -> Result<Self, MatchRegistryError> {
        let enabled = request_matches && calculator.supports_matches();
        let mut session = Self {
            solution,
            calculator,
            registry: MatchRegistry::new(enabled),
            ready: false,
        };
        session
            .calculator
            .reset(&session.solution, &mut session.registry)?;
        session.ready = true;
        Ok(session)
    }
    /// Internal best-state snapshot. Calculator clones must own independent
    /// mutable scoring state; immutable match records may retain their identity.
    pub(crate) fn snapshot(&self) -> Self
    where
        P: Clone,
        C: Clone,
    {
        Self {
            solution: self.solution.clone(),
            calculator: self.calculator.clone(),
            registry: MatchRegistry {
                enabled: self.registry.enabled,
                owner: self.registry.owner.clone(),
                next_id: self.registry.next_id,
                high_water: self.registry.high_water,
                total: self.registry.total.clone(),
                constraints: self
                    .registry
                    .constraints
                    .iter()
                    .map(|(name, entry)| {
                        (
                            name.clone(),
                            RegisteredConstraint {
                                score: entry.score.clone(),
                                matches: entry.matches.clone(),
                            },
                        )
                    })
                    .collect(),
            },
            ready: self.ready,
        }
    }
    pub(crate) fn restore_snapshot(&mut self, mut snapshot: Self) {
        // Never recycle a registration issued on a discarded branch. Saved
        // calculator handles keep their original owner and record identities.
        snapshot.registry.high_water = snapshot.registry.high_water.max(self.registry.high_water);
        snapshot.registry.next_id = snapshot.registry.high_water;
        *self = snapshot;
    }
    pub(crate) fn restore_failed(&mut self, solution: P) {
        self.ready = false;
        self.solution = solution;
    }
    pub fn solution(&self) -> &P {
        &self.solution
    }
    pub fn calculator(&self) -> &C {
        &self.calculator
    }
    pub(crate) fn require_ready(&self) -> Result<(), MatchRegistryError> {
        if self.ready {
            Ok(())
        } else {
            Err(MatchRegistryError::NeedsReset)
        }
    }
    pub fn registry(
        &self,
    ) -> Result<&MatchRegistry<C::Score, C::Justification>, MatchRegistryError> {
        self.require_ready()?;
        Ok(&self.registry)
    }
    /// Allows explicit registrations/cancellations outside calculator callbacks.
    /// These alter explanation totals, not the calculator's independent score.
    pub fn registry_mut(
        &mut self,
    ) -> Result<&mut MatchRegistry<C::Score, C::Justification>, MatchRegistryError> {
        self.require_ready()?;
        Ok(&mut self.registry)
    }
    pub fn reset(&mut self, solution: P) -> Result<(), MatchRegistryError> {
        self.ready = false;
        self.solution = solution;
        self.reset_current()
    }
    /// Recover or refresh the current owned solution without cloning it or
    /// replacing the calculator. Expires existing match registrations; does not
    /// calculate a score. A failed reset leaves the session unavailable.
    pub fn reset_current(&mut self) -> Result<(), MatchRegistryError> {
        self.ready = false;
        self.registry.reset();
        self.calculator.reset(&self.solution, &mut self.registry)?;
        self.ready = true;
        Ok(())
    }
    pub(crate) fn invalidate(&mut self) {
        self.ready = false;
    }
    pub(crate) fn flush_current(&mut self) -> Result<(), MatchRegistryError> {
        self.require_ready()?;
        self.ready = false;
        self.calculator
            .flush_changes(&mut self.solution, &mut self.registry)?;
        self.ready = true;
        Ok(())
    }
    pub fn change(
        &mut self,
        change: &C::Change,
        update: impl FnOnce(&mut P),
    ) -> Result<(), MatchRegistryError> {
        self.require_ready()?;
        self.ready = false;
        self.calculator
            .before_change(&self.solution, change, &mut self.registry)?;
        update(&mut self.solution);
        self.calculator
            .after_change(&self.solution, change, &mut self.registry)?;
        self.ready = true;
        Ok(())
    }
    pub fn score(&mut self) -> Result<C::Score, MatchRegistryError> {
        self.require_ready()?;
        self.ready = false;
        let score = self
            .calculator
            .calculate_score_with_matches(&mut self.registry)?;
        self.ready = true;
        Ok(score)
    }
}

#[cfg(test)]
mod retained_snapshot_tests {
    use super::*;
    #[derive(Clone)]
    struct Empty;
    impl IncrementalCalculator<()> for Empty {
        type Score = crate::SimpleScore;
        type Justification = ();
        type Change = ();
        fn supports_matches(&self) -> bool {
            true
        }
        fn reset(
            &mut self,
            _: &(),
            _: &mut MatchRegistry<Self::Score, ()>,
        ) -> Result<(), MatchRegistryError> {
            Ok(())
        }
        fn calculate_score(&mut self) -> Self::Score {
            crate::SimpleScore::ZERO
        }
    }
    #[test]
    fn restored_registry_keeps_handles_and_never_reuses_discarded_ids_even_after_reset() {
        let mut session = CalculatorSession::new((), Empty, true).unwrap();
        let kept = session
            .registry_mut()
            .unwrap()
            .register("same", crate::SimpleScore::new(3), ())
            .unwrap();
        let snapshot = session.snapshot();
        let discarded = session
            .registry_mut()
            .unwrap()
            .register("same", crate::SimpleScore::new(7), ())
            .unwrap();
        session.reset(()).unwrap(); // Reset changes owner and restarts ordinary IDs.
        session.restore_snapshot(snapshot);
        let next = session
            .registry_mut()
            .unwrap()
            .register("same", crate::SimpleScore::new(2), ())
            .unwrap();
        assert!(next.id() > discarded.id());
        assert_eq!(
            discarded.cancel(session.registry_mut().unwrap()),
            Err(MatchRegistryError::AlreadyCanceled)
        );
        kept.cancel(session.registry_mut().unwrap()).unwrap();
        assert_eq!(
            *session.registry().unwrap().total().unwrap(),
            crate::SimpleScore::new(2)
        );
        next.cancel(session.registry_mut().unwrap()).unwrap();
        assert_eq!(
            *session.registry().unwrap().total().unwrap(),
            crate::SimpleScore::ZERO
        );
    }
}
