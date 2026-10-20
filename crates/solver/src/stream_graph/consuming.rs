//! Private retention permission for a graph consumed by a basic search call.
//! Callback effects remain unrestricted. Only cache contents/ownership are private.
use super::*;
use std::any::TypeId;

pub(super) fn payload<T: 'static>() -> bool {
    let id = TypeId::of::<T>();
    macro_rules! exact { ($($t:ty),+ $(,)?) => { $(id==TypeId::of::<$t>())||+ }; }
    exact!(
        (),
        bool,
        char,
        usize,
        isize,
        u8,
        u16,
        u32,
        u64,
        u128,
        i8,
        i16,
        i32,
        i64,
        i128,
        Option<usize>,
        Option<i64>,
        (usize, Option<usize>),
        (usize, Option<i64>),
        (usize, usize),
        (usize, usize, usize),
        (usize, usize, usize, usize),
        (usize, usize, usize, usize, i64),
        (usize, i64),
        (usize, usize, i64),
        ((usize, usize, usize), (usize, usize, usize)),
        ((usize, usize, usize, usize), (usize, usize, usize, usize)),
        IntSummary,
        (usize, IntSummary),
        ((usize, usize), IntSummary),
        ((usize, usize, usize), IntSummary),
        ((usize, usize, usize, usize), IntSummary),
        (((usize, usize), usize), IntSummary),
        ((usize, (usize, usize)), IntSummary),
        (((usize, usize), (usize, usize)), IntSummary)
    )
}

impl<S: AdditiveScore + 'static> StreamGraph<'_, S> {
    // All checks precede any node mutation. This method is called ONLY by the
    // private constructor inside public ownership-consuming basic searches.
    pub(crate) fn try_enable_consuming_search(&mut self, planning: usize, count: usize) -> bool {
        if self.consuming_search
            || !self.consuming_eligible
            || self.active.is_some()
            || self.initial_activity.is_some()
            || self.batched_updates
            || !crate::primitive_rows::consumed_scores::builtin_score::<S>()
            || self.score != S::ZERO
            || self.nodes.iter().any(|node| {
                node.source_count().is_some_and(|n| n != 0)
                    || (node.is_constraint() && node.match_count() != 0)
            })
        {
            return false;
        }
        for node in &mut self.nodes {
            node.enable_consuming_storage();
        }
        // A separately certified optional source backend may replace this empty
        // planning source. Terminal-only mode does not require source replacement.
        if let Some(source) = self.nodes[planning].consuming_source(count) {
            self.nodes[planning] = source;
        }
        self.consuming_search = true;
        true
    }
}

pub(super) enum TerminalSaved<I, S> {
    Full(crate::primitive_rows::consumed_scores::Rows<I, S>, S),
    Delta(crate::primitive_rows::consumed_scores::Capture<I, S>, S),
}
impl<
    I: Ord + Clone + 'static,
    T: Clone + 'static,
    S: AdditiveScore + 'static,
    E: TerminalEvaluator<T, S>,
> TerminalState<I, T, S, E>
{
    pub(super) fn consumed_checkpoint(&self) -> Box<dyn Any> {
        Box::new(TerminalSaved::Full(
            self.consumed.as_ref().unwrap().clone(),
            self.total.clone(),
        ))
    }
    pub(super) fn consumed_capture(&self, original: &dyn Any) -> Box<dyn Any> {
        let Some(saved) = CheckpointBox::<TerminalJournal<S>>::get(original) else {
            return self.consumed_checkpoint();
        };
        Box::new(TerminalSaved::Delta(
            self.consumed.as_ref().unwrap().capture(saved.rows),
            self.total.clone(),
        ))
    }
    pub(super) fn consumed_restore(&mut self, saved: Box<dyn Any>) {
        match CheckpointBox::<TerminalJournal<S>>::unpack(saved) {
            Ok((saved, empty)) => {
                self.consumed.as_mut().unwrap().rollback(saved.rows);
                self.total = saved.total;
                self.checkpoint_box.recycle(empty);
            }
            Err(saved) => match *saved
                .downcast::<TerminalSaved<I, S>>()
                .expect("consumed terminal checkpoint")
            {
                TerminalSaved::Full(rows, total) => {
                    self.consumed.as_mut().unwrap().install(rows);
                    self.total = total;
                }
                TerminalSaved::Delta(rows, total) => {
                    self.consumed.as_mut().unwrap().install_capture(rows);
                    self.total = total;
                }
            },
        }
    }
    pub(super) fn consumed_matches(&self, checkpoint: &dyn Any) -> bool {
        let current = self.consumed.as_ref().unwrap();
        let snapshot;
        let (old, total) = if let Some(saved) = CheckpointBox::<TerminalJournal<S>>::get(checkpoint)
        {
            snapshot = current.journal_snapshot(saved.rows);
            (&snapshot, &saved.total)
        } else {
            match checkpoint
                .downcast_ref::<TerminalSaved<I, S>>()
                .expect("consumed terminal checkpoint")
            {
                TerminalSaved::Full(rows, total) => (rows, total),
                TerminalSaved::Delta(..) => panic!("consumed diagnostic needs full state"),
            }
        };
        self.total == *total && current.iter().eq(old.iter())
    }
    pub(super) fn apply_consumed(
        &mut self,
        change: &TupleChange<I, T>,
        contribution: Option<S>,
        arithmetic: NodeArithmetic,
    ) -> Result<Outputs, GraphError> {
        let (prepared, (old, unchanged)) =
            self.consumed
                .as_mut()
                .unwrap()
                .prepare_change(&change.id, |score| {
                    let old = score.cloned();
                    let unchanged = old.as_ref() == contribution.as_ref();
                    (old.unwrap_or(S::ZERO), unchanged)
                });
        let new = contribution.clone().unwrap_or(S::ZERO);
        let total = arithmetic
            .totals
            .adjust(&self.total, [(&old, &new)])
            .map_err(|_| GraphError::ScoreOverflow {
                constraint: Some(self.name.clone()),
            })?;
        if let Some(value) = &change.value {
            // Preserve ordinary terminal publication's ID then payload Clone,
            // after all checks. Exact certified T has no destruction hooks; the
            // copied payload is discarded only at its original storage phase.
            let id = change.id.clone();
            let payload = value.clone();
            let contribution = contribution.expect("scored match");
            // This graph is consumed and exact builtin row/ID/score types were
            // certified before activation. No public owner or payload history
            // exists here. Keep every evaluation/check/Clone above, but omit an
            // identical private contribution write and its undo allocation.
            // Option equality distinguishes a present zero from an absent row.
            if unchanged {
                drop((id, contribution));
            } else {
                prepared.insert(id, contribution);
            }
            drop(payload);
        } else if !unchanged {
            prepared.remove();
        }
        self.total = total;
        Ok(Outputs::new())
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod equal_tests;
