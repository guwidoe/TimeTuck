// Original registered-value/backlink, optional/pinned and failure checks.
#[path = "fixtures/list_clone_plan_fixture.rs"]
mod fixture;
use fixture::*;
use std::sync::{Arc, Mutex};
use timetuck::*;
#[test]
fn registered_values_rebase_once_into_new_owners_and_keep_independent_shadows() {
    let original = source(true);
    let copied = plan(true, false).copy(&original).unwrap();
    let root = &copied.solution;
    assert_eq!(copied.entities.len(), 5);
    for (old, new) in original.values.iter().zip(root.values.iter()) {
        assert!(!Arc::ptr_eq(old, new));
        assert_eq!(old.code, new.code);
        let owner = new
            .owner
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .upgrade()
            .unwrap();
        assert!(root.owners.iter().any(|o| Arc::ptr_eq(o, &owner)));
        assert!(!original.owners.iter().any(|o| Arc::ptr_eq(o, &owner)));
        let index = new.index.lock().unwrap().unwrap();
        assert!(Arc::ptr_eq(&owner.list.lock().unwrap()[index], new));
        let native = copied.binding.problem().initial_assignment();
        assert_eq!(native.element(new).unwrap().position.index, index);
    }
    *original.values[0].index.lock().unwrap() = Some(99);
    assert_eq!(*root.values[0].index.lock().unwrap(), Some(0));
}
#[test]
fn optional_values_pins_and_owner_aliases_survive_rebinding() {
    let mut original = source(true);
    original.values = Arc::new(
        original
            .values
            .iter()
            .cloned()
            .chain([Arc::new(Value::new("extra"))])
            .collect(),
    );
    let first_list = original.owners[0].list.lock().unwrap().clone();
    original.owners = Arc::new(vec![
        Arc::new(Owner {
            code: "a",
            list: Mutex::new(first_list),
            prefix: 1,
        }),
        original.owners[1].clone(),
    ]);
    original.owners = Arc::new(
        original
            .owners
            .iter()
            .cloned()
            .chain([original.owners[0].clone()])
            .collect(),
    );
    update_shadows(&original);
    let copied = plan(true, true).copy(&original).unwrap();
    let root = &copied.solution;
    assert_eq!(copied.entities.len(), 6);
    assert!(Arc::ptr_eq(&root.owners[0], &root.owners[2]));
    assert_eq!(copied.binding.problem().entities()[0].pinned_prefix(), 1);
    assert_eq!(*root.values[3].index.lock().unwrap(), None);
    assert!(root.values[3].owner.lock().unwrap().is_none());
    assert!(
        copied
            .binding
            .problem()
            .initial_assignment()
            .element(&root.values[3])
            .is_none()
    );
}
#[test]
fn collapsed_value_mapping_and_altered_output_lists_are_rejected() {
    for collapse in [false, true] {
        let original = source(true);
        let layout = layout(true, false);
        let first = original.values[0].clone();
        let mut p = ListSolutionClonePlan::new(
            move |s| bind(layout.clone(), true, s),
            move |s: &Root, m: &BasicEntityClones| {
                let owners = m.collection(&s.owners)?;
                if !collapse {
                    let list = owners[0]
                        .list
                        .lock()
                        .unwrap()
                        .iter()
                        .rev()
                        .cloned()
                        .collect();
                    *owners[0].list.lock().unwrap() = Arc::new(list);
                }
                Ok(Root {
                    values: Arc::new(m.collection(&s.values)?),
                    owners: Arc::new(owners),
                    score: s.score,
                })
            },
            move |v: &Arc<Value>, m: &BasicEntityClones| {
                let out = m.entity(v)?;
                if collapse && v.code == "3" {
                    Ok(m.entity(&first)?)
                } else {
                    Ok(out)
                }
            },
            |s| s.score,
        );
        p.entity::<Value>(|v, _| Ok(Value::new(v.code))).unwrap();
        p.entity::<Owner>(|o, f| {
            Ok(Owner {
                code: o.code,
                list: Mutex::new(Arc::new(f[0].1.clone())),
                prefix: o.prefix,
            })
        })
        .unwrap();
        let error = p.copy(&original).err().unwrap();
        if collapse {
            assert!(
                matches!(
                    error,
                    ListSolutionCloneError::Binding(ListSolutionBindingError::Range(
                        ManagedRangeError::List(ListError::DuplicateAssignedValue)
                    ))
                ),
                "{error:?}"
            );
        } else {
            assert!(
                matches!(error, ListSolutionCloneError::AssignmentChanged),
                "{error:?}"
            );
        }
        assert_eq!(original.owners[0].list.lock().unwrap()[0].code, "1");
    }
}
