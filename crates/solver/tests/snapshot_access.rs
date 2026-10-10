//! Original typed access, merge ordering, missing-entity and registration checks.
use std::collections::{BTreeMap, BTreeSet};
use timetuck::*;
struct Solution;
struct Entity;
fn schema() -> PlanningSchema {
    PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Entity>(vec![
        PlanningVariableDefinition::basic::<i32>("a", true),
        PlanningVariableDefinition::list::<i32>("b", true),
    ])])
    .unwrap()
}
#[test]
fn comparison_handles_interleaved_insertions_deletions_nulls_and_label_collisions() {
    let s = schema();
    let field = s.entity::<Entity>().unwrap().variable("a").unwrap();
    let mut t = SolutionTracker::new(&s);
    // Deliberately identical labels must not hide distinct typed values.
    t.scalar(
        field,
        |w: &BTreeMap<i32, Option<i32>>| w.clone(),
        |_| "same".into(),
    )
    .unwrap();
    for seed in 0..32 {
        let before: BTreeMap<_, _> = (-8..=8)
            .filter(|i| (i + seed) % 3 != 0)
            .map(|i| (i, if i % 2 == 0 { None } else { Some(i) }))
            .collect();
        let after: BTreeMap<_, _> = (-8..=8)
            .filter(|i| (i + seed) % 4 != 0)
            .map(|i| (i, if i % 2 == 0 { None } else { Some(i + seed) }))
            .collect();
        let a = t.snapshot(&before);
        let b = t.snapshot(&after);
        let expected: Vec<_> = before
            .keys()
            .chain(after.keys())
            .copied()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .filter(|k| before.get(k) != after.get(k))
            .collect();
        let actual = a
            .changed_variables(&b)
            .unwrap()
            .map(|(v, k)| {
                assert_eq!(v, field);
                *k
            })
            .collect::<Vec<_>>();
        assert_eq!(actual, expected);
        assert_eq!(a.mutation_count(&b).unwrap(), expected.len());
        let full = a.differences(&b).unwrap();
        assert_eq!(full.iter().map(|d| d.entity).collect::<Vec<_>>(), expected);
        assert_eq!(a.mutation_count(&b), b.mutation_count(&a));
        for difference in full {
            let expected_label = |v: Option<&Option<i32>>| match v {
                None => "<missing entity>",
                Some(None) => "null",
                Some(Some(_)) => "same",
            };
            assert_eq!(
                difference.expected,
                expected_label(before.get(&difference.entity))
            );
            assert_eq!(
                difference.actual,
                expected_label(after.get(&difference.entity))
            );
        }
    }
}
#[test]
fn getters_preserve_null_and_reject_foreign_metadata_or_wrong_types() {
    let s = schema();
    let a = s.entity::<Entity>().unwrap().variable("a").unwrap();
    let b = s.entity::<Entity>().unwrap().variable("b").unwrap();
    let mut t = SolutionTracker::new(&s);
    t.scalar(
        a,
        |w: &Option<i32>| BTreeMap::from([(0, *w)]),
        i32::to_string,
    )
    .unwrap();
    let snapshot = t.snapshot(&None);
    assert_eq!(
        snapshot.get::<Option<i32>>(a, &0).unwrap().unwrap().value,
        &None
    );
    assert!(snapshot.get::<Option<i32>>(a, &1).unwrap().is_none());
    assert!(matches!(
        snapshot.get::<Vec<i32>>(a, &0),
        Err(SolutionTrackingError::ValueType)
    ));
    assert!(matches!(
        snapshot.get::<Vec<i32>>(b, &0),
        Err(SolutionTrackingError::Metadata)
    ));
    let foreign = schema();
    let foreign_field = foreign.entity::<Entity>().unwrap().variable("a").unwrap();
    assert!(matches!(
        snapshot.get::<Option<i32>>(foreign_field, &0),
        Err(SolutionTrackingError::Metadata)
    ));
    let mut other = SolutionTracker::new(&s);
    other
        .scalar(
            a,
            |w: &Option<i32>| BTreeMap::from([(0, *w)]),
            i32::to_string,
        )
        .unwrap();
    assert!(matches!(
        snapshot.changed_variables(&other.snapshot(&None)),
        Err(SolutionTrackingError::ForeignSnapshot)
    ));
    assert_eq!(
        snapshot.mutation_count(&other.snapshot(&None)),
        Err(SolutionTrackingError::ForeignSnapshot)
    );
    t.list(
        b,
        |_: &Option<i32>| BTreeMap::from([(0, vec![1])]),
        i32::to_string,
    )
    .unwrap();
    assert_eq!(
        snapshot.mutation_count(&t.snapshot(&None)),
        Err(SolutionTrackingError::ForeignSnapshot)
    );
}
#[test]
fn multiple_fields_count_independently_and_snapshot_clones_share_identity() {
    let s = schema();
    let entity = s.entity::<Entity>().unwrap();
    let a = entity.variable("a").unwrap();
    let b = entity.variable("b").unwrap();
    let mut t = SolutionTracker::new(&s);
    // Register in reverse name order to distinguish registration from lexical order.
    t.list(
        b,
        |w: &(Option<i32>, Vec<i32>)| BTreeMap::from([(1, w.1.clone())]),
        i32::to_string,
    )
    .unwrap();
    t.scalar(
        a,
        |w: &(Option<i32>, Vec<i32>)| BTreeMap::from([(1, w.0)]),
        i32::to_string,
    )
    .unwrap();
    let before = t.snapshot(&(Some(1), vec![1, 2]));
    let after = t.snapshot(&(Some(2), vec![2, 1]));
    assert_eq!(
        before
            .changed_variables(&after)
            .unwrap()
            .collect::<Vec<_>>(),
        [(b, &1), (a, &1)]
    );
    assert_eq!(before.mutation_count(&after).unwrap(), 2);
    assert_eq!(before.clone().mutation_count(&before).unwrap(), 0);
}
