use timetuck::*;
fn field(entity: usize, variable: &str) -> VerificationShadowRef<usize> {
    VerificationShadowRef {
        entity,
        variable: variable.into(),
    }
}
fn shadow(name: &str, refs: Vec<VerificationShadowRef<usize>>) -> VerificationShadow<usize> {
    VerificationShadow {
        variable: name.into(),
        dependencies: refs,
    }
}
fn entity(
    id: usize,
    inconsistent: Option<bool>,
    shadows: Vec<VerificationShadow<usize>>,
) -> VerificationEntity<usize> {
    VerificationEntity {
        id,
        inconsistent,
        shadows,
    }
}
#[test]
fn independent_fields_do_not_create_false_entity_cycles() {
    let entities = vec![
        entity(
            0,
            None,
            vec![shadow("a", vec![field(1, "a")]), shadow("b", vec![])],
        ),
        entity(
            1,
            None,
            vec![shadow("a", vec![]), shadow("b", vec![field(0, "b")])],
        ),
    ];
    let result = verification_consistency(&entities, [1, 0, 0]).unwrap();
    assert_eq!(
        result.iter().collect::<Vec<_>>(),
        vec![(&0, false), (&1, false)]
    );
    assert_eq!(result.is_inconsistent(&2), None);
}
#[test]
fn cycles_propagate_to_dependents_and_explicit_flags_override_only_their_entity() {
    let mut entities = vec![
        entity(0, Some(false), vec![shadow("a", vec![field(1, "a")])]),
        entity(1, None, vec![shadow("a", vec![field(0, "a")])]),
        entity(2, None, vec![shadow("a", vec![field(0, "a")])]),
        entity(3, Some(true), vec![]),
    ];
    let before = entities.clone();
    let old = verification_consistency(&entities, 0..4).unwrap();
    assert_eq!(
        old.iter().map(|(_, v)| v).collect::<Vec<_>>(),
        vec![false, true, true, true]
    );
    assert_eq!(entities, before);
    entities[1].shadows[0].dependencies.clear();
    let fresh = verification_consistency(&entities, 0..4).unwrap();
    assert_eq!(
        fresh.iter().map(|(_, v)| v).collect::<Vec<_>>(),
        vec![false, false, false, true]
    );
    assert_eq!(old.is_inconsistent(&1), Some(true));
}
#[test]
fn complete_missing_reference_report_is_deduplicated_bounded_and_cycle_safe() {
    let mut entities: Vec<_> = (0..9)
        .map(|id| {
            entity(
                id,
                Some(false),
                vec![shadow(
                    "a",
                    vec![field((id + 1) % 9, "a"), field((id + 1) % 9, "a")],
                )],
            )
        })
        .collect();
    let err = verification_consistency(&entities, [0]).unwrap_err();
    let text = err.to_string();
    let VerificationConsistencyError::MissingEntities(refs) = err else {
        panic!()
    };
    assert_eq!(refs.len(), 8);
    assert!(text.contains("3 more references"));
    assert_eq!(refs[7].reference, field(8, "a"));
    // Metadata for an unknown target need not be available to identify that edge.
    entities.truncate(1);
    assert!(
        matches!(verification_consistency(&entities,[0]),Err(VerificationConsistencyError::MissingEntities(v)) if v.len()==1 && v[0].reference==field(1,"a"))
    );
}
#[test]
fn malformed_metadata_is_rejected_and_unrelated_entities_are_not_selected() {
    let e = entity(0, None, vec![shadow("a", vec![])]);
    assert_eq!(
        verification_consistency(&[e.clone(), e.clone()], [0]),
        Err(VerificationConsistencyError::DuplicateEntity(0))
    );
    assert_eq!(
        verification_consistency(std::slice::from_ref(&e), [1]),
        Err(VerificationConsistencyError::UnknownGiven(1))
    );
    let duplicate = entity(0, None, vec![shadow("a", vec![]), shadow("a", vec![])]);
    assert_eq!(
        verification_consistency(&[duplicate], [0]),
        Err(VerificationConsistencyError::DuplicateShadow(field(0, "a")))
    );
    let broken = entity(1, None, vec![shadow("a", vec![field(1, "missing")])]);
    assert!(
        matches!(verification_consistency(std::slice::from_ref(&broken),[1]),Err(VerificationConsistencyError::UnknownShadow(v)) if v.reference==field(1,"missing"))
    );
    assert_eq!(
        verification_consistency(&[e, broken], [0])
            .unwrap()
            .iter()
            .collect::<Vec<_>>(),
        vec![(&0, false)]
    );
    assert_eq!(
        verification_consistency::<usize>(&[], [])
            .unwrap()
            .iter()
            .count(),
        0
    );
}
#[test]
fn long_chains_are_prepared_without_recursive_traversal() {
    let entities: Vec<_> = (0..4000)
        .map(|id| {
            entity(
                id,
                None,
                vec![shadow(
                    "a",
                    if id == 0 {
                        vec![]
                    } else {
                        vec![field(id - 1, "a")]
                    },
                )],
            )
        })
        .collect();
    assert!(
        verification_consistency(&entities, 0..4000)
            .unwrap()
            .iter()
            .all(|(_, flag)| !flag)
    );
}
