//! Problem-specific candidate generation using the existing local-search engine.

/// Supply candidates for one immutable assignment snapshot at a completed-step
/// boundary. The solver recreates this iterator each step and applies its normal
/// scoring, acceptance, tabu, foraging, rollback and stopping rules.
///
/// Providers replace built-in neighborhoods, union settings and mixed joint-move
/// generation. Variable shuffling does not reorder provider inputs; candidate
/// shuffling and tie breaking still apply. Invalid moves fail the search using
/// the existing model/scoring error. Returning no moves ends an ordinary scan.
/// Providers may retain state, but must not mutate the model or snapshot.
///
/// Iterator work is cooperative: factory/next calls must return promptly. Only
/// scored proposals count toward evaluation limits; candidate shuffling may
/// prefetch a bounded window. Infinite iterators require a search budget.
///
/// Closures returning Vec<M> implement this trait for convenience. Implement the
/// trait directly to return lazy iterators borrowing the current snapshot.
pub trait MoveProvider<P, A, M> {
    fn moves<'a>(
        &'a mut self,
        problem: &'a P,
        assignment: &'a A,
        completed_steps: u64,
    ) -> Box<dyn Iterator<Item = M> + 'a>
    where
        M: 'a;
}
impl<P, A, M, F> MoveProvider<P, A, M> for F
where
    F: FnMut(&P, &A, u64) -> Vec<M>,
{
    fn moves<'a>(&'a mut self, p: &'a P, a: &'a A, step: u64) -> Box<dyn Iterator<Item = M> + 'a>
    where
        M: 'a,
    {
        Box::new(self(p, a, step).into_iter())
    }
}

pub(crate) enum ProvidedMoves<'a, M, I> {
    BuiltIn(I),
    Custom(Box<dyn Iterator<Item = M> + 'a>),
}
impl<M, I: Iterator<Item = M>> Iterator for ProvidedMoves<'_, M, I> {
    type Item = M;
    fn next(&mut self) -> Option<M> {
        match self {
            Self::BuiltIn(i) => i.next(),
            Self::Custom(i) => i.next(),
        }
    }
}
