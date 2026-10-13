// Complete pinned Community ListSwapMoveSelectorTest.noReachableEntities fixture.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::num::NonZeroUsize;
use timetuck::*;
#[test]
fn no_reachable_entities() {
    let problem = ListProblem::new(
        vec![1, 2],
        vec![
            ListEntity::new(vec![2]).with_value_range(vec![1, 2]),
            ListEntity::new(vec![1]).with_value_range(vec![1]),
        ],
    )
    .unwrap();
    let before = problem.initial_assignment();
    let mut cursor = repeating_list_swap_attempts(
        &problem,
        &before,
        &[1, 2],
        &[1, 2],
        NonZeroUsize::new(8).unwrap(),
        false,
        0,
    )
    .unwrap();
    assert!(cursor.has_next());
    let ListSwapAttempt::NoChange(marker) = cursor.next().unwrap() else {
        panic!("unreachable swap must fall back")
    };
    assert!(std::ptr::eq(marker, &LIST_SWAP_NO_CHANGE));
}
