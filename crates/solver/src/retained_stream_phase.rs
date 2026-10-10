//! Borrow an already initialized director for one phase without charging its old
//! initialization work again. The optional best callback retains a graph snapshot.
use crate::{Assignment, ListAssignment, ListMove, MixedAssignment, MixedMove, Move};
use crate::{
    list_search_scoring::ListSearchDirector, mixed_search_scoring::MixedSearchDirector,
    search_scoring::SearchDirector,
};

type BestCallback<'a, D> = Option<&'a mut dyn FnMut(&D)>;
pub(crate) struct RetainedPhase<'a, D> {
    pub director: &'a mut D,
    pub on_best: BestCallback<'a, D>,
}
macro_rules! retained {
    (($($v:ident),+), $trait:ident, $assignment:ty, $movement:ty, $selection:ty) => {
        impl<$($v,)+ S, D: $trait<$($v,)+ S>> $trait<$($v,)+ S> for RetainedPhase<'_, D> {
            type Error = D::Error;
            fn assignment(&self) -> &$assignment { self.director.assignment() }
            fn score(&self) -> S { self.director.score() }
            fn initial_constraint_evaluations(&self) -> u128 { 0 }
            fn notify_best(&mut self, complete: impl FnOnce(&$assignment) -> bool) {
                self.director.notify_best(complete);
                if let Some(on_best) = &mut self.on_best { on_best(self.director); }
            }
            fn assert_consistent(&mut self) -> Result<(), Self::Error> {
                self.director.assert_consistent()
            }
            fn select_best(
                &mut self,
                moves: impl IntoIterator<Item = $movement>,
                max_evaluations: u64,
                forager: impl Into<crate::ForagerConfig<S>>,
                accepts: impl FnMut(&$movement, S) -> bool,
                quit_early: impl crate::construction::SelectionPolicy<S>,
                assert_scores: bool,
            ) -> Result<$selection, Self::Error> {
                self.director.select_best(moves, max_evaluations, forager, accepts, quit_early, assert_scores)
            }
        }
    }
}
retained!(
    (V),
    SearchDirector,
    Assignment<V>,
    Move<V>,
    crate::scoring::Selection<V>
);
retained!(
    (V),
    ListSearchDirector,
    ListAssignment<V>,
    ListMove<V>,
    crate::list_scoring::ListSelection<V>
);
retained!((B,L), MixedSearchDirector, MixedAssignment<B,L>, MixedMove<B,L>, crate::mixed_scoring::MixedSelection<B,L>);
