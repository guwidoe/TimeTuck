//! Optional identity-bearing occurrences backed by owned score explanations.
use crate::{
    AdditiveScore, ConstraintExplanation, DefaultJustification, ExplainedMatch, Indictment,
    ScoreExplanation, ScoreOverflow, ScoringArithmetic,
};
use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
    hash::{Hash, Hasher},
    sync::Arc,
};

/// Unique occurrence identity; cloning preserves identity. Its ordering is only
/// for native maps, never a diagnostic payload order. Live identities cannot
/// collide, even across independent ledgers or after an occurrence is removed.
#[derive(Clone, Debug)]
pub struct ConstraintOccurrenceId(Arc<()>);
impl PartialEq for ConstraintOccurrenceId {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for ConstraintOccurrenceId {}
impl Hash for ConstraintOccurrenceId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.0).hash(state)
    }
}
impl PartialOrd for ConstraintOccurrenceId {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for ConstraintOccurrenceId {
    fn cmp(&self, other: &Self) -> Ordering {
        Arc::as_ptr(&self.0).cmp(&Arc::as_ptr(&other.0))
    }
}

/// Correspondence between a retained source tuple and its imported occurrence.
#[derive(Clone, Debug)]
pub struct ConstraintOccurrenceBinding<I> {
    pub constraint: String,
    pub tuple: I,
    pub occurrence: ConstraintOccurrenceId,
}

/// An immutable occurrence snapshot. Equal payloads retain distinct identities;
/// updates preserve the identity while previously returned handles keep their
/// historical payload. Payload equality on ExplainedMatch remains unchanged.
#[derive(Clone, Debug)]
pub struct ConstraintOccurrence<S, J, F> {
    id: ConstraintOccurrenceId,
    name: String,
    payload: ExplainedMatch<S, J, F>,
}
impl<S, J, F> ConstraintOccurrence<S, J, F> {
    pub fn id(&self) -> &ConstraintOccurrenceId {
        &self.id
    }
    pub fn constraint_name(&self) -> &str {
        &self.name
    }
    pub fn payload(&self) -> &ExplainedMatch<S, J, F> {
        &self.payload
    }
}
impl<S, J, F> PartialEq for ConstraintOccurrence<S, J, F> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl<S, J, F> Eq for ConstraintOccurrence<S, J, F> {}
impl<S, J, F> Hash for ConstraintOccurrence<S, J, F> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state)
    }
}
impl<S: Ord, F: Ord> ConstraintOccurrence<S, DefaultJustification<S, F>, F> {
    /// Name, score, ordered-fact count, then ordered facts. Distinct identities
    /// may compare equal, so this deliberately does not implement Rust Ord.
    pub fn diagnostic_cmp(&self, other: &Self) -> Ordering {
        self.name
            .cmp(&other.name)
            .then_with(|| self.payload.score.cmp(&other.payload.score))
            .then_with(|| {
                self.payload
                    .justification
                    .facts
                    .len()
                    .cmp(&other.payload.justification.facts.len())
            })
            .then_with(|| {
                self.payload
                    .justification
                    .facts
                    .cmp(&other.payload.justification.facts)
            })
    }
}

/// Borrowed live total view; key identity ignores score and occurrence contents.
#[derive(Debug)]
pub struct ConstraintOccurrenceTotal<'a, S, J, F>(
    &'a ConstraintExplanation<ConstraintOccurrenceId, S, J, F>,
);
impl<S, J, F> ConstraintOccurrenceTotal<'_, S, J, F> {
    pub fn name(&self) -> &str {
        &self.0.name
    }
    pub fn score(&self) -> &S {
        &self.0.score
    }
    pub fn explanation(&self) -> &ConstraintExplanation<ConstraintOccurrenceId, S, J, F> {
        self.0
    }
}
impl<S, J, F> PartialEq for ConstraintOccurrenceTotal<'_, S, J, F> {
    fn eq(&self, other: &Self) -> bool {
        self.name() == other.name()
    }
}
impl<S, J, F> Eq for ConstraintOccurrenceTotal<'_, S, J, F> {}
impl<S, J, F> Hash for ConstraintOccurrenceTotal<'_, S, J, F> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name().hash(state)
    }
}
impl<S, J, F> PartialOrd for ConstraintOccurrenceTotal<'_, S, J, F> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl<S, J, F> Ord for ConstraintOccurrenceTotal<'_, S, J, F> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.name().cmp(other.name())
    }
}

