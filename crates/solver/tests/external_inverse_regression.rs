// Original nullable external-publication controls; no upstream method credit.
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
struct Solution;
fn schema() -> PlanningSchema {
    PlanningSchema::new::<Solution>(vec![
        PlanningEntityDefinition::of::<usize>(vec![PlanningVariableDefinition::basic::<u32>(
            "worker", true,
        )]),
        PlanningEntityDefinition::of::<u32>(vec![
            PlanningVariableDefinition::shadow::<Vec<usize>>("jobs"),
        ]),
    ])
    .unwrap()
}
#[test]
fn null_duplicate_and_reader_panic_preserve_publication_then_repair_retries() {
    let schema = schema();
    let field = schema
        .entity::<usize>()
        .unwrap()
        .variable("worker")
        .unwrap();
    let inverse = schema.entity::<u32>().unwrap().variable("jobs").unwrap();
    let mut registry = VariableStateRegistry::new(&schema);
    registry
        .reset_basic(field, [(0usize, Some(10u32)), (1, Some(20))])
        .unwrap();
    let mut supply =
        BasicStateSupply::new(registry.basic::<usize, u32>(field).unwrap(), [10, 20, 30]).unwrap();
    supply.externalize(inverse).unwrap();
    supply.refresh_changed().unwrap();
    let calls = Cell::new(0);
    let old = [vec![0], vec![1], vec![]];
    assert!(matches!(
        supply.refresh_external_inverse(|value| {
            calls.set(calls.get() + 1);
            if *value == 20 {
                None
            } else {
                Some(old[0].as_slice())
            }
        }),
        Err(BasicSupplyError::NullInverseCollection {
            entity: Some(1),
            value: 20,
            ..
        })
    ));
    assert_eq!(calls.get(), 2);
    assert_eq!(supply.published_inverse(&10), Some(&[0][..]));
    assert_eq!(supply.published_inverse(&20), Some(&[1][..]));
    let duplicates = [1usize, 1];
    assert!(matches!(
        supply.refresh_external_inverse(|value| if *value == 20 {
            Some(&duplicates)
        } else {
            Some(&[])
        }),
        Err(BasicSupplyError::DuplicateEntity(1))
    ));
    assert_eq!(supply.published_inverse(&10), Some(&[0][..]));
    assert!(
        catch_unwind(AssertUnwindSafe(|| supply.refresh_external_inverse(
            |value| {
                assert_ne!(*value, 20, "reader failed");
                Some(&[])
            }
        )))
        .is_err()
    );
    assert_eq!(supply.published_inverse(&20), Some(&[1][..]));
    // Refresh imports the actual stale external rows, not the previous internal
    // publication, and reports events whose before values match that storage.
    let stale = [vec![1usize], vec![0usize], vec![]];
    let update = supply
        .refresh_external_inverse(|value| Some(stale[(*value / 10 - 1) as usize].as_slice()))
        .unwrap();
    assert_eq!(update.shadows.len(), 2);
    assert_eq!(update.shadows[0].value, 10);
    assert_eq!(update.shadows[0].before, [1]);
    assert_eq!(update.shadows[0].after, [0]);
    assert_eq!(update.shadows[1].value, 20);
    assert_eq!(update.shadows[1].before, [0]);
    assert_eq!(update.shadows[1].after, [1]);
    assert!(update.notifications.is_empty());
    assert_eq!(supply.published_inverse(&30), Some(&[][..]));
    let empty: &[usize] = &[];
    let error = supply
        .validate_external_inverse(|value| if *value == 30 { None } else { Some(empty) })
        .unwrap_err();
    assert!(matches!(
        error,
        BasicSupplyError::NullInverseCollection {
            entity: None,
            value: 30,
            ..
        }
    ));
}

#[test]
fn unknown_assignment_and_missing_binding_reject_before_external_read() {
    let schema = schema();
    let field = schema
        .entity::<usize>()
        .unwrap()
        .variable("worker")
        .unwrap();
    let mut registry = VariableStateRegistry::new(&schema);
    registry
        .reset_basic(field, [(0usize, Some(10u32))])
        .unwrap();
    let mut supply =
        BasicStateSupply::new(registry.basic::<usize, u32>(field).unwrap(), [10]).unwrap();
    assert!(matches!(
        supply.validate_external_inverse(|_| panic!("unbound read")),
        Err(BasicSupplyError::Metadata(_))
    ));
    supply
        .externalize(schema.entity::<u32>().unwrap().variable("jobs").unwrap())
        .unwrap();
    registry
        .reset_basic(field, [(0usize, Some(99u32))])
        .unwrap();
    assert!(matches!(
        supply.refresh_external_inverse(|_| panic!("unknown read")),
        Err(BasicSupplyError::UnknownValue(99))
    ));
    assert_eq!(supply.published_inverse(&10), Some(&[][..]));
}

