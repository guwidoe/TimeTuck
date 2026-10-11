//! Atomic replacement of explicitly selected explanation snapshots.
use super::*;
use crate::{ScoreOverflow, ScoringArithmetic};
use std::collections::{HashMap, HashSet};

impl<I: Ord + Clone, S: AdditiveScore, J, F: Ord + Clone> ScoreExplanation<I, S, J, F> {
    /// Insert or replace one constraint snapshot, returning its previous snapshot.
    /// Only that constraint's facts and qualified match references are updated;
    /// unrelated justifications are neither cloned nor reconstructed. A retained
    /// zero-score match still contributes an indictment membership.
    ///
    /// This uses checked arithmetic. All public snapshot fields must remain
    /// internally consistent, as produced by the explanation constructors.
    /// Errors and panics during calculation, cloning, or comparison leave this
    /// explanation unchanged. Panicking destructors are not supported.
    /// Existing-fact replacements touch only affected indictment entries. Adding
    /// or removing fact keys also scans the fact-key index; it does not clone
    /// unrelated matches, facts, or justifications.
    ///
    /// ```
    /// use timetuck::*;
    /// let mut b = StreamGraphBuilder::<SimpleScore>::new();
    /// let source = b.source::<usize, i64>();
    /// let cost = b.score("cost", source.port(), |v| SimpleScore::new(-v))?;
    /// let mut graph = b.build();
    /// let mut explanation = ScoreExplanation::default();
    /// for value in [2, 7] {
    ///     let mut tx = graph.transaction();
    ///     tx.apply(&source, TupleChange::put(0, value))?;
    ///     tx.commit()?;
    ///     explanation.replace_constraint(graph.explain_constraint_default(&cost, |_| vec!["job"] )?)?;
    ///     assert_eq!(explanation.score, SimpleScore::new(-value));
    /// }
    /// explanation.remove_constraint("cost")?;
    /// assert!(explanation.indictments.is_empty());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn replace_constraint(
        &mut self,
        report: ConstraintExplanation<I, S, J, F>,
    ) -> Result<Option<ConstraintExplanation<I, S, J, F>>, ScoreOverflow> {
        self.replace_constraint_with_arithmetic(report, ScoringArithmetic::Checked)
    }

    /// Replacement with an explicit arithmetic policy. Checked arithmetic tests
    /// final totals, permitting intermediate overflow that cancels in the same
    /// replacement. Wrapping matches the existing `add_constraint` arithmetic.
    pub fn replace_constraint_with_arithmetic(
        &mut self,
        report: ConstraintExplanation<I, S, J, F>,
        arithmetic: ScoringArithmetic,
    ) -> Result<Option<ConstraintExplanation<I, S, J, F>>, ScoreOverflow> {
        let name = report.name.clone();
        self.update_constraint(&name, Some(report), arithmetic)
    }

    /// Remove a selected constraint and its indictment memberships. An unknown
    /// name returns `None` without touching the explanation or invoking callbacks.
    pub fn remove_constraint(
        &mut self,
        name: &str,
    ) -> Result<Option<ConstraintExplanation<I, S, J, F>>, ScoreOverflow> {
        self.remove_constraint_with_arithmetic(name, ScoringArithmetic::Checked)
    }

    /// Removal with an explicit arithmetic policy; see `replace_constraint`.
    pub fn remove_constraint_with_arithmetic(
        &mut self,
        name: &str,
        arithmetic: ScoringArithmetic,
    ) -> Result<Option<ConstraintExplanation<I, S, J, F>>, ScoreOverflow> {
        self.update_constraint(name, None, arithmetic)
    }

    fn update_constraint(
        &mut self,
        name: &str,
        replacement: Option<ConstraintExplanation<I, S, J, F>>,
        arithmetic: ScoringArithmetic,
    ) -> Result<Option<ConstraintExplanation<I, S, J, F>>, ScoreOverflow> {
        let previous = self.constraints.get(name);
        if previous.is_none() && replacement.is_none() {
            return Ok(None);
        }
        let zero = S::ZERO;
        let score = arithmetic.adjust(
            &self.score,
            [(
                previous.map_or(&zero, |c| &c.score),
                replacement.as_ref().map_or(&zero, |c| &c.score),
            )],
        )?;
        let facts: BTreeSet<_> = previous
            .into_iter()
            .flat_map(|c| c.indictments.keys())
            .chain(replacement.iter().flat_map(|c| c.indictments.keys()))
            .collect();
        let mut updates = Vec::with_capacity(facts.len());
        for fact in facts {
            let old = previous.and_then(|c| c.indictments.get(fact));
            let new = replacement.as_ref().and_then(|c| c.indictments.get(fact));
            let mut indictment =
                self.indictments
                    .get(fact)
                    .cloned()
                    .unwrap_or_else(|| Indictment {
                        score: S::ZERO,
                        matches: BTreeSet::new(),
                    });
            indictment.score = arithmetic.adjust(
                &indictment.score,
                [(
                    old.map_or(&zero, |i| &i.score),
                    new.map_or(&zero, |i| &i.score),
                )],
            )?;
            if let Some(old) = old {
                for id in &old.matches {
                    indictment.matches.remove(&ConstraintMatchRef {
                        constraint: name.to_owned(),
                        tuple: id.clone(),
                    });
                }
            }
            if let Some(new) = new {
                indictment
                    .matches
                    .extend(new.matches.iter().map(|id| ConstraintMatchRef {
                        constraint: name.to_owned(),
                        tuple: id.clone(),
                    }));
            }
            updates.push((fact.clone(), indictment));
        }
        // All user callbacks, arithmetic and match-ID comparisons have finished.
        // Fact-key comparisons during insertion/publication remain guarded.
        let mut edit = IndictmentEdit::new(&mut self.indictments, &updates);
        edit.install(updates);
        let previous = match replacement {
            Some(report) => self.constraints.insert(report.name.clone(), report),
            None => self.constraints.remove(name),
        };
        self.score = score;
        edit.commit();
        Ok(previous)
    }
}

