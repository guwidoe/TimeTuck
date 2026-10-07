// Adapted from pinned Community DefaultNeighborhoodProviderTest. Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use DefaultMoveFamily::*;
use timetuck::*;
#[test]
fn basic_variable() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap();
    assert_eq!(
        DefaultNeighborhood::default().basic_families(&p),
        [Change, Swap]
    );
}
#[test]
fn basic_variable_allows_unassigned() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]).allowing_unassigned()]).unwrap();
    assert_eq!(
        DefaultNeighborhood::default().basic_families(&p),
        [Change, Assign, Unassign, Swap]
    );
}
#[test]
fn list_variable() {
    let p = ListProblem::new(vec![0, 1], vec![ListEntity::new(vec![0, 1])]).unwrap();
    assert_eq!(
        DefaultNeighborhood::default().list_families(&p),
        [ListChange, ListSwap, TwoOpt, TailSwap]
    );
}
#[test]
fn list_variable_allows_unassigned_values() {
    let p = ListProblem::new(vec![0, 1], vec![ListEntity::new(vec![0])])
        .unwrap()
        .allowing_unassigned();
    assert_eq!(
        DefaultNeighborhood::default().list_families(&p),
        [
            ListChange,
            ListSwap,
            TwoOpt,
            TailSwap,
            ListAssign,
            ListUnassign
        ]
    );
}