#[test]
fn preflight_validation_does_not_consume_pending_membership_changes() {
    let schema = schema();
    let field = schema
        .entity::<usize>()
        .unwrap()
        .variable("worker")
        .unwrap();
    let mut registry = VariableStateRegistry::new(&schema);
    registry
        .reset_basic(field, [(0usize, Some(10u32))])
        .unwrap();
    let mut supply =
        BasicStateSupply::new(registry.basic::<usize, u32>(field).unwrap(), [10, 20]).unwrap();
    supply
        .externalize(schema.entity::<u32>().unwrap().variable("jobs").unwrap())
        .unwrap();
    supply.refresh_changed().unwrap();
    registry
        .reset_basic(field, [(0usize, Some(20u32))])
        .unwrap();
    let external = [0usize];
    supply
        .validate_external_inverse(|value| {
            if *value == 10 {
                Some(&external)
            } else {
                Some(&[])
            }
        })
        .unwrap();
    assert_eq!(supply.published_inverse(&10), Some(&[0][..]));
    let update = supply.refresh_changed().unwrap();
    assert_eq!(update.shadows.len(), 2);
    assert!(update.shadows[0].after.is_empty());
    assert_eq!(update.shadows[1].after, [0]);
}

#[test]
fn clone_panics_during_import_and_staged_refresh_never_install_stale_rows() {
    use std::{cmp::Ordering, rc::Rc};
    #[derive(Debug)]
    struct Key {
        id: usize,
        fault: Rc<Cell<Option<usize>>>,
    }
    impl Clone for Key {
        fn clone(&self) -> Self {
            if let Some(left) = self.fault.get() {
                assert_ne!(left, 0, "clone failed");
                self.fault.set(Some(left - 1));
            }
            Self {
                id: self.id,
                fault: self.fault.clone(),
            }
        }
    }
    impl PartialEq for Key {
        fn eq(&self, other: &Self) -> bool {
            self.id == other.id
        }
    }
    impl Eq for Key {}
    impl PartialOrd for Key {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            Some(self.cmp(other))
        }
    }
    impl Ord for Key {
        fn cmp(&self, other: &Self) -> Ordering {
            self.id.cmp(&other.id)
        }
    }
    let schema = PlanningSchema::new::<Solution>(vec![
        PlanningEntityDefinition::of::<Key>(vec![PlanningVariableDefinition::basic::<u32>(
            "worker", true,
        )]),
        PlanningEntityDefinition::of::<u32>(vec![PlanningVariableDefinition::shadow::<Vec<Key>>(
            "jobs",
        )]),
    ])
    .unwrap();
    let field = schema.entity::<Key>().unwrap().variable("worker").unwrap();
    let mut failures = 0;
    for limit in 0..24 {
        let fault = Rc::new(Cell::new(None));
        let mut registry = VariableStateRegistry::new(&schema);
        registry
            .reset_basic(
                field,
                [(
                    Key {
                        id: 0,
                        fault: fault.clone(),
                    },
                    Some(10u32),
                )],
            )
            .unwrap();
        let mut supply =
            BasicStateSupply::new(registry.basic::<Key, u32>(field).unwrap(), [10]).unwrap();
        supply
            .externalize(schema.entity::<u32>().unwrap().variable("jobs").unwrap())
            .unwrap();
        supply.refresh_changed().unwrap();
        let stale = [Key {
            id: 1,
            fault: fault.clone(),
        }];
        fault.set(Some(limit));
        let result = catch_unwind(AssertUnwindSafe(|| {
            supply.refresh_external_inverse(|_| Some(&stale))
        }));
        fault.set(None);
        match result {
            Err(_) => failures += 1,
            Ok(update) => {
                let update = update.unwrap();
                assert_eq!(update.shadows[0].before[0].id, 1);
                assert_eq!(update.shadows[0].after[0].id, 0);
            }
        }
        assert_eq!(supply.published_inverse(&10).unwrap()[0].id, 0);
        assert!(supply.refresh_changed().unwrap().shadows.is_empty());
    }
    assert!(failures > 3);
}