// Tree insertions may relocate keys. Save insertion ranks before editing, then
// insert missing keys in order. A failed insertion leaves a successful prefix;
// rollback removes that prefix by rank without calling user Ord again.
// After insertion finishes, keys stay put until commit. Their addresses serve
// only as opaque identities (never dereferenced) for a comparison-free rollback.
struct IndictmentEdit<'a, F: Ord, I: Ord, S> {
    map: &'a mut BTreeMap<F, Indictment<I, S>>,
    insertion_ranks: Vec<usize>,
    inserted: usize,
    undo: HashMap<usize, Indictment<I, S>>,
    retired: HashSet<usize>,
    complete: bool,
}
impl<'a, F: Ord + Clone, I: Ord, S: AdditiveScore> IndictmentEdit<'a, F, I, S> {
    fn new(map: &'a mut BTreeMap<F, Indictment<I, S>>, updates: &[(F, Indictment<I, S>)]) -> Self {
        let missing: Vec<_> = updates
            .iter()
            .filter(|(f, _)| !map.contains_key(f))
            .map(|(f, _)| f)
            .collect();
        let mut insertion_ranks = Vec::with_capacity(missing.len());
        // Existing-only replacements avoid a scan of unrelated facts.
        if !missing.is_empty() {
            let mut existing = map.keys().peekable();
            let mut rank = 0;
            for fact in missing {
                while existing.peek().is_some_and(|key| *key < fact) {
                    existing.next();
                    rank += 1;
                }
                insertion_ranks.push(rank);
                rank += 1;
            }
        }
        Self {
            map,
            insertion_ranks,
            inserted: 0,
            undo: HashMap::with_capacity(updates.len()),
            retired: HashSet::with_capacity(updates.len()),
            complete: false,
        }
    }
    fn install(&mut self, updates: Vec<(F, Indictment<I, S>)>) {
        for (fact, _) in &updates {
            if !self.map.contains_key(fact) {
                // Clone before entry lookup; comparison panics cannot leave a
                // partially inserted placeholder in the standard BTreeMap.
                self.map.insert(
                    fact.clone(),
                    Indictment {
                        score: S::ZERO,
                        matches: BTreeSet::new(),
                    },
                );
                self.inserted += 1;
            }
        }
        for (fact, value) in updates {
            let address = self
                .map
                .get_key_value(&fact)
                .map(|(key, _)| key as *const F as usize)
                .expect("prepared fact");
            let target = self.map.get_mut(&fact).expect("prepared fact");
            if value.matches.is_empty() {
                self.retired.insert(address);
            }
            self.undo.insert(address, std::mem::replace(target, value));
        }
    }
    fn commit(mut self) {
        if !self.retired.is_empty() {
            let positions: Vec<_> = self
                .map
                .keys()
                .enumerate()
                .filter_map(|(index, key)| {
                    self.retired
                        .contains(&(key as *const F as usize))
                        .then_some(index)
                })
                .collect();
            // Removing entries can relocate other keys, so consume stable
            // traversal positions instead of addresses during the removal pass.
            let mut positions = positions.into_iter().peekable();
            let mut index = 0;
            self.map.retain(|_, _| {
                let remove = positions.peek() == Some(&index);
                if remove {
                    positions.next();
                }
                index += 1;
                !remove
            });
        }
        self.complete = true;
    }
}
impl<F: Ord, I: Ord, S> Drop for IndictmentEdit<'_, F, I, S> {
    fn drop(&mut self) {
        if self.complete {
            return;
        }
        for (key, value) in self.map.iter_mut() {
            if let Some(original) = self.undo.remove(&(key as *const F as usize)) {
                *value = original;
            }
        }
        if self.inserted != 0 {
            let mut ranks = self.insertion_ranks[..self.inserted]
                .iter()
                .copied()
                .peekable();
            let mut rank = 0;
            self.map.retain(|_, _| {
                let remove = ranks.peek() == Some(&rank);
                if remove {
                    ranks.next();
                }
                rank += 1;
                !remove
            });
        }
    }
}
