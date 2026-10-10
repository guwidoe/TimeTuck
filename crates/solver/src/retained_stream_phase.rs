//! Borrow an already initialized director for one phase without charging its old
//! initialization work again. The optional best callback retains a graph snapshot.
use crate::{Assignment, ListAssignment, ListMove, MixedAssignment, MixedMove, Move};
use crate::{
    list_search_scoring::ListSearchDirector, mixed_search_scoring::MixedSearchDirector,
    search_scoring::SearchDirector,
};

pub(crate) type Observer<'a, A, S> = Option<&'a mut dyn FnMut(crate::SolutionUpdate<'_, A, S>)>;

type BestCallback<'a, D> = Option<&'a mut dyn FnMut(&D, bool)>;
pub(crate) struct RetainedPhase<'a, D> {
    pub director: &'a mut D,
    pub initialization_work: u128,
    pub on_best: BestCallback<'a, D>,
}
macro_rules! retained {
    (($($v:ident),+), $trait:ident, $assignment:ty, $movement:ty, $selection:ty) => {
        impl<$($v,)+ S, D: $trait<$($v,)+ S>> $trait<$($v,)+ S> for RetainedPhase<'_, D> {
            type Error = D::Error;
            fn assignment(&self) -> &$assignment { self.director.assignment() }
            fn score(&self) -> S { self.director.score() }
            fn initial_constraint_evaluations(&self) -> u128 { self.initialization_work }
            fn notify_best(&mut self, complete: impl FnOnce(&$assignment) -> bool) {
                if let Some(on_best) = &mut self.on_best {
                    let complete = complete(self.director.assignment());
                    self.director.notify_best(|_| complete);
                    on_best(self.director, complete);
                } else {
                    self.director.notify_best(complete);
                }
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

impl<A, M, S, D: crate::custom_phase::CommandDirector<A, M, S>>
    crate::custom_phase::CommandDirector<A, M, S> for RetainedPhase<'_, D>
{
    type Error = D::Error;
    fn assignment(&self) -> &A {
        self.director.assignment()
    }
    fn score(&self) -> S {
        self.director.score()
    }
    fn initial_evaluations(&self) -> u128 {
        self.initialization_work
    }
    fn preview(&self, movement: &M) -> Result<Option<crate::MoveInspection<A>>, Self::Error> {
        self.director.preview(movement)
    }
    fn assert_consistent(&mut self) -> Result<(), Self::Error> {
        self.director.assert_consistent()
    }
    fn trial(
        &mut self,
        movement: &M,
        visit: &mut dyn FnMut(&A, S, u128) -> bool,
    ) -> Result<(), Self::Error> {
        self.director.trial(movement, visit)
    }
}
