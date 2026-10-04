//! Owned explanations of retained scoped and stream contributions, built on demand.
use crate::{AdditiveScore, GraphError, StreamConstraint, StreamGraph};
use std::collections::{BTreeMap, BTreeSet};

/// Default justification retains fact order and repetitions independently of the
/// deduplicated facts receiving blame. Scores are exact, including decimal scores.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DefaultJustification<S, F> {
    pub score: S,
    pub facts: Vec<F>,
}

/// One occurrence: equal payloads at different tuple IDs remain separate matches.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExplainedMatch<S, J, F> {
    pub score: S,
    pub justification: J,
    pub indicted_facts: BTreeSet<F>,
}

/// Each match contributes once per fact, even if a tuple repeats that fact.
/// A match indicting several facts contributes its entire score to each of them;
/// consequently summing indictment scores need not equal the solution score.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Indictment<I, S> {
    pub score: S,
    pub matches: BTreeSet<I>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConstraintExplanation<I, S, J, F> {
    pub name: String,
    pub score: S,
    pub matches: BTreeMap<I, ExplainedMatch<S, J, F>>,
    pub indictments: BTreeMap<F, Indictment<I, S>>,
}

/// Match identity remains qualified by its constraint when aggregating reports.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ConstraintMatchRef<I> {
    pub constraint: String,
    pub tuple: I,
}

/// Aggregation of explicitly selected constraint snapshots. Capture the component
/// snapshots at the same graph state. This does not include structural penalties
/// added outside the graph or constraints the caller has not supplied.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScoreExplanation<I, S, J, F> {
    pub score: S,
    pub constraints: BTreeMap<String, ConstraintExplanation<I, S, J, F>>,
    pub indictments: BTreeMap<F, Indictment<ConstraintMatchRef<I>, S>>,
}
impl<I: Ord + Clone, S: AdditiveScore, J, F: Ord + Clone> Default for ScoreExplanation<I, S, J, F> {
    fn default() -> Self {
        Self {
            score: S::ZERO,
            constraints: BTreeMap::new(),
            indictments: BTreeMap::new(),
        }
    }
}
impl<I: Ord + Clone, S: AdditiveScore, J, F: Ord + Clone> ScoreExplanation<I, S, J, F> {
    /// Reject a duplicate name without altering the existing report.
    pub fn add_constraint(
        &mut self,
        report: ConstraintExplanation<I, S, J, F>,
    ) -> Result<(), GraphError> {
        if self.constraints.contains_key(&report.name) {
            return Err(GraphError::DuplicateConstraint(report.name));
        }
        for (fact, indictment) in &report.indictments {
            let combined = self
                .indictments
                .entry(fact.clone())
                .or_insert_with(|| Indictment {
                    score: S::ZERO,
                    matches: BTreeSet::new(),
                });
            combined.score = combined.score.clone() + indictment.score.clone();
            combined
                .matches
                .extend(indictment.matches.iter().map(|id| ConstraintMatchRef {
                    constraint: report.name.clone(),
                    tuple: id.clone(),
                }));
        }
        self.score = self.score.clone() + report.score.clone();
        self.constraints.insert(report.name.clone(), report);
        Ok(())
    }
}

impl<I: Ord, S, J, F: Ord> ConstraintExplanation<I, S, J, F> {
    /// Normalize differing tuple-ID types for aggregation (for example scalar IDs
    /// and join pairs). Return the original IDs in index order so no identity is
    /// lost. Indictment references are rewritten to these same indices.
    pub fn into_indexed(self) -> (Vec<I>, ConstraintExplanation<usize, S, J, F>) {
        let mut identities = Vec::with_capacity(self.matches.len());
        let matches = self
            .matches
            .into_iter()
            .enumerate()
            .map(|(index, (id, row))| {
                identities.push(id);
                (index, row)
            })
            .collect();
        let indictments = self
            .indictments
            .into_iter()
            .map(|(fact, indictment)| {
                (
                    fact,
                    Indictment {
                        score: indictment.score,
                        matches: indictment
                            .matches
                            .into_iter()
                            .map(|id| {
                                identities
                                    .binary_search(&id)
                                    .expect("indictment references a retained match")
                            })
                            .collect(),
                    },
                )
            })
            .collect();
        (
            identities,
            ConstraintExplanation {
                name: self.name,
                score: self.score,
                matches,
                indictments,
            },
        )
    }
}

impl<S: AdditiveScore + 'static> StreamGraph<'_, S> {
    /// Snapshot a constraint with caller-defined justifications and blame facts.
    /// The callback receives the retained score; scoring callbacks are never run.
    /// It runs once per live match in tuple-ID order, including zero-score matches.
    /// Justifications and blamed facts may differ. No callbacks are retained by the
    /// graph, and requesting an explanation cannot change incremental state.
    pub fn explain_constraint<I: Ord + Clone + 'static, T: 'static, J, F: Ord + Clone>(
        &self,
        constraint: &StreamConstraint<I, T>,
        mut explain: impl FnMut(&T, &S) -> (J, Vec<F>),
    ) -> Result<ConstraintExplanation<I, S, J, F>, GraphError> {
        let rows = self.constraint_matches(constraint)?;
        let mut report = ConstraintExplanation {
            name: self.constraint_name(constraint)?.to_owned(),
            score: S::ZERO,
            matches: BTreeMap::new(),
            indictments: BTreeMap::new(),
        };
        for (id, (tuple, score)) in rows.iter() {
            let (justification, facts) = explain(tuple, score);
            let facts: BTreeSet<_> = facts.into_iter().collect();
            for fact in &facts {
                let entry = report
                    .indictments
                    .entry(fact.clone())
                    .or_insert_with(|| Indictment {
                        score: S::ZERO,
                        matches: BTreeSet::new(),
                    });
                entry.score = entry.score.clone() + score.clone();
                entry.matches.insert(id.clone());
            }
            report.score = report.score.clone() + score.clone();
            report.matches.insert(
                id.clone(),
                ExplainedMatch {
                    score: score.clone(),
                    justification,
                    indicted_facts: facts,
                },
            );
        }
        Ok(report)
    }

