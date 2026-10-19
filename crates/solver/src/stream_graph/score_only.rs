//! Explicit contribution-only terminals. Full rows remain in upstream sources/operators.
use super::*;

struct ScoreOnlyTerminal<'a, I, T, S> {
    evaluate: TerminalEvaluation<'a, T, S>,
    total: S,
    checkpoint_box: CheckpointBox<TerminalJournal<S>>,
    name: String,
    contributions: SnapshotMap<I, S>,
}
impl<I: Ord + Clone + 'static, T: Clone + 'static, S: AdditiveScore + 'static> Node<S>
    for ScoreOnlyTerminal<'_, I, T, S>
{
    fn constraint_weight(&self) -> Option<&S> {
        match &self.evaluate {
            TerminalEvaluation::Weighted { weight, .. } => Some(weight),
            _ => None,
        }
    }
    fn validate_total(&self, arithmetic: crate::ScoringArithmetic) -> Result<(), GraphError> {
        arithmetic
            .sum(self.contributions.values())
            .map(|_| ())
            .map_err(|_| GraphError::ScoreOverflow {
                constraint: Some(self.name.clone()),
            })
    }
    fn replace_weight(&mut self, new: S) {
        match &mut self.evaluate {
            TerminalEvaluation::Weighted { weight, .. } => *weight = new,
            _ => unreachable!("unweighted terminal"),
        }
    }

    fn clear(&mut self) {
        self.total = S::ZERO;
        self.contributions.clear();
    }
    fn evaluates_score(&self, event: &Event) -> bool {
        typed::<I, T>(event).value.is_some()
    }
    fn scores_match(&self, checkpoint: &dyn Any) -> bool {
        let journal_view;
        let (old, total) = if let Some(saved) = CheckpointBox::<TerminalJournal<S>>::get(checkpoint)
        {
            journal_view = self.contributions.journal_snapshot(saved.rows);
            (&journal_view, &saved.total)
        } else {
            let (old, total) = checkpoint
                .downcast_ref::<(SnapshotMap<I, S>, S)>()
                .expect("terminal checkpoint type");
            (old, total)
        };
        self.total == *total && self.contributions.iter().eq(old.iter())
    }
    fn checkpoint(&self) -> Box<dyn Any> {
        Box::new((self.contributions.clone(), self.total.clone()))
    }
    fn begin_checkpoint(&mut self) -> Box<dyn Any> {
        // User-defined score cloning may panic; it must precede opening a frame.
        let total = self.total.clone();
        let saved = TerminalJournal {
            rows: self.contributions.begin_journal(),
            total,
        };
        self.checkpoint_box.begin(saved)
    }
    fn commit_checkpoint(&mut self, saved: Box<dyn Any>) {
        let (saved, empty) =
            CheckpointBox::<TerminalJournal<S>>::unpack(saved).expect("terminal journal");
        self.contributions.commit_journal(saved.rows);
        drop(saved);
        self.checkpoint_box.recycle(empty);
    }
    fn restore(&mut self, c: Box<dyn Any>) {
        match CheckpointBox::<TerminalJournal<S>>::unpack(c) {
            Ok((saved, empty)) => {
                self.contributions.rollback_journal(saved.rows);
                self.total = saved.total;
                self.checkpoint_box.recycle(empty);
            }
            Err(c) => {
                let (contributions, total) = *c
                    .downcast::<(SnapshotMap<I, S>, S)>()
                    .expect("terminal checkpoint type");
                self.contributions.install_snapshot(contributions);
                self.total = total;
            }
        }
    }
    fn is_constraint(&self) -> bool {
        true
    }
    fn match_count(&self) -> usize {
        self.contributions.len()
    }
    fn score(&self) -> S {
        self.total.clone()
    }
    fn apply(
        &mut self,
        _: bool,
        event: &Event,
        arithmetic: NodeArithmetic,
    ) -> Result<Outputs, GraphError> {
        let change = typed::<I, T>(event);
        let contribution = change
            .value
            .as_ref()
            .map(|value| self.evaluate.evaluate(value, &self.name, arithmetic))
            .transpose()?;
        if contribution
            .as_ref()
            .is_some_and(|score| score.structural() != 0)
        {
            return Err(GraphError::StructuralContribution(self.name.clone()));
        }
        let old = self
            .contributions
            .get(&change.id)
            .cloned()
            .unwrap_or(S::ZERO);
        let new = contribution.clone().unwrap_or(S::ZERO);
        let total = arithmetic
            .totals
            .adjust(&self.total, [(&old, &new)])
            .map_err(|_| GraphError::ScoreOverflow {
                constraint: Some(self.name.clone()),
            })?;
        if let Some(contribution) = contribution {
            self.contributions.insert(change.id.clone(), contribution);
        } else {
            self.contributions.remove(&change.id);
        }
        self.total = total;
        Ok(Outputs::new())
    }
}

