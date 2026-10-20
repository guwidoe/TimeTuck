//! Explicit final-batch projection directly into retained scoring rows.
use super::*;
use std::cell::{Cell, OnceCell};

type FusedSnapshot<I, J, L, R, U, S> = (
    SharedLinkSnapshot<I, J, L, R>,
    SlotSnapshot<(U, S)>,
    S,
    usize,
);
type FusedMatches<E, I, J, U, S> = SnapshotMap<LinkId<E, I, J>, (U, S)>;
struct FusedJournal<S> {
    left: usize,
    right: usize,
    rows: usize,
    total: S,
    count: usize,
}
struct FusedFixedLinkConstraint<'a, E, I, J, L, R, U, S> {
    inputs: SharedFixedLinks<'a, E, I, J, L, R, U>,
    rows: Slots<(U, S)>,
    materialized: OnceCell<FusedMatches<E, I, J, U, S>>,
    evaluate: TerminalEvaluation<'a, U, S>,
    total: S,
    count: usize,
    // Actual callback attempts never roll back, including failed trials/replay.
    evaluations: Rc<Cell<u128>>,
    checkpoint_box: CheckpointBox<FusedJournal<S>>,
    name: String,
}
impl<
    E: Ord + Clone + 'static,
    I: Ord + Clone + 'static,
    J: Ord + Clone + 'static,
    L: 'static,
    R: 'static,
    U: Clone + 'static,
    S: AdditiveScore + 'static,
