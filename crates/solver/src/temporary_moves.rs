//! Temporary scored moves and replayable, snapshot-bound undo records.
use crate::{
    AdditiveScore, Assignment, ListAssignment, ListMove, ListPosition, ListScoreDirector,
    ListScoringError, ListStreamScoreDirector, ListStreamScoringError, MixedAssignment, MixedMove,
    MixedScoreDirector, MixedScoringError, MixedStreamScoreDirector, MixedStreamScoringError, Move,
    ScoreDirector, ScoringError, StreamScoreDirector, StreamScoringError,
};
use std::{
    error::Error,
    fmt,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TrialFailurePolicy {
    #[default]
    Rollback,
    /// Keep the validated candidate or successful live-edit prefix when a
    /// construction/postprocessor callback errors or panics. A failed individual
    /// validation/scoring step always rolls itself back.
    KeepApplied,
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RestoredScorePolicy {
    #[default]
    Cached,
    /// After successful postprocessing and rollback, recalculate contributions
    /// (or replay the stream graph) and check the restored cache against them.
    /// A mismatch is reported, not silently installed as a replacement score.
    CheckFresh,
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TemporaryMoveOptions {
    pub failure: TrialFailurePolicy,
    pub restored_score: RestoredScorePolicy,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TemporaryMoveError<D, E> {
    Apply(D),
    Construction(E),
    FlawedProcessor(E),
    Postprocessor(E),
    RestoredScore(D),
}
impl<D: fmt::Display, E: fmt::Display> fmt::Display for TemporaryMoveError<D, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Apply(e) => write!(f, "temporary move failed: {e}"),
            Self::Construction(e) => write!(f, "move construction failed: {e}"),
            Self::FlawedProcessor(e) => write!(f, "flawed candidate processor failed: {e}"),
            Self::Postprocessor(e) => write!(f, "temporary postprocessor failed: {e}"),
            Self::RestoredScore(e) => write!(f, "restored score check failed: {e}"),
        }
    }
}
impl<D: Error + 'static, E: Error + 'static> Error for TemporaryMoveError<D, E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(match self {
            Self::Apply(e) | Self::RestoredScore(e) => e,
            Self::Construction(e) | Self::Postprocessor(e) | Self::FlawedProcessor(e) => e,
        })
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplayDirection {
    Forward,
    Undo,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StaleMoveRecord {
    pub direction: ReplayDirection,
}
impl fmt::Display for StaleMoveRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "assignment does not match the {:?} replay state",
            self.direction
        )
    }
}
impl Error for StaleMoveRecord {}

/// Owned forward/undo moves plus their expected preimages. A returned temporary
/// record is already undone: apply forward before replaying undo. Records may be
/// reused for repeated forward/undo cycles. Comparisons use assignment Eq; include
/// meaningful payload fields in equality or keep them immutable while recording.
/// Apply the returned move through the same model/director that produced it.
/// Undo restores changed variables/lists only; list undo clears affected lists
/// together before reinsertion so cross-owner moves cannot duplicate identities.
#[derive(Clone, Debug)]
pub struct RecordedMove<A, M> {
    before: A,
    after: A,
    forward: M,
    undo: M,
}
impl<A: Eq, M> RecordedMove<A, M> {
    pub(crate) fn from_parts(before: A, after: A, forward: M, undo: M) -> Self {
        Self {
            before,
            after,
            forward,
            undo,
        }
    }
    pub fn before(&self) -> &A {
        &self.before
    }
    pub fn after(&self) -> &A {
        &self.after
    }
    pub fn forward(&self, current: &A) -> Result<&M, StaleMoveRecord> {
        if current == &self.before {
            Ok(&self.forward)
        } else {
            Err(StaleMoveRecord {
                direction: ReplayDirection::Forward,
            })
        }
    }
    pub fn undo(&self, current: &A) -> Result<&M, StaleMoveRecord> {
        if current == &self.after {
            Ok(&self.undo)
        } else {
            Err(StaleMoveRecord {
                direction: ReplayDirection::Undo,
            })
        }
    }
}
pub type RecordedTemporaryResult<R, A, M, D, E> =
    Result<(R, RecordedMove<A, M>), TemporaryMoveError<D, E>>;