    /// Preserve the fact list as the justification; indict each distinct fact once.
    /// Use stable fact identities (or an enum of identities) for mixed tuple types.
    ///
    /// ```
    /// use timetuck::*;
    /// let mut b = StreamGraphBuilder::<SimpleScore>::new();
    /// let jobs = b.source::<usize, (u32, i64)>();
    /// let cost = b.score("cost", jobs.port(), |(_, cost)| SimpleScore::new(-cost))?;
    /// let mut g = b.build();
    /// let mut tx = g.transaction();
    /// tx.apply(&jobs, TupleChange::put(0, (7, 12)))?;
    /// tx.commit()?;
    /// let report = g.explain_constraint_default(&cost, |(machine, _)| vec![*machine])?;
    /// assert_eq!(report.indictments[&7].score, SimpleScore::new(-12));
    /// assert_eq!(report.matches[&0].justification.facts, vec![7]);
    /// # Ok::<(), GraphError>(())
    /// ```
    pub fn explain_constraint_default<I: Ord + Clone + 'static, T: 'static, F: Ord + Clone>(
        &self,
        constraint: &StreamConstraint<I, T>,
        mut facts: impl FnMut(&T) -> Vec<F>,
    ) -> Result<ConstraintExplanation<I, S, DefaultJustification<S, F>, F>, GraphError> {
        self.explain_constraint(constraint, |tuple, score| {
            let facts = facts(tuple);
            (
                DefaultJustification {
                    score: score.clone(),
                    facts: facts.clone(),
                },
                facts,
            )
        })
    }
}

/// A complete scoped director snapshot. `score` includes structural state;
/// `matches.score` sums only ordinary constraint contributions. Each active scoped
/// callback is represented by one aggregate match, not inferred inner matches.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScopedScoreExplanation<S, J, F> {
    pub score: S,
    pub matches: ScoreExplanation<usize, S, J, F>,
}

pub(crate) fn scoped_contribution<S: AdditiveScore, J, F: Ord + Clone>(
    name: &str,
    score: S,
    explanation: Option<(J, Vec<F>)>,
) -> ConstraintExplanation<usize, S, J, F> {
    let mut matches = BTreeMap::new();
    let mut indictments = BTreeMap::new();
    if let Some((justification, facts)) = explanation {
        let facts: BTreeSet<_> = facts.into_iter().collect();
        for fact in &facts {
            indictments.insert(
                fact.clone(),
                Indictment {
                    score: score.clone(),
                    matches: BTreeSet::from([0]),
                },
            );
        }
        matches.insert(
            0,
            ExplainedMatch {
                score: score.clone(),
                justification,
                indicted_facts: facts,
            },
        );
    }
    ConstraintExplanation {
        name: name.to_owned(),
        score,
        matches,
        indictments,
    }
}

/// Before/after snapshots distinguish additions and removals from changes whose
/// score delta is zero (for example a changed justification or set of blamed facts).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExplanationChange<T, S> {
    pub before: Option<T>,
    pub after: Option<T>,
    pub score_delta: S,
}

/// A retained before/after change to one constraint.
pub type ConstraintExplanationChange<I, S, J, F> =
    ExplanationChange<ConstraintExplanation<I, S, J, F>, S>;

/// Difference from a baseline to a current explanation. Only changed constraints
/// and indictments are retained. There is no rescoring or callback invocation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScoreExplanationDiff<I, S, J, F> {
    pub score_delta: S,
    pub constraints: BTreeMap<String, ConstraintExplanationChange<I, S, J, F>>,
    pub indictments: BTreeMap<F, ExplanationChange<Indictment<ConstraintMatchRef<I>, S>, S>>,
}
impl<I: Ord + Clone, S: AdditiveScore, J: Clone + PartialEq, F: Ord + Clone>
    ScoreExplanation<I, S, J, F>
{
    /// Compare `self` against `baseline`; deltas are current minus baseline.
    /// Match IDs must retain the same meaning across snapshots. Independently
    /// indexed reports should first align their original identities. Differences
    /// include justification/blame/occurrence changes even when scores agree.
    pub fn diff(&self, baseline: &Self) -> ScoreExplanationDiff<I, S, J, F> {
        ScoreExplanationDiff {
            score_delta: self.score.clone() - baseline.score.clone(),
            constraints: changed_entries(&baseline.constraints, &self.constraints, |c| {
                c.score.clone()
            }),
            indictments: changed_entries(&baseline.indictments, &self.indictments, |i| {
                i.score.clone()
            }),
        }
    }
}
fn changed_entries<K: Ord + Clone, T: Clone + PartialEq, S: AdditiveScore>(
    before: &BTreeMap<K, T>,
    after: &BTreeMap<K, T>,
    score: impl Fn(&T) -> S,
) -> BTreeMap<K, ExplanationChange<T, S>> {
    before
        .keys()
        .chain(after.keys())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .filter_map(|key| {
            let old = before.get(key);
            let new = after.get(key);
            (old != new).then(|| {
                (
                    key.clone(),
                    ExplanationChange {
                        before: old.cloned(),
                        after: new.cloned(),
                        score_delta: new.map_or(S::ZERO, &score) - old.map_or(S::ZERO, &score),
                    },
                )
            })
        })
        .collect()
}