> Node<S> for FusedFixedLinkConstraint<'_, E, I, J, L, R, U, S>
{
    fn prefers_batch(&self) -> bool {
        true
    }
    fn score_evaluation_counter(&self) -> Option<Rc<Cell<u128>>> {
        Some(self.evaluations.clone())
    }
    fn constraint_weight(&self) -> Option<&S> {
        match &self.evaluate {
            TerminalEvaluation::Weighted { weight, .. } => Some(weight),
            _ => None,
        }
    }
    fn replace_weight(&mut self, new: S) {
        match &mut self.evaluate {
            TerminalEvaluation::Weighted { weight, .. } => *weight = new,
            _ => unreachable!("unweighted fused constraint"),
        }
    }
    fn validate_total(&self, arithmetic: crate::ScoringArithmetic) -> Result<(), GraphError> {
        arithmetic
            .sum(self.rows.iter().flatten().map(|(_, score)| score))
            .map(|_| ())
            .map_err(|_| GraphError::ScoreOverflow {
                constraint: Some(self.name.clone()),
            })
    }
    fn score(&self) -> S {
        self.total.clone()
    }
    fn match_count(&self) -> usize {
        self.count
    }
    fn is_constraint(&self) -> bool {
        true
    }
    fn matches(&self) -> Option<&dyn Any> {
        Some(self.materialized.get_or_init(|| {
            let mut rows = SnapshotMap::new();
            for (id, row) in self.inputs.links.iter().zip(self.rows.iter()) {
                if let Some(row) = row {
                    rows.insert(id.clone(), row.clone());
                }
            }
            rows
        }))
    }
    fn clear(&mut self) {
        self.inputs.left.clear();
        self.inputs.right.clear();
        self.rows.clear();
        self.total = S::ZERO;
        self.count = 0;
        self.materialized.take();
    }
    fn checkpoint(&self) -> Box<dyn Any> {
        Box::new((
            (self.inputs.left.snapshot(), self.inputs.right.snapshot()),
            self.rows.snapshot(),
            self.total.clone(),
            self.count,
        ))
    }
    fn begin_checkpoint(&mut self) -> Box<dyn Any> {
        // Clone user score before opening any journal frame.
        let total = self.total.clone();
        let saved = FusedJournal {
            left: self.inputs.left.begin(),
            right: self.inputs.right.begin(),
            rows: self.rows.begin(),
            total,
            count: self.count,
        };
        self.checkpoint_box.begin(saved)
    }
    fn commit_checkpoint(&mut self, checkpoint: Box<dyn Any>) {
        let (saved, empty) =
            CheckpointBox::<FusedJournal<S>>::unpack(checkpoint).expect("fused fixed-link journal");
        self.inputs.left.commit(saved.left);
        self.inputs.right.commit(saved.right);
        self.rows.commit(saved.rows);
        drop(saved);
        self.checkpoint_box.recycle(empty);
    }
    fn restore(&mut self, checkpoint: Box<dyn Any>) {
        match CheckpointBox::<FusedJournal<S>>::unpack(checkpoint) {
            Ok((saved, empty)) => {
                self.inputs.left.rollback(saved.left);
                self.inputs.right.rollback(saved.right);
                self.rows.rollback(saved.rows);
                self.total = saved.total;
                self.count = saved.count;
                self.checkpoint_box.recycle(empty);
            }
            Err(saved) => {
                let ((left, right), rows, total, count) = *saved
                    .downcast::<FusedSnapshot<I, J, L, R, U, S>>()
                    .expect("fused fixed-link snapshot");
                self.inputs.left.install(left);
                self.inputs.right.install(right);
                self.rows.install(rows);
                self.total = total;
                self.count = count;
            }
        }
        self.materialized.take();
    }
    fn scores_match(&self, checkpoint: &dyn Any) -> bool {
        let view;
        let (rows, total, count) =
            if let Some(saved) = CheckpointBox::<FusedJournal<S>>::get(checkpoint) {
                view = self.rows.entry_snapshot(saved.rows);
                (&view, &saved.total, saved.count)
            } else {
                let (_, rows, total, count) = checkpoint
                    .downcast_ref::<FusedSnapshot<I, J, L, R, U, S>>()
                    .expect("fused fixed-link snapshot");
                (rows, total, *count)
            };
        self.total == *total
            && self.count == count
            && self
                .rows
                .iter()
                .map(|row| row.map(|(_, score)| score))
                .eq(rows
                    .iter()
                    .map(|row| row.as_deref().map(|(_, score)| score)))
    }
    fn apply(
        &mut self,
        right: bool,
        event: &Event,
        arithmetic: NodeArithmetic,
    ) -> Result<Outputs, GraphError> {
        <Self as Node<S>>::apply_batch(self, &[(right, event.clone())], arithmetic)
            .map(Outputs::from)
    }
    fn apply_batch(
        &mut self,
        changes: &[(bool, Event)],
        arithmetic: NodeArithmetic,
    ) -> Result<BatchOutputs, GraphError> {
        let inputs = &mut self.inputs;
        // Recover scratch after callback/ordering panics exactly as the shared
        // endpoint operator does. These arrays never own user values.
        if inputs.inputs_indexed {
            for &(slot, _) in &inputs.left_changes {
                inputs.left_inputs[slot] = usize::MAX;
            }
            for &(slot, _) in &inputs.right_changes {
                inputs.right_inputs[slot] = usize::MAX;
            }
            inputs.inputs_indexed = false;
        }
        latest_slots::<I, L>(
            changes,
            false,
            &inputs.by_left,
            inputs.direct_left,
            &mut inputs.left_changes,
        );
        latest_slots::<J, R>(
            changes,
            true,
            &inputs.by_right,
            inputs.direct_right,
            &mut inputs.right_changes,
        );
        inputs.inputs_indexed = true;
        inputs.affected.clear();
        for &(slot, input) in &inputs.left_changes {
            inputs.left_inputs[slot] = input;
            inputs.affected.extend_from_slice(&inputs.left_links[slot]);
        }
        for &(slot, input) in &inputs.right_changes {
            inputs.right_inputs[slot] = input;
            inputs.affected.extend_from_slice(&inputs.right_links[slot]);
        }
        inputs.affected.sort_unstable();
        inputs.affected.dedup();
        let mut total = self.total.clone();
        let mut count = self.count;
        // Only graph-owned dispatch reaches this private adapter. Its checkpoint
        // restores early endpoint installation if a later callback/check fails.
        // Settling typed shared rows once avoids per-link Any downcasts. Captured
        // matches determine removals, so old endpoint payloads are not needed.
        for &(slot, input) in &inputs.left_changes {
            let row = changes[input]
                .1
                .clone()
                .downcast::<TupleChange<I, L>>()
                .expect("fused fixed-link left event");
            inputs
                .left
                .set_shared(slot, row.value.is_some().then_some(row));
        }
        for &(slot, input) in &inputs.right_changes {
            let row = changes[input]
                .1
                .clone()
                .downcast::<TupleChange<J, R>>()
                .expect("fused fixed-link right event");
            inputs
                .right
                .set_shared(slot, row.value.is_some().then_some(row));
        }
        for &slot in &inputs.affected {
            let (i, j) = inputs.endpoints[slot];
            let left = inputs.left.get(i).and_then(|row| row.value.as_ref());
            let right = inputs.right.get(j).and_then(|row| row.value.as_ref());
            let edge = &inputs.links[slot].0.0;
            let endpoints = left.zip(right);
            let row = match endpoints {
                Some((left, right)) if (inputs.predicate)(edge, left, right) => {
                    let value = (inputs.projection)(edge, left, right);
                    // Increment immediately before invoking the actual user score
                    // callback/factor. Failed arithmetic and unwind remain work.
                    self.evaluations.set(
                        self.evaluations
                            .get()
                            .checked_add(1)
                            .expect("actual scoring callback counter exhausted"),
                    );
                    let score = self.evaluate.evaluate(&value, &self.name, arithmetic)?;
                    if score.structural() != 0 {
                        return Err(GraphError::StructuralContribution(self.name.clone()));
                    }
                    Some((value, score))
                }
                _ => None,
            };
            let previous = self.rows.get(slot);
            if row.is_none() && previous.is_none() {
                continue;
            }
            let zero = S::ZERO;
            let old = previous.map_or(&zero, |(_, score)| score);
            let new = row.as_ref().map_or(&zero, |(_, score)| score);
            total = arithmetic
                .totals
                .adjust(&total, [(old, new)])
                .map_err(|_| GraphError::ScoreOverflow {
                    constraint: Some(self.name.clone()),
                })?;
            match (previous.is_some(), row.is_some()) {
                (false, true) => count += 1,
                (true, false) => count -= 1,
                _ => {}
            }
            // Dispatch checkpoints both endpoint and match journals before this
            // call. Install directly instead of allocating a second batch buffer;
            // a later callback/arithmetic failure poisons the transaction and
            // dropping it restores every earlier match in this batch.
            self.rows.set(slot, row);
        }
        self.total = total;
        self.count = count;
        self.materialized.take();
        Ok(Vec::new())
    }
}