pub(crate) fn inverse_basic<V: Eq + Clone>(
    before: &Assignment<V>,
    after: &Assignment<V>,
) -> Move<V> {
    Move::Composite(
        before
            .values()
            .iter()
            .zip(after.values())
            .enumerate()
            .filter(|(_, (old, new))| old != new)
            .map(|(variable, (value, _))| Move::Change {
                variable,
                value: value.clone(),
            })
            .collect(),
    )
}
pub(crate) fn inverse_list<V: Eq + Clone>(
    before: &ListAssignment<V>,
    after: &ListAssignment<V>,
) -> ListMove<V> {
    let changed: Vec<_> = before
        .lists()
        .iter()
        .zip(after.lists())
        .enumerate()
        .filter(|(_, (old, new))| old != new)
        .map(|(entity, _)| entity)
        .collect();
    let mut steps = vec![];
    for &entity in &changed {
        if !after.lists()[entity].is_empty() {
            steps.push(ListMove::Unassign {
                entity,
                start: 0,
                end: after.lists()[entity].len(),
            });
        }
    }
    for entity in changed {
        for (index, value) in before.lists()[entity].iter().enumerate() {
            steps.push(ListMove::Assign {
                value: value.clone(),
                destination: ListPosition { entity, index },
            });
        }
    }
    ListMove::Composite(steps)
}
pub(crate) fn inverse_mixed<B: Eq + Clone, L: Eq + Clone>(
    before: &MixedAssignment<B, L>,
    after: &MixedAssignment<B, L>,
) -> MixedMove<B, L> {
    MixedMove {
        basic: inverse_basic(before.basic(), after.basic()),
        lists: inverse_list(before.lists(), after.lists()),
    }
}
fn postprocess<T, R, E>(
    trial: T,
    policy: TrialFailurePolicy,
    process: impl FnOnce(&T) -> Result<R, E>,
    commit: impl FnOnce(T),
) -> Result<R, E> {
    let result = catch_unwind(AssertUnwindSafe(|| process(&trial)));
    if policy == TrialFailurePolicy::KeepApplied && !matches!(&result, Ok(Ok(_))) {
        commit(trial);
    } else {
        drop(trial);
    }
    match result {
        Ok(value) => value,
        Err(payload) => resume_unwind(payload),
    }
}