impl<'a, S: AdditiveScore + 'static> StreamGraphBuilder<'a, S> {
    /// Score every occurrence while retaining only its ID and contribution score
    /// in this terminal. Upstream sources/operators still retain their full rows.
    ///
    /// This is explicit: `score` keeps its ordinary payload-retention contract.
    /// `constraint_matches` and tuple-based explanations on this handle return
    /// `GraphError::ConstraintMatchesUnavailable`, even for an empty constraint.
    /// Totals, match-count metrics, rollback, saved candidates and consistency
    /// checks remain available. The terminal does not clone payloads; user Clone
    /// side effects/panics at ordinary tracked-terminal storage are absent here.
    /// Callbacks still run for every delivered put, including replacements, and
    /// not for retractions. Activity/weight replay follows ordinary graph rules.
    pub fn score_only<I: Ord + Clone + 'static, T: Clone + 'static>(
        &mut self,
        name: impl Into<String>,
        input: &StreamPort<I, T>,
        evaluate: impl Fn(&T) -> S + 'a,
    ) -> Result<StreamConstraint<I, T>, GraphError> {
        self.register_score_only(
            name.into(),
            input,
            TerminalEvaluation::Direct(Box::new(evaluate)),
        )
    }

    /// Integer-weighted score-only terminal. Zero weights prune branches and
    /// live overrides replay current full source rows through existing machinery.
    pub fn score_only_weighted<I: Ord + Clone + 'static, T: Clone + 'static>(
        &mut self,
        name: impl Into<String>,
        input: &StreamPort<I, T>,
        weight: S,
        factor: impl Fn(&T) -> i64 + 'a,
    ) -> Result<StreamConstraint<I, T>, GraphError> {
        self.register_score_only(
            name.into(),
            input,
            TerminalEvaluation::Weighted {
                weight,
                impact: Box::new(move |value, weight, arithmetic| {
                    Ok(arithmetic.scale(weight, factor(value))?)
                }),
            },
        )
    }

    /// Exact decimal-weighted score-only terminal, with the ordinary checked
    /// decimal impact errors and transactional weight-override behavior.
    pub fn score_only_weighted_decimal<I: Ord + Clone + 'static, T: Clone + 'static>(
        &mut self,
        name: impl Into<String>,
        input: &StreamPort<I, T>,
        weight: S,
        factor: impl Fn(&T) -> crate::Decimal + 'a,
    ) -> Result<StreamConstraint<I, T>, GraphError>
    where
        S: crate::DecimalWeightedScore,
    {
        self.register_score_only(
            name.into(),
            input,
            TerminalEvaluation::Weighted {
                weight,
                impact: Box::new(
                    move |value, weight, _| Ok(weight.multiply_exact(&factor(value))?),
                ),
            },
        )
    }

    fn register_score_only<I: Ord + Clone + 'static, T: Clone + 'static>(
        &mut self,
        name: String,
        input: &StreamPort<I, T>,
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
        let out = self.push(ScoreOnlyTerminal::<I, T, S> {
            evaluate,
            total: S::ZERO,
            checkpoint_box: CheckpointBox::new(),
            name: name.clone(),
            contributions: SnapshotMap::new(),
        });
        self.constraints.insert(name, out.node);
        self.connect(input.node, out.node, false);
        Ok(StreamConstraint(out))
    }
}

#[cfg(test)]
mod tests;
