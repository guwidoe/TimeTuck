// Adapted pinned Community move-provider tests and TEST-only domain fixtures.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;
struct Solution;
struct Entity;
fn declarations() -> PlanningSchema {
    PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Entity>(vec![
        PlanningVariableDefinition::basic::<usize>("value", false),
    ])])
    .unwrap()
}
fn binding(schema: &PlanningSchema, count: usize) -> SchemaVariableSlots<'_> {
    SchemaVariableSlots {
        variable: schema
            .genuine_entity::<Entity>()
            .unwrap()
            .genuine_variable("value")
            .unwrap(),
        slots: (0..count).collect(),
    }
}
fn fixture(initial: &[usize]) -> FiniteProblem<usize> {
    FiniteProblem::new(
        initial
            .iter()
            .map(|&value| Variable::new(vec![0, 1]).with_initial(value))
            .collect(),
    )
    .unwrap()
}
fn pillar_successors(provider: &ScopedPillarMoveProvider) {
    let p = fixture(&[0, 0, 1, 1]);
    let a = p.initial_assignment();
    let moves: Vec<_> = provider.select(&p, &a).unwrap().collect();
    // Preserve both source orientations, even though their successors coincide.
    assert_eq!(moves.len(), 2);
    let left = Pillar::from_rows(vec![vec![0], vec![1]]).unwrap();
    let right = Pillar::from_rows(vec![vec![2], vec![3]]).unwrap();
    assert!(moves.contains(&left.swap_move(&right, &a).unwrap()));
    assert!(moves.contains(&right.swap_move(&left, &a).unwrap()));
    for movement in moves {
        let mut trial = a.clone();
        {
            let guard = p.apply(&mut trial, &movement).unwrap();
            assert_eq!(guard.values(), &[Some(1), Some(1), Some(0), Some(0)]);
        }
        assert_eq!(trial, a);
    }
}
#[test]
fn pillar_single_variable_constructor_matches_one_element_list() {
    let s = declarations();
    let field = binding(&s, 4);
    pillar_successors(&ScopedPillarMoveProvider::swap_field(&field).unwrap());
    pillar_successors(&ScopedPillarMoveProvider::swap_fields(&[field]).unwrap());
}
#[test]
fn subpillar_single_variable_constructor_matches_one_element_list() {
    let s = declarations();
    let field = binding(&s, 4);
    let exactly = || PillarSelectionPolicy::subsets(2, 2).unwrap();
    pillar_successors(
        &ScopedPillarMoveProvider::subpillar_swap_field(&field, exactly(), exactly()).unwrap(),
    );
    pillar_successors(
        &ScopedPillarMoveProvider::subpillar_swap_fields(&[field], exactly(), exactly()).unwrap(),
    );
}
#[test]
fn entity_single_variable_constructor_matches_one_element_list() {
    let s = declarations();
    let field = binding(&s, 3);
    // TEST generateSolution(2,3) cycles its two values across the three entities.
    let p = fixture(&[0, 1, 0]);
    let a = p.initial_assignment();
    for provider in [
        EntitySwapProvider::from_planning_field(&field, EntitySwapOrder::Unordered(vec![0, 1, 2]))
            .unwrap(),
        EntitySwapProvider::from_planning_fields(
            std::slice::from_ref(&field),
            EntitySwapOrder::Unordered(vec![0, 1, 2]),
        )
        .unwrap(),
    ] {
        let moves: Vec<_> = provider.select(&p, &a, 0).unwrap().collect();
        assert!(
            moves
                .iter()
                .any(|candidate| candidate.left == 0 && candidate.right == 1)
        );
        for candidate in moves {
            let mut trial = a.clone();
            let guard = p.apply(&mut trial, &candidate.movement).unwrap();
            assert_eq!(guard.values()[candidate.left], a.values()[candidate.right]);
            assert_eq!(guard.values()[candidate.right], a.values()[candidate.left]);
        }
    }
}
fn empty_error<T>(result: Result<T, FieldProviderError>) {
    let Err(error) = result else {
        panic!("empty selected field list accepted")
    };
    assert_eq!(error, FieldProviderError::EmptyFields);
    assert!(error.to_string().contains("is empty"));
}
#[test]
fn entity_empty_list_constructor_throws() {
    empty_error(EntitySwapProvider::from_planning_fields(
        &[],
        EntitySwapOrder::Unordered(vec![]),
    ));
}
#[test]
fn pillar_empty_list_constructor_throws() {
    empty_error(ScopedPillarMoveProvider::swap_fields(&[]));
}
#[test]
fn subpillar_empty_list_constructor_throws() {
    empty_error(ScopedPillarMoveProvider::subpillar_swap_fields(
        &[],
        PillarSelectionPolicy::subsets(2, 2).unwrap(),
        PillarSelectionPolicy::subsets(2, 2).unwrap(),
    ));
}
#[test]
fn constructor_rejects_explicit_crossing_null_on_non_unassigned_variable() {
    let s = declarations();
    let field = binding(&s, 2);
    assert!(matches!(
        ScopedPillarMoveProvider::subpillar_change_fields(
            &[field],
            PillarSelectionPolicy::subsets(2, 2).unwrap(),
            true,
        ),
        Err(FieldProviderError::Schema(
            SchemaMoveError::CrossingUnassigned(_)
        ))
    ));
}
#[test]
fn no_arg_constructor_never_exceeds_default_maximum_sublist_size() {
    // Pinned Community's public SubListSampler constant is 10, observed through
    // the binary-only constant probe. No production implementation inspected.
    let maximum = 10;
    let size = maximum * 3;
    let p = ListProblem::new(
        (0..size).collect(),
        vec![ListEntity::new((0..size).collect())],
    )
    .unwrap();
    let a = p.initial_assignment();
    let lengths = |provider: SampledSubListProvider| {
        provider
            .select(&p, &a, 0)
            .unwrap()
            .take(300)
            .map(|movement| {
                let ListMove::SubListChange { start, end, .. } = movement else {
                    panic!("required model yields sublist changes")
                };
                end - start
            })
            .collect::<Vec<_>>()
    };
    let default = lengths(SampledSubListProvider::default_change(0, 5000));
    assert_eq!(default.len(), 300);
    assert!(default.iter().all(|&length| length <= maximum));
    let explicit = lengths(SampledSubListProvider::new(
        SampledSubListNeighborhood::Change(SubListChangeConfig::new(2, size).unwrap()),
        0,
        5000,
    ));
    assert_eq!(explicit.len(), 300);
    assert!(explicit.iter().any(|&length| length > maximum));
}