/// Borrow a native scoring director for temporary processing. Implemented for
/// basic, list and mixed assignments in both scoped and stream scoring engines.
/// Processors see immutable candidate assignments and their calculated scores.
/// Success always restores assignment and score; failure obeys the explicit policy.
/// Ordinary unwind is supported; abort/forget and panicking destructors are not.
///
/// ```
/// use timetuck::*;
/// let p=FiniteProblem::new(vec![Variable::new(vec![0,1]).with_initial(0)])?;
/// let mut n=ConstraintNetwork::new(1);
/// n.add_constraint("value",vec![0],|v|SimpleScore::new(i64::from(*v[0].unwrap())))?;
/// let mut d=ScoreDirector::new(&p,&n,p.initial_assignment())?;
/// let movement=Move::Change {variable:0,value:Some(1)};
/// let (seen,record)=TemporaryMoves::new(&mut d).recorded(&movement,TemporaryMoveOptions::default(),
///     |_,score|Ok::<_,std::convert::Infallible>(score))?;
/// assert_eq!(seen,SimpleScore::new(1));
/// assert_eq!(d.score(),SimpleScore::new(0));
/// let forward=record.forward(d.assignment())?;
/// d.apply(forward)?.commit();
/// let undo=record.undo(d.assignment())?;
/// d.apply(undo)?.commit();
/// assert_eq!(d.score(),SimpleScore::new(0));
/// # Ok::<(),Box<dyn std::error::Error>>(())
/// ```
pub struct TemporaryMoves<'d, D> {
    director: &'d mut D,
}
impl<'d, D> TemporaryMoves<'d, D> {
    pub fn new(director: &'d mut D) -> Self {
        Self { director }
    }
}
macro_rules! processor {
    (($($v:ident),+), $director:ty, $assignment:ty, $movement:ty, $error:ty, $inverse:ident, [$($bounds:tt)*]) => {
        impl<'d, 'p, 'c, $($v,)+ S> TemporaryMoves<'d, $director>
        where
            $($bounds)*
        {
            /// Apply, inspect and restore a candidate. Invalid/scoring-failed moves
            /// do not invoke the postprocessor. Fresh checking runs only on success.
            pub fn process<R, E>(
                &mut self,
                movement: &$movement,
                options: TemporaryMoveOptions,
                process: impl FnOnce(&$assignment, S) -> Result<R, E>,
            ) -> Result<R, TemporaryMoveError<$error, E>> {
                let trial = self.director.apply(movement).map_err(TemporaryMoveError::Apply)?;
                let result = postprocess(
                    trial,
                    options.failure,
                    |t| process(t.assignment(), t.score()),
                    |t| t.commit(),
                ).map_err(TemporaryMoveError::Postprocessor)?;
                if options.restored_score == RestoredScorePolicy::CheckFresh {
                    self.director.assert_consistent().map_err(TemporaryMoveError::RestoredScore)?;
                }
                Ok(result)
            }

            /// Build a move through live, individually validated edits. The build
            /// callback sees each installed assignment/score through the session.
            /// On success, run the postprocessor and restore the entry snapshot.
            /// Construction and postprocessor errors/panics share the failure policy;
            /// KeepApplied retains the last successful edit, never a failed partial step.
            pub fn construct<R, E>(
                &mut self,
                options: TemporaryMoveOptions,
                build: impl FnOnce(&mut crate::MoveSession<'_, $director>) -> Result<(), E>,
                process: impl FnOnce(&$assignment, S) -> Result<R, E>,
            ) -> Result<R, TemporaryMoveError<$error, E>> {
                let mut session = self.director.edit_session().map_err(TemporaryMoveError::Apply)?;
                let outcome = catch_unwind(AssertUnwindSafe(|| {
                    build(&mut session).map_err(TemporaryMoveError::Construction)?;
                    process(session.assignment(), session.score()).map_err(TemporaryMoveError::Postprocessor)
                }));
                if options.failure == TrialFailurePolicy::KeepApplied && !matches!(&outcome, Ok(Ok(_))) {
                    session.commit();
                } else {
                    drop(session);
                }
                let result = match outcome {
                    Ok(value) => value?,
                    Err(payload) => resume_unwind(payload),
                };
                if options.restored_score == RestoredScorePolicy::CheckFresh {
                    self.director.assert_consistent().map_err(TemporaryMoveError::RestoredScore)?;
                }
                Ok(result)
            }

            /// Successful processing returns a record whose forward move is no
            /// longer applied. Both preimages are owned; replay checks stale state.
            pub fn recorded<R, E>(
                &mut self,
                movement: &$movement,
                options: TemporaryMoveOptions,
                process: impl FnOnce(&$assignment, S) -> Result<R, E>,
            ) -> RecordedTemporaryResult<R, $assignment, $movement, $error, E> {
                let before = self.director.assignment().clone();
                let mut after = None;
                let result = self.process(movement, options, |assignment, score| {
                    after = Some(assignment.clone());
                    process(assignment, score)
                })?;
                let after = after.expect("successful postprocessor observed the candidate");
                let undo = $inverse(&before, &after);
                Ok((result, RecordedMove { before, after, forward: movement.clone(), undo }))
            }
        }
    }
}
processor!(
    (V), ScoreDirector<'p, 'c, V, S>, Assignment<V>, Move<V>, ScoringError,
    inverse_basic, [V: Eq + Clone, S: AdditiveScore]
);
processor!(
    (V), StreamScoreDirector<'p, 'c, V, S>, Assignment<V>, Move<V>, StreamScoringError,
    inverse_basic, [V: Eq + Clone + 'static, S: AdditiveScore + 'static]
);
processor!(
    (V), ListScoreDirector<'p, 'c, V, S>, ListAssignment<V>, ListMove<V>, ListScoringError,
    inverse_list, [V: Eq + Clone, S: AdditiveScore]
);
processor!(
    (V), ListStreamScoreDirector<'p, 'c, V, S>, ListAssignment<V>, ListMove<V>, ListStreamScoringError,
    inverse_list, [V: Eq + Clone + 'static, S: AdditiveScore + 'static]
);
processor!(
    (B, L), MixedScoreDirector<'p, 'c, B, L, S>, MixedAssignment<B, L>, MixedMove<B, L>, MixedScoringError,
    inverse_mixed, [B: Eq + Clone, L: Eq + Clone, S: AdditiveScore]
);
processor!(
    (B, L), MixedStreamScoreDirector<'p, 'c, B, L, S>, MixedAssignment<B, L>, MixedMove<B, L>, MixedStreamScoringError,
    inverse_mixed, [B: Eq + Clone + 'static, L: Eq + Clone + 'static, S: AdditiveScore + 'static]
);

macro_rules! inspected_processor {
    (($($v:ident),+), $director:ty, $assignment:ty, $movement:ty, $error:ty, [$($bounds:tt)*]) => {
        impl<'d, 'p, 'c, $($v,)+ S> TemporaryMoves<'d, $director>
        where $($bounds)*
        {
            /// Route an actual structural preview to a separate callback. Sound
            /// candidates use normal temporary scoring and failure policies. Flawed
            /// candidates are never installed or scored, including with KeepApplied;
            /// their callback sees the proposed assignment and affected registry IDs.
            /// Errors/panics in that callback leave working state and caches intact.
            /// CheckFresh checks the original cache after either successful branch.
            pub fn process_inspected<R,E>(
                &mut self,
                movement: &$movement,
                options: TemporaryMoveOptions,
                valid: impl FnOnce(&$assignment,S)->Result<R,E>,
                flawed: impl FnOnce(&crate::MoveInspection<$assignment>)->Result<R,E>,
            )->Result<R,TemporaryMoveError<$error,E>> {
                let preview=self.director.inspect_move(movement).map_err(TemporaryMoveError::Apply)?;
                if !preview.is_flawed() {return self.process(movement,options,valid);}
                let result=flawed(&preview).map_err(TemporaryMoveError::FlawedProcessor)?;
                if options.restored_score==RestoredScorePolicy::CheckFresh {
                    self.director.assert_consistent().map_err(TemporaryMoveError::RestoredScore)?;
                }
                Ok(result)
            }
        }
    }
}
inspected_processor!((V),ListScoreDirector<'p,'c,V,S>,ListAssignment<V>,ListMove<V>,ListScoringError,[V:Eq+Clone,S:AdditiveScore]);
inspected_processor!((V),ListStreamScoreDirector<'p,'c,V,S>,ListAssignment<V>,ListMove<V>,ListStreamScoringError,[V:Eq+Clone+'static,S:AdditiveScore+'static]);
inspected_processor!((B,L),MixedScoreDirector<'p,'c,B,L,S>,MixedAssignment<B,L>,MixedMove<B,L>,MixedScoringError,[B:Eq+Clone,L:Eq+Clone,S:AdditiveScore]);
inspected_processor!((B,L),MixedStreamScoreDirector<'p,'c,B,L,S>,MixedAssignment<B,L>,MixedMove<B,L>,MixedStreamScoringError,[B:Eq+Clone+'static,L:Eq+Clone+'static,S:AdditiveScore+'static]);