impl<'a, S: AdditiveScore + 'static> StreamGraphBuilder<'a, S> {
    /// Explicitly fuse immutable fixed-link projection and tracked scoring.
    ///
    /// Complete source facts stay upstream. Each linked endpoint shares its
    /// complete immutable producer TupleChange, including its ID. Final events
    /// per source ID settle both sides of a batch; each affected relationship is
    /// inspected once in relationship-ID order. Accepted relationships project
    /// and immediately score, including zero contributions. Retractions use the
    /// captured accepted match, without evaluating the old predicate. A single
    /// immediate update is a one-event batch. Empty topology/batches are valid.
    ///
    /// This new declaration changes callback and ownership schedules: projection
    /// and scoring interleave, there is no intermediate per-link event/queue,
    /// and projected U moves into its retained match slot. Lazy ordinary match
    /// inspection clones U, IDs and scores. Endpoint retention, snapshots,
    /// journals and release order may differ from separate join + score nodes.
    /// It does not promise their physical Clone/Drop/callback trace. Callbacks
    /// must be pure, identities/content stable, and destructors nonpanicking.
    ///
    /// Full matches and zero-score membership remain inspectable through the
    /// returned ordinary StreamConstraint. Checked/structural arithmetic, graph
    /// transactions, saved candidates, activity and weight replay are supported.
    /// Endpoint caches settle before callbacks and match rows settle during them,
    /// under that graph checkpoint. A failed transaction is unreadable and
    /// restores both when dropped.
    /// Actual scoring attempts (including failures/panics) count in the current
    /// transaction; nested diagnostics account independently. Topology changes
    /// require rebuilding the graph. Duplicate relationship IDs are rejected;
    /// sparse/generic IDs, parallel and self links are supported.
    #[allow(clippy::too_many_arguments)]
    pub fn score_fixed_links_batched_shared<
        E: Ord + Clone + 'static,
        I: Ord + Clone + 'static,
        J: Ord + Clone + 'static,
        L: 'static,
        R: 'static,
        U: Clone + 'static,
    >(
        &mut self,
        name: impl Into<String>,
        left: &StreamPort<I, L>,
        right: &StreamPort<J, R>,
        links: impl IntoIterator<Item = (E, I, J)>,
        predicate: impl Fn(&E, &L, &R) -> bool + 'a,
        projection: impl Fn(&E, &L, &R) -> U + 'a,
        evaluate: impl Fn(&U) -> S + 'a,
    ) -> Result<StreamConstraint<LinkId<E, I, J>, U>, GraphError> {
        self.register_fused_fixed_links(
            name.into(),
            left,
            right,
            links,
            predicate,
            projection,
            TerminalEvaluation::Direct(Box::new(evaluate)),
        )
    }
    /// Integer-weighted fused counterpart; zero disables the branch and named
    /// weight overrides rebuild it from all retained source facts.
    #[allow(clippy::too_many_arguments)]
    pub fn score_fixed_links_batched_shared_weighted<
        E: Ord + Clone + 'static,
        I: Ord + Clone + 'static,
        J: Ord + Clone + 'static,
        L: 'static,
        R: 'static,
        U: Clone + 'static,
    >(
        &mut self,
        name: impl Into<String>,
        left: &StreamPort<I, L>,
        right: &StreamPort<J, R>,
        links: impl IntoIterator<Item = (E, I, J)>,
        predicate: impl Fn(&E, &L, &R) -> bool + 'a,
        projection: impl Fn(&E, &L, &R) -> U + 'a,
        weight: S,
        factor: impl Fn(&U) -> i64 + 'a,
    ) -> Result<StreamConstraint<LinkId<E, I, J>, U>, GraphError> {
        self.register_fused_fixed_links(
            name.into(),
            left,
            right,
            links,
            predicate,
            projection,
            TerminalEvaluation::Weighted {
                weight,
                impact: Box::new(move |value, weight, arithmetic| {
                    Ok(arithmetic.scale(weight, factor(value))?)
                }),
            },
        )
    }
    /// Exact decimal-weighted counterpart of the fused declaration.
    #[allow(clippy::too_many_arguments)]
    pub fn score_fixed_links_batched_shared_weighted_decimal<
        E: Ord + Clone + 'static,
        I: Ord + Clone + 'static,
        J: Ord + Clone + 'static,
        L: 'static,
        R: 'static,
        U: Clone + 'static,
    >(
        &mut self,
        name: impl Into<String>,
        left: &StreamPort<I, L>,
        right: &StreamPort<J, R>,
        links: impl IntoIterator<Item = (E, I, J)>,
        predicate: impl Fn(&E, &L, &R) -> bool + 'a,
        projection: impl Fn(&E, &L, &R) -> U + 'a,
        weight: S,
        factor: impl Fn(&U) -> crate::Decimal + 'a,
    ) -> Result<StreamConstraint<LinkId<E, I, J>, U>, GraphError>
    where
        S: crate::DecimalWeightedScore,
    {
        self.register_fused_fixed_links(
            name.into(),
            left,
            right,
            links,
            predicate,
            projection,
            TerminalEvaluation::Weighted {
                weight,
                impact: Box::new(
                    move |value, weight, _| Ok(weight.multiply_exact(&factor(value))?),
                ),
            },
        )
    }
    #[allow(clippy::too_many_arguments)]
    fn register_fused_fixed_links<
        E: Ord + Clone + 'static,
        I: Ord + Clone + 'static,
        J: Ord + Clone + 'static,
        L: 'static,
        R: 'static,
        U: Clone + 'static,
    >(
        &mut self,
        name: String,
        left: &StreamPort<I, L>,
        right: &StreamPort<J, R>,
        links: impl IntoIterator<Item = (E, I, J)>,
        predicate: impl Fn(&E, &L, &R) -> bool + 'a,
        projection: impl Fn(&E, &L, &R) -> U + 'a,
        evaluate: TerminalEvaluation<'a, U, S>,
    ) -> Result<StreamConstraint<LinkId<E, I, J>, U>, GraphError> {
        self.validate(left)?;
        self.validate(right)?;
        if self.constraints.contains_key(&name) {
            return Err(GraphError::DuplicateConstraint(name));
        }
        if let TerminalEvaluation::Weighted { weight, .. } = &evaluate
            && weight.structural() != 0
        {
            return Err(GraphError::StructuralConstraintWeight(name));
        }
        let mut links: Vec<_> = links.into_iter().map(|(e, i, j)| ((e, i), j)).collect();
        links.sort_by(|a, b| a.0.0.cmp(&b.0.0));
        if links.windows(2).any(|w| w[0].0.0 == w[1].0.0) {
            return Err(GraphError::Dependency(
                "duplicate fixed relationship ID".into(),
            ));
        }
        let left_keys: BTreeSet<_> = links.iter().map(|((_, i), _)| i.clone()).collect();
        let right_keys: BTreeSet<_> = links.iter().map(|(_, j)| j.clone()).collect();
        let by_left: BTreeMap<_, _> = left_keys
            .into_iter()
            .enumerate()
            .map(|(n, i)| (i, n))
            .collect();
        let by_right: BTreeMap<_, _> = right_keys
            .into_iter()
            .enumerate()
            .map(|(n, j)| (j, n))
            .collect();
        let mut left_links = vec![Vec::new(); by_left.len()];
        let mut right_links = vec![Vec::new(); by_right.len()];
        let endpoints = links
            .iter()
            .enumerate()
            .map(|(n, ((_, i), j))| {
                let (i, j) = (by_left[i], by_right[j]);
                left_links[i].push(n);
                right_links[j].push(n);
                (i, j)
            })
            .collect();
        let inputs = SharedFixedLinks {
            left: Slots::new(by_left.len()),
            right: Slots::new(by_right.len()),
            checkpoint_box: CheckpointBox::new(),
            left_changes: Vec::new(),
            right_changes: Vec::new(),
            affected: Vec::new(),
            left_inputs: vec![usize::MAX; by_left.len()],
            right_inputs: vec![usize::MAX; by_right.len()],
            inputs_indexed: false,
            endpoints,
            direct_left: contiguous_slots(&by_left),
            direct_right: contiguous_slots(&by_right),
            by_left,
            by_right,
            left_links,
            right_links,
            predicate: Box::new(predicate),
            projection: Box::new(projection),
            links,
        };
        let rows = Slots::new(inputs.links.len());
        let out = self.push(FusedFixedLinkConstraint {
            inputs,
            rows,
            materialized: OnceCell::new(),
            evaluate,
            total: S::ZERO,
            count: 0,
            evaluations: Rc::new(Cell::new(0)),
            checkpoint_box: CheckpointBox::new(),
            name: name.clone(),
        });
        self.constraints.insert(name, out.node);
        self.connect(left.node, out.node, false);
        self.connect(right.node, out.node, true);
        Ok(StreamConstraint(out))
    }
}
#[cfg(test)]
mod tests;
