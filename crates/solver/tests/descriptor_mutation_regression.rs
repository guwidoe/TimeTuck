//! Original actual-move, selected-view and invalid-registration controls.
#[path = "fixtures/descriptor_mutation_fixture.rs"]
mod fixture;
use fixture::*;
use timetuck::*;
#[test]
fn real_applied_moves_count_inherited_and_child_only_occurrences_separately() {
    for (base_count, child_count, raw_count) in [(0, 0, 0), (2, 3, 4), (3, 7, 17)] {
        let c = composition();
        let layout = layout(base_count, child_count, raw_count);
        let p = problem(&layout);
        let tracker = tracker(&c, &p, &layout);
        let views = DescriptorMutationViews::all(&c).unwrap();
        let initial = p.initial_assignment();
        let before = tracker.snapshot(&initial);
        assert_eq!(
            views.compare(&before, &before).unwrap().occurrence_count(),
            0
        );
        for (slots, expected_unique, expected_occurrences) in [
            (vec![layout.base[0]], 1, 1),
            (vec![layout.child[0]], 1, 2),
            (vec![layout.child_sub[0]], 1, 1),
            (
                (0..p.variables().len()).collect(),
                p.variables().len(),
                layout.base.len() + 3 * layout.child.len(),
            ),
        ] {
            let movement = Move::Composite(
                slots
                    .iter()
                    .map(|&variable| Move::Change {
                        variable,
                        value: Some(2),
                    })
                    .collect(),
            );
            let mut actual = initial.clone();
            {
                let trial = p.apply(&mut actual, &movement).unwrap();
                let during = tracker.snapshot(&trial);
                let report = views.compare(&before, &during).unwrap();
                assert_eq!(report.unique_changes, expected_unique);
                assert_eq!(report.occurrence_count(), expected_occurrences);
            }
            assert_eq!(actual, initial);
            p.apply(&mut actual, &movement).unwrap().commit();
            let after = tracker.snapshot(&actual);
            assert_eq!(
                views.compare(&before, &after).unwrap().occurrence_count(),
                expected_occurrences
            );
            assert_eq!(before.mutation_count(&after).unwrap(), expected_unique);
        }
        let only_base = DescriptorMutationViews::new(&c, &[PlanningType::of::<Base>()]).unwrap();
        let mut actual = initial.clone();
        p.apply(
            &mut actual,
            &Move::Change {
                variable: layout.child_sub[0],
                value: Some(2),
            },
        )
        .unwrap()
        .commit();
        assert_eq!(
            only_base
                .compare(&before, &tracker.snapshot(&actual))
                .unwrap()
                .occurrence_count(),
            0
        );
    }
}
#[test]
fn invalid_views_foreign_snapshots_and_missing_real_field_registration_reject() {
    struct Unrelated;
    let c = composition();
    assert!(matches!(
        DescriptorMutationViews::new(
            &c,
            &[PlanningType::of::<Base>(), PlanningType::of::<Base>()]
        ),
        Err(DescriptorMutationError::DuplicateDescriptor(_))
    ));
    assert!(matches!(
        DescriptorMutationViews::new(&c, &[PlanningType::of::<Unrelated>()]),
        Err(DescriptorMutationError::UnknownDescriptor(_))
    ));
    let layout = layout(1, 1, 1);
    let p = problem(&layout);
    let tracker = tracker(&c, &p, &layout);
    let state = p.initial_assignment();
    let views = DescriptorMutationViews::all(&c).unwrap();
    let before = tracker.snapshot(&state);
    let second = fixture::tracker(&c, &p, &layout);
    assert!(matches!(
        views.compare(&before, &second.snapshot(&state)),
        Err(DescriptorMutationError::Snapshot(
            SolutionTrackingError::ForeignSnapshot
        ))
    ));
    let mut incomplete = SolutionTracker::new(c.schema());
    incomplete
        .scalar(
            c.schema()
                .entity::<Base>()
                .unwrap()
                .variable("value")
                .unwrap(),
            |_: &Assignment<i32>| std::collections::BTreeMap::from([(0usize, Some(1i32))]),
            i32::to_string,
        )
        .unwrap();
    let partial = incomplete.snapshot(&state);
    assert!(matches!(
        views.compare(&partial, &partial),
        Err(DescriptorMutationError::MissingSnapshotField { .. })
    ));
    let foreign_composition = composition();
    let foreign_views = DescriptorMutationViews::all(&foreign_composition).unwrap();
    assert!(matches!(
        foreign_views.compare(&before, &before),
        Err(DescriptorMutationError::MissingSnapshotField { .. })
    ));
}
