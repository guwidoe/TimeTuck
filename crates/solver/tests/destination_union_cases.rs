// Adapted from Community DefaultMoveStreamFactoryTest and DefaultNeighborhoodTest.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;

fn assert_positions(
    p: ListProblem<i32>,
    include_unassigned: bool,
    mut expected: Vec<Option<(usize, usize)>>,
) {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let sources = ListStreamSources::new(&mut b);
    let data = list_destination_dataset(&mut b, &sources, &p, include_unassigned).unwrap();
    let director =
        ListStreamScoreDirector::new(&p, b.build(), sources, p.initial_assignment()).unwrap();
    let mut actual: Vec<_> = data
        .iter(director.graph())
        .unwrap()
        .map(|p| p.map(|p| (p.entity, p.index)))
        .collect();
    actual.sort();
    expected.sort();
    assert_eq!(actual, expected);
    let mut direct: Vec<_> = list_destinations(&p, director.assignment(), include_unassigned)
        .unwrap()
        .map(|p| p.map(|p| (p.entity, p.index)))
        .collect();
    direct.sort();
    assert_eq!(direct, expected);
}
#[test]
fn for_each_destination_value_and_end_of_list_positions() {
    let p = ListProblem::new(
        vec![0, 1, 2, 3],
        vec![ListEntity::new(vec![0, 2]), ListEntity::new(vec![1, 3])],
    )
    .unwrap();
    assert_positions(
        p,
        false,
        vec![
            Some((0, 0)),
            Some((0, 1)),
            Some((1, 0)),
            Some((1, 1)),
            Some((0, 2)),
            Some((1, 2)),
        ],
    );
}
#[test]
fn for_each_destination_entity_provided_disjoint_value_ranges() {
    let p = ListProblem::new(
        vec![0, 1],
        vec![
            ListEntity::new(vec![0]).with_value_range(vec![0]),
            ListEntity::new(vec![1]).with_value_range(vec![1]),
        ],
    )
    .unwrap();
    assert_positions(
        p,
        false,
        vec![Some((0, 0)), Some((1, 0)), Some((0, 1)), Some((1, 1))],
    );
}
#[test]
fn for_each_destination_including_unassigned_adds_exactly_one_unassigned_row() {
    let p = ListProblem::new(vec![0, 1], vec![ListEntity::new(vec![0, 1])])
        .unwrap()
        .allowing_unassigned();
    assert_positions(
        p,
        true,
        vec![Some((0, 0)), Some((0, 1)), Some((0, 2)), None],
    );
}
#[test]
fn rejects_empty_move_provider_list() {
    let error = ProviderUnion::<(), (), usize>::new(vec![], UnionOrder::Sequential)
        .err()
        .unwrap();
    assert_eq!(error, EmptyMoveProviders);
    assert!(error.to_string().contains("at least one move provider"));
}
#[test]
fn accepts_nonempty_move_provider_list_without_calling_it() {
    let mut provider =
        |_: &(), _: &(), _: u64| -> Vec<usize> { panic!("construction must not invoke provider") };
    let expected = &provider as *const _ as *const ();
    let children: Vec<WeightedMoveProvider<'_, (), (), usize>> = vec![(&mut provider, 1)];
    let union = ProviderUnion::new(children, UnionOrder::Sequential).unwrap();
    let actual: Vec<_> = union.providers().collect();
    assert_eq!(actual.len(), 1);
    assert_eq!(actual[0].0 as *const _ as *const (), expected);
    assert_eq!(actual[0].1, 1);
}
