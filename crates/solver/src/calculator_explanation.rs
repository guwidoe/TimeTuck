//! On-demand explanations of registered calculator matches, without rescoring.
use crate::*;
use std::collections::{BTreeMap, BTreeSet};

/// The calculator's cached score and its independently registered contributions.
/// `score` includes required-variable structure; `matches.score` does not. A
/// calculator may deliberately register only some contributions, so equality is
/// reported rather than assumed. Match IDs are stable only until registry reset;
/// compare snapshots with `matches.diff` only within the same registry generation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CalculatorScoreExplanation<S, J, F> {
    pub score: S,
    pub matches: ScoreExplanation<usize, S, J, F>,
}
impl<S: AdditiveScore, J, F> CalculatorScoreExplanation<S, J, F> {
    pub fn matches_agree(&self) -> bool {
        self.score.clone().with_structural(0) == self.matches.score
    }
}

/// Snapshot every live registration, retaining empty constraints and zero-score
/// matches. Calls `explain` once per occurrence, in constraint-name then match-ID
/// order. The callback can map arbitrary stored justifications to a different
/// owned justification and fact identities; stored justifications need not Clone.
/// Repeated facts receive a match's score only once. Different equal occurrences
/// remain separate. All fact totals use checked arithmetic, including cross-
/// constraint totals that can overflow even when the registered grand total fits.
/// An error or callback unwind leaves the registry unchanged.
///
/// ```
/// use timetuck::*;
/// let mut registry = MatchRegistry::new(true);
/// let handle = registry.register("overlap", SimpleScore::new(-4), vec![7, 7, 9])?;
/// let report = explain_calculator_matches_default(&registry, |m| m.justification.clone())?;
/// assert_eq!(report.indictments[&7].score, SimpleScore::new(-4));
/// assert_eq!(report.constraints["overlap"].matches[&handle.id()].justification.facts,
///     vec![7, 7, 9]);
/// # Ok::<(), MatchRegistryError>(())
/// ```
pub fn explain_calculator_matches<S: AdditiveScore, J, O, F: Ord + Clone>(
    registry: &MatchRegistry<S, J>,
    mut explain: impl FnMut(&RegisteredMatch<S, J>) -> (O, Vec<F>),
) -> Result<ScoreExplanation<usize, S, O, F>, MatchRegistryError> {
    let source = registry.constraints()?;
    let mut report = ScoreExplanation {
        score: registry.total()?.clone(),
        constraints: BTreeMap::new(),
        indictments: BTreeMap::new(),
    };
    for (name, constraint) in source {
        let mut part = ConstraintExplanation {
            name: name.clone(),
            score: constraint.score().clone(),
            matches: BTreeMap::new(),
            indictments: BTreeMap::new(),
        };
        for (&id, record) in constraint.matches() {
            let (justification, facts) = explain(record);
            let facts: BTreeSet<_> = facts.into_iter().collect();
            for fact in &facts {
                part.indictments
                    .entry(fact.clone())
                    .or_insert_with(|| Indictment {
                        score: S::ZERO,
                        matches: BTreeSet::new(),
                    })
                    .matches
                    .insert(id);
                report
                    .indictments
                    .entry(fact.clone())
                    .or_insert_with(|| Indictment {
                        score: S::ZERO,
                        matches: BTreeSet::new(),
                    })
                    .matches
                    .insert(ConstraintMatchRef {
                        constraint: name.clone(),
                        tuple: id,
                    });
            }
            part.matches.insert(
                id,
                ExplainedMatch {
                    score: record.score.clone(),
                    justification,
                    indicted_facts: facts,
                },
            );
        }
        for indictment in part.indictments.values_mut() {
            indictment.score =
                S::checked_sum(indictment.matches.iter().map(|id| &part.matches[id].score))
                    .ok_or(MatchRegistryError::ScoreOverflow)?;
        }
        report.constraints.insert(name.clone(), part);
    }
    for indictment in report.indictments.values_mut() {
        indictment.score = S::checked_sum(
            indictment
                .matches
                .iter()
                .map(|id| &report.constraints[&id.constraint].matches[&id.tuple].score),
        )
        .ok_or(MatchRegistryError::ScoreOverflow)?;
    }
    Ok(report)
}

/// Preserve fact order and duplicates in the default justification, while
/// indicting each distinct fact once per match. No scoring callback is run.
#[allow(clippy::type_complexity)]
pub fn explain_calculator_matches_default<S: AdditiveScore, J, F: Ord + Clone>(
    registry: &MatchRegistry<S, J>,
    mut facts: impl FnMut(&RegisteredMatch<S, J>) -> Vec<F>,
) -> Result<ScoreExplanation<usize, S, DefaultJustification<S, F>, F>, MatchRegistryError> {
    explain_calculator_matches(registry, |record| {
        let facts = facts(record);
        (
            DefaultJustification {
                score: record.score.clone(),
                facts: facts.clone(),
            },
            facts,
        )
    })
}

macro_rules! director_explanation {
    ($director:ident, [$($v:ident),+], $assignment:ty, $change:ty, $error:ident) => {
        impl<$($v: Eq + Clone,)+ C, Factory> $director<'_, $($v,)+ C, Factory>
        where C: IncrementalCalculator<$assignment, Change = $change>, Factory: Fn() -> C {
            /// Explain the cached score and live registrations without calculating
            /// score or resetting the calculator. Disabled matching and unhealthy
            /// directors return an error before invoking the explanation callback.
            pub fn explain<J, F: Ord + Clone>(
                &self,
                explain: impl FnMut(&RegisteredMatch<C::Score, C::Justification>) -> (J, Vec<F>),
            ) -> Result<CalculatorScoreExplanation<C::Score, J, F>, $error> {
                let score = self.score()?;
                let matches = explain_calculator_matches(self.registry()?, explain)
                    .map_err($error::Calculator)?;
                Ok(CalculatorScoreExplanation { score, matches })
            }
            /// Build default justifications from fact identities. Repetitions are
            /// retained in justifications and deduplicated only for indictments.
            #[allow(clippy::type_complexity)]
            pub fn explain_default<F: Ord + Clone>(
                &self,
                facts: impl FnMut(&RegisteredMatch<C::Score, C::Justification>) -> Vec<F>,
            ) -> Result<CalculatorScoreExplanation<C::Score, DefaultJustification<C::Score, F>, F>, $error> {
                let score = self.score()?;
                let matches = explain_calculator_matches_default(self.registry()?, facts)
                    .map_err($error::Calculator)?;
                Ok(CalculatorScoreExplanation { score, matches })
            }
        }
    };
}
director_explanation!(
    CalculatorScoreDirector,
    [V],
    Assignment<V>,
    Vec<usize>,
    ScoringError
);
director_explanation!(
    ListCalculatorScoreDirector,
    [V],
    ListAssignment<V>,
    Vec<usize>,
    ListScoringError
);
director_explanation!(MixedCalculatorScoreDirector, [B, L], MixedAssignment<B, L>, MixedCalculatorChange, MixedScoringError);
