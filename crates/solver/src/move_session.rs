//! Live multi-move sessions with snapshot rollback of assignments and scores.
use crate::{
    AdditiveScore, Assignment, ListAssignment, ListMove, ListScoreDirector, ListScoringError,
    ListStreamScoreDirector, ListStreamScoringError, MixedAssignment, MixedMove,
    MixedScoreDirector, MixedScoringError, MixedStreamScoreDirector, MixedStreamScoringError, Move,
    ScoreDirector, ScoringError, StreamScoreDirector, StreamScoringError,
};

type RestoreSession<'d, D> = Box<dyn FnOnce(&mut D) + 'd>;
/// A live edit scope. Each apply validates, scores and installs one whole move;
/// rejected moves leave the successful prefix available. Drop restores the entire
/// entry snapshot, including scoring caches, without replaying scoring callbacks.
/// Commit retains the prefix. An inner session may roll back or commit independently;
/// committing an inner session still leaves its edits subject to outer rollback.
///
/// Session state is opaque and tied to one borrowed director. Snapshot storage is
/// proportional to assignment/cache size; use ordinary scored guards for hot loops.
/// External callback side effects are not rolled back. Normal drop-guard limits
/// (forget, abort, panicking destructors) apply. The restore callback is internal.
///
/// ```
/// use timetuck::*;
/// let p=FiniteProblem::new(vec![Variable::new(vec![0,1,2]).with_initial(0)])?;
/// let mut n=ConstraintNetwork::new(1);
/// n.add_constraint("value",vec![0],|v|SimpleScore::new(i64::from(*v[0].unwrap())))?;
/// let mut d=ScoreDirector::new(&p,&n,p.initial_assignment())?;
/// {
///     let mut edits=d.edit_session()?;
///     edits.apply(&Move::Change {variable:0,value:Some(1)})?;
///     {
///         let mut inner=edits.nested()?;
///         inner.apply(&Move::Change {variable:0,value:Some(2)})?;
///     }
///     assert_eq!(edits.score(),SimpleScore::new(1));
/// }
/// assert_eq!(d.score(),SimpleScore::new(0));
/// # Ok::<(),Box<dyn std::error::Error>>(())
/// ```
#[must_use = "drop restores the entry state; commit to retain edits"]
pub struct MoveSession<'d, D> {
    director: &'d mut D,
    restore: Option<RestoreSession<'d, D>>,
}
impl<'d, D> MoveSession<'d, D> {
    pub(crate) fn new(director: &'d mut D, restore: impl FnOnce(&mut D) + 'd) -> Self {
        Self {
            director,
            restore: Some(Box::new(restore)),
        }
    }
    /// Run a temporary program inside this session; its changes remain subject to outer rollback.
    pub fn temporary(&mut self) -> crate::TemporaryMoves<'_, D> {
        crate::TemporaryMoves::new(self.director)
    }
    pub fn commit(mut self) {
        self.restore = None;
    }
    pub fn rollback(self) {
        drop(self);
    }
}
impl<D> Drop for MoveSession<'_, D> {
    fn drop(&mut self) {
        if let Some(restore) = self.restore.take() {
            restore(self.director);
        }
    }
}
macro_rules! session {
    (($($v:ident),+), $director:ty, $assignment:ty, $movement:ty, $error:ty, [$($bounds:tt)*]) => {
        impl<'d, 'p, 'c, $($v,)+ S> MoveSession<'d, $director>
        where $($bounds)*
        {
            pub fn assignment(&self) -> &$assignment { self.director.assignment() }
            pub fn score(&self) -> S { self.director.score() }
            /// Install one validated/scored move. Failure restores this step only.
            pub fn apply(&mut self, movement: &$movement) -> Result<(), $error> {
                self.director.apply(movement)?.commit();
                Ok(())
            }
            pub fn nested(&mut self) -> Result<MoveSession<'_, $director>, $error> {
                self.director.edit_session()
            }
            pub fn assert_consistent(&mut self) -> Result<(), $error> {
                self.director.assert_consistent()
            }
        }
    }
}
session!((V), ScoreDirector<'p,'c,V,S>, Assignment<V>, Move<V>, ScoringError, [V:Eq+Clone,S:AdditiveScore]);
session!((V), StreamScoreDirector<'p,'c,V,S>, Assignment<V>, Move<V>, StreamScoringError, [V:Eq+Clone+'static,S:AdditiveScore+'static]);
session!((V), ListScoreDirector<'p,'c,V,S>, ListAssignment<V>, ListMove<V>, ListScoringError, [V:Eq+Clone,S:AdditiveScore]);
session!((V), ListStreamScoreDirector<'p,'c,V,S>, ListAssignment<V>, ListMove<V>, ListStreamScoringError, [V:Eq+Clone+'static,S:AdditiveScore+'static]);
session!((B,L), MixedScoreDirector<'p,'c,B,L,S>, MixedAssignment<B,L>, MixedMove<B,L>, MixedScoringError, [B:Eq+Clone,L:Eq+Clone,S:AdditiveScore]);
session!((B,L), MixedStreamScoreDirector<'p,'c,B,L,S>, MixedAssignment<B,L>, MixedMove<B,L>, MixedStreamScoringError, [B:Eq+Clone+'static,L:Eq+Clone+'static,S:AdditiveScore+'static]);
