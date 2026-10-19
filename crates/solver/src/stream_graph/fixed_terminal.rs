//! Dense scoring for a caller-declared, immutable occurrence-ID universe.
use super::fixed_slots::{SlotSnapshot, Slots};
use super::*;
use std::cell::OnceCell;
struct FixedTerminal<'a, I, T, S> {
    ids: Vec<I>,
    positions: BTreeMap<I, usize>,
    // Validated integer ordinals only; generic/custom ordering keeps the tree.
    direct_ordinals: bool,
    rows: Slots<(T, S)>,
    materialized: OnceCell<SnapshotMap<I, (T, S)>>,
    evaluate: TerminalEvaluation<'a, T, S>,
    total: S,
    checkpoint_box: CheckpointBox<TerminalJournal<S>>,
    name: String,
}
// Primitive planning IDs and fixed relationship IDs can expose a zero-based
// ordinal. Type identity excludes user-defined Ord/Eq/Clone implementations.
// Relationship endpoints still participate in the full-ID validation below.
fn numeric_ordinal<I: 'static>(id: &I) -> Option<usize> {
    let id = id as &dyn Any;
    if let Some(id) = id.downcast_ref::<usize>() {
        Some(*id)
    } else {
        id.downcast_ref::<((usize, usize), usize)>()
            .map(|id| id.0.0)
    }
}
fn supports_numeric_ordinal<I: 'static>() -> bool {
    std::any::TypeId::of::<I>() == std::any::TypeId::of::<usize>()
        || std::any::TypeId::of::<I>() == std::any::TypeId::of::<((usize, usize), usize)>()
}
fn contiguous_ordinals<I: 'static>(ids: &[I]) -> bool {
    supports_numeric_ordinal::<I>()
        && !ids.is_empty()
        && ids
            .iter()
            .enumerate()
            .all(|(slot, id)| numeric_ordinal(id) == Some(slot))
}
impl<I: Ord + 'static, T, S> FixedTerminal<'_, I, T, S> {
    fn position(&self, id: &I) -> Option<usize> {
        if supports_numeric_ordinal::<I>() && self.direct_ordinals {
            let slot = numeric_ordinal(id)?;
            // Keep the complete relationship identity: an in-range relationship
            // ordinal with different endpoints remains an unknown occurrence.
            self.ids
                .get(slot)
                .filter(|stored| *stored == id)
                .map(|_| slot)
        } else {
            self.positions.get(id).copied()
        }
    }
}
impl<I: Ord + Clone + 'static, T: Clone + 'static, S: AdditiveScore + 'static> Node<S>
    for FixedTerminal<'_, I, T, S>
{
    fn constraint_weight(&self) -> Option<&S> {
        match &self.evaluate {
            TerminalEvaluation::Weighted { weight, .. } => Some(weight),
            _ => None,
        }
    }
    fn replace_weight(&mut self, new: S) {
        match &mut self.evaluate {
            TerminalEvaluation::Weighted { weight, .. } => *weight = new,
            _ => unreachable!("unweighted terminal"),
        }
    }
    fn evaluates_score(&self, event: &Event) -> bool {
        typed::<I, T>(event).value.is_some()
    }
    fn score(&self) -> S {
        self.total.clone()
    }
    fn clear(&mut self) {
        self.rows.clear();
        self.total = S::ZERO;
        self.materialized.take();
    }
    fn checkpoint(&self) -> Box<dyn Any> {
        Box::new((self.rows.snapshot(), self.total.clone()))
    }
    fn begin_checkpoint(&mut self) -> Box<dyn Any> {
        let total = self.total.clone();
        let saved = TerminalJournal {
            rows: self.rows.begin(),
            total,
        };
        self.checkpoint_box.begin(saved)
    }
    fn commit_checkpoint(&mut self, saved: Box<dyn Any>) {
        let (saved, empty) =
            CheckpointBox::<TerminalJournal<S>>::unpack(saved).expect("fixed terminal journal");
        self.rows.commit(saved.rows);
        drop(saved);
        self.checkpoint_box.recycle(empty);
    }
    fn restore(&mut self, saved: Box<dyn Any>) {
        match CheckpointBox::<TerminalJournal<S>>::unpack(saved) {
            Ok((saved, empty)) => {
                self.rows.rollback(saved.rows);
                self.total = saved.total;
                self.checkpoint_box.recycle(empty);
            }
            Err(saved) => {
                let (rows, total) = *saved
                    .downcast::<(SlotSnapshot<(T, S)>, S)>()
                    .expect("fixed terminal snapshot");
                self.rows.install(rows);
                self.total = total;
            }
        }
        self.materialized.take();
    }
    fn scores_match(&self, checkpoint: &dyn Any) -> bool {
        let view;
        let (old, total) = if let Some(saved) = CheckpointBox::<TerminalJournal<S>>::get(checkpoint)
        {
            view = self.rows.entry_snapshot(saved.rows);
            (&view, &saved.total)
        } else {
            let (old, total) = checkpoint
                .downcast_ref::<(SlotSnapshot<(T, S)>, S)>()
                .expect("fixed terminal snapshot");
            (old, total)
        };
        self.total == *total
            && self
                .rows
                .iter()
                .map(|v| v.map(|(_, score)| score))
                .eq(old.iter().map(|v| v.as_deref().map(|(_, score)| score)))
    }
    fn is_constraint(&self) -> bool {
        true
    }
    fn matches(&self) -> Option<&dyn Any> {
        Some(self.materialized.get_or_init(|| {
            let mut matches = SnapshotMap::new();
            for (id, row) in self.ids.iter().zip(self.rows.iter()) {
                if let Some(row) = row {
                    matches.insert(id.clone(), row.clone());
                }
            }
            matches
        }))
    }
    fn apply(
        &mut self,
        _: bool,
        event: &Event,
        arithmetic: NodeArithmetic,
    ) -> Result<Outputs, GraphError> {
        let change = typed::<I, T>(event);
        let slot = self.position(&change.id).ok_or_else(|| {
            GraphError::Dependency(format!(
                "occurrence ID outside fixed universe of constraint {}",
                self.name
            ))
        })?;
        let contribution = change
            .value
            .as_ref()
            .map(|value| self.evaluate.evaluate(value, &self.name, arithmetic))
            .transpose()?;
        if contribution.as_ref().is_some_and(|s| s.structural() != 0) {
            return Err(GraphError::StructuralContribution(self.name.clone()));
        }
        let old = self
            .rows
            .get(slot)
            .map_or(S::ZERO, |(_, score)| score.clone());
        let new = contribution.clone().unwrap_or(S::ZERO);
        let total = arithmetic
            .totals
            .adjust(&self.total, [(&old, &new)])
            .map_err(|_| GraphError::ScoreOverflow {
                constraint: Some(self.name.clone()),
            })?;
        let row = change
            .value
            .as_ref()
            .map(|v| (v.clone(), contribution.expect("scored match")));
        self.rows.set(slot, row);
        self.total = total;
        self.materialized.take();
        Ok(Outputs::new())
    }
}
impl<'a, S: AdditiveScore + 'static> StreamGraphBuilder<'a, S> {
    /// Score a stream with a caller-declared immutable set of possible occurrence
    /// IDs. Duplicate declared IDs and events outside that set are errors. IDs
    /// from the declaration are the canonical keys used for match inspection.
    ///
    /// Dense rows and move-based undo replace tree updates. Capturing a saved
    /// candidate copies O(declared IDs) shared row pointers, without cloning row
    /// payloads. This favors small, fixed universes with frequent trials.
    /// Match inspection lazily builds the ordinary ordered SnapshotMap and may
    /// clone IDs/payloads/scores; it is cached until state changes. Retained match
    /// snapshots remain independent. Checked arithmetic, callback order, graph
    /// rollback and consistency diagnostics behave as for `score`.
    pub fn score_fixed<I: Ord + Clone + 'static, T: Clone + 'static>(
        &mut self,
        name: impl Into<String>,
        input: &StreamPort<I, T>,
        ids: impl IntoIterator<Item = I>,
        evaluate: impl Fn(&T) -> S + 'a,
    ) -> Result<StreamConstraint<I, T>, GraphError> {
        self.register_fixed(
            name.into(),
            input,
            ids,
            TerminalEvaluation::Direct(Box::new(evaluate)),
        )
    }
    /// Configurable integer-weighted counterpart of `score_fixed`. Zero-weight
    /// pruning and named overrides use ordinary graph rules.
    pub fn score_fixed_weighted<I: Ord + Clone + 'static, T: Clone + 'static>(
        &mut self,
        name: impl Into<String>,
        input: &StreamPort<I, T>,
        ids: impl IntoIterator<Item = I>,
        weight: S,
        factor: impl Fn(&T) -> i64 + 'a,
    ) -> Result<StreamConstraint<I, T>, GraphError> {
        self.register_fixed(
            name.into(),
            input,
            ids,
            TerminalEvaluation::Weighted {
                weight,
                impact: Box::new(move |v, w, a| Ok(a.scale(w, factor(v))?)),
            },
        )
    }
    /// Exact decimal-weighted counterpart of `score_fixed`, with the same fixed
    /// ID universe and lazy match-inspection contract.
    pub fn score_fixed_weighted_decimal<I: Ord + Clone + 'static, T: Clone + 'static>(
        &mut self,
        name: impl Into<String>,
        input: &StreamPort<I, T>,
        ids: impl IntoIterator<Item = I>,
        weight: S,
        factor: impl Fn(&T) -> crate::Decimal + 'a,
    ) -> Result<StreamConstraint<I, T>, GraphError>
    where
        S: crate::DecimalWeightedScore,
    {
        self.register_fixed(
            name.into(),
            input,
            ids,
            TerminalEvaluation::Weighted {
                weight,
                impact: Box::new(move |v, w, _| Ok(w.multiply_exact(&factor(v))?)),
            },
        )
    }
    fn register_fixed<I: Ord + Clone + 'static, T: Clone + 'static>(
        &mut self,
        name: String,
        input: &StreamPort<I, T>,
        ids: impl IntoIterator<Item = I>,
        evaluate: TerminalEvaluation<'a, T, S>,
    ) -> Result<StreamConstraint<I, T>, GraphError> {
        self.validate(input)?;
        if self.constraints.contains_key(&name) {
            return Err(GraphError::DuplicateConstraint(name));
        }
        if let TerminalEvaluation::Weighted { weight, .. } = &evaluate
            && weight.structural() != 0
        {
            return Err(GraphError::StructuralConstraintWeight(name));
        }
        let mut ids: Vec<_> = ids.into_iter().collect();
        ids.sort();
        if ids.windows(2).any(|w| w[0] == w[1]) {
            return Err(GraphError::Dependency(
                "duplicate fixed occurrence ID".into(),
            ));
        }
        let direct_ordinals = contiguous_ordinals(&ids);
        let positions = ids
            .iter()
            .cloned()
            .enumerate()
            .map(|(n, id)| (id, n))
            .collect();
        let out = self.push(FixedTerminal {
            rows: Slots::new(ids.len()),
            ids,
            positions,
            direct_ordinals,
            materialized: OnceCell::new(),
            evaluate,
            total: S::ZERO,
            checkpoint_box: CheckpointBox::new(),
            name: name.clone(),
        });
        self.constraints.insert(name, out.node);
        self.connect(input.node, out.node, false);
        Ok(StreamConstraint(out))
    }
}

#[cfg(test)]
mod index_tests;
