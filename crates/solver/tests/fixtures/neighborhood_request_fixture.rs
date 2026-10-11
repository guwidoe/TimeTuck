// Adapted from pinned Community DefaultSolverTest.TestingNeighborhoodProvider.
// Apache-2.0; see compat/provenance.json. Its definition must never be reached
// by the feature-disabled configuration test.
use timetuck::*;
pub struct TestingNeighborhoodProvider;
impl MoveProvider<FiniteProblem<i64>, Assignment<i64>, Move<i64>> for TestingNeighborhoodProvider {
    fn moves<'a>(
        &'a mut self,
        _: &'a FiniteProblem<i64>,
        _: &'a Assignment<i64>,
        _: u64,
    ) -> Box<dyn Iterator<Item = Move<i64>> + 'a>
    where
        Move<i64>: 'a,
    {
        panic!("TestingNeighborhoodProvider is not reached")
    }
}