/// On-demand occurrence ledger. Every handle refers to a contribution retained
/// in the actual ScoreExplanation. Adding, replacing and removing rebuild only
/// the selected constraint's exact total and deduplicated indictment memberships,
/// then use the existing checked explanation publication path. Zero totals stay
/// visible after their last match is removed. Ordinary graph scoring is unchanged.
/// Import creates fresh native identities; Java object or graph tuple identity is
/// not claimed. Equal-payload occurrences never collapse. Rebuild cost scales
/// with the selected matches and indictment memberships; this is an optional API.
#[derive(Clone, Debug)]
pub struct ConstraintOccurrenceLedger<S, J, F> {
    explanation: ScoreExplanation<ConstraintOccurrenceId, S, J, F>,
    occurrences: BTreeMap<ConstraintOccurrenceId, ConstraintOccurrence<S, J, F>>,
}
impl<S: AdditiveScore, J: Clone, F: Ord + Clone> Default for ConstraintOccurrenceLedger<S, J, F> {
    fn default() -> Self {
        Self {
            explanation: ScoreExplanation::default(),
            occurrences: BTreeMap::new(),
        }
    }
}
impl<S: AdditiveScore, J: Clone, F: Ord + Clone> ConstraintOccurrenceLedger<S, J, F> {
    pub fn explanation(&self) -> &ScoreExplanation<ConstraintOccurrenceId, S, J, F> {
        &self.explanation
    }
    pub fn total(&self, name: &str) -> Option<ConstraintOccurrenceTotal<'_, S, J, F>> {
        self.explanation
            .constraints
            .get(name)
            .map(ConstraintOccurrenceTotal)
    }
    pub fn lookup(&self, id: &ConstraintOccurrenceId) -> Option<&ConstraintOccurrence<S, J, F>> {
        self.occurrences.get(id)
    }
    /// Import actual retained contribution payloads without running scoring.
    /// Empty selected constraints remain present. Input tuple IDs are returned
    /// with their newly allocated handles so callers can retain the correspondence.
    pub fn from_explanation<I: Ord + Clone>(
        source: &ScoreExplanation<I, S, J, F>,
    ) -> Result<(Self, Vec<ConstraintOccurrenceBinding<I>>), ScoreOverflow> {
        let mut ledger = Self::default();
        let mut bindings = vec![];
        for (name, report) in &source.constraints {
            let mut matches = BTreeMap::new();
            let mut handles = Vec::with_capacity(report.matches.len());
            for (tuple, payload) in &report.matches {
                let handle = ConstraintOccurrence {
                    id: ConstraintOccurrenceId(Arc::new(())),
                    name: name.clone(),
                    payload: payload.clone(),
                };
                matches.insert(handle.id.clone(), handle.payload.clone());
                bindings.push(ConstraintOccurrenceBinding {
                    constraint: name.clone(),
                    tuple: tuple.clone(),
                    occurrence: handle.id.clone(),
                });
                handles.push(handle);
            }
            // Stage every user Clone before checked publication, and rebuild
            // this selected constraint only once rather than once per match.
            ledger.publish(name, matches)?;
            for handle in handles {
                ledger.occurrences.insert(handle.id.clone(), handle);
            }
        }
        Ok((ledger, bindings))
    }
    pub fn ensure_total(&mut self, name: &str) -> Result<(), ScoreOverflow> {
        if !self.explanation.constraints.contains_key(name) {
            self.publish(name, BTreeMap::new())?;
        }
        Ok(())
    }
    pub fn add(
        &mut self,
        name: &str,
        payload: ExplainedMatch<S, J, F>,
    ) -> Result<ConstraintOccurrence<S, J, F>, ScoreOverflow> {
        let handle = ConstraintOccurrence {
            id: ConstraintOccurrenceId(Arc::new(())),
            name: name.into(),
            payload,
        };
        let mut matches = self
            .explanation
            .constraints
            .get(name)
            .map_or_else(BTreeMap::new, |r| r.matches.clone());
        matches.insert(handle.id.clone(), handle.payload.clone());
        let retained = handle.clone();
        self.publish(name, matches)?;
        self.occurrences.insert(handle.id.clone(), retained);
        Ok(handle)
    }
    pub fn replace(
        &mut self,
        id: &ConstraintOccurrenceId,
        payload: ExplainedMatch<S, J, F>,
    ) -> Result<Option<ConstraintOccurrence<S, J, F>>, ScoreOverflow> {
        let Some(old) = self.occurrences.get(id) else {
            return Ok(None);
        };
        let name = old.name.clone();
        let handle = ConstraintOccurrence {
            id: id.clone(),
            name: name.clone(),
            payload,
        };
        let mut matches = self.explanation.constraints[&name].matches.clone();
        matches.insert(id.clone(), handle.payload.clone());
        self.publish(&name, matches)?;
        Ok(self.occurrences.insert(id.clone(), handle))
    }
    pub fn remove(
        &mut self,
        id: &ConstraintOccurrenceId,
    ) -> Result<Option<ConstraintOccurrence<S, J, F>>, ScoreOverflow> {
        let Some(old) = self.occurrences.get(id) else {
            return Ok(None);
        };
        let name = old.name.clone();
        let mut matches = self.explanation.constraints[&name].matches.clone();
        matches.remove(id);
        self.publish(&name, matches)?;
        Ok(self.occurrences.remove(id))
    }
    fn publish(
        &mut self,
        name: &str,
        matches: BTreeMap<ConstraintOccurrenceId, ExplainedMatch<S, J, F>>,
    ) -> Result<(), ScoreOverflow> {
        let zero = S::ZERO;
        let score = ScoringArithmetic::Checked
            .adjust(&zero, matches.values().map(|m| (&zero, &m.score)))?;
        let mut members: BTreeMap<F, BTreeSet<ConstraintOccurrenceId>> = BTreeMap::new();
        for (id, payload) in &matches {
            for fact in &payload.indicted_facts {
                members.entry(fact.clone()).or_default().insert(id.clone());
            }
        }
        let indictments = members
            .into_iter()
            .map(|(fact, ids)| {
                let score = ScoringArithmetic::Checked
                    .adjust(&zero, ids.iter().map(|id| (&zero, &matches[id].score)))?;
                Ok((
                    fact,
                    Indictment {
                        score,
                        matches: ids,
                    },
                ))
            })
            .collect::<Result<_, ScoreOverflow>>()?;
        self.explanation.replace_constraint(ConstraintExplanation {
            name: name.into(),
            score,
            matches,
            indictments,
        })?;
        Ok(())
    }
}
