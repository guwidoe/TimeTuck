use std::sync::Arc;
use timetuck::*;
trait JobView: std::fmt::Debug {
    fn id(&self) -> String;
    fn slot(&self) -> usize;
    fn value(&self) -> i32;
}
#[derive(Debug)]
struct Job {
    id: String,
    slot: usize,
    value: i32,
}
impl JobView for Job {
    fn id(&self) -> String {
        self.id.clone()
    }
    fn slot(&self) -> usize {
        self.slot
    }
    fn value(&self) -> i32 {
        self.value
    }
}
fn object(id: &str, slot: usize, value: i32) -> Arc<dyn JobView> {
    Arc::new(Job {
        id: id.into(),
        slot,
        value,
    })
}
#[test]
fn dynamic_canonical_lookup_drives_real_move_rebase_and_execution() {
    let mut lookup = WorkingObjectLookup::<dyn JobView, String>::new(
        WorkingLookupStrategy::IdOrFail,
        WorkingLookupSchema::id("id", |v: &(dyn JobView + 'static)| Some(v.id())),
    );
    let canonical = object("job", 1, 2);
    lookup.reset_with([canonical.clone()]).unwrap();
    let external = object("job", 99, -17);
    let rebased = rebase_basic_move(
        &Move::Change {
            variable: 0,
            value: Some("job".to_string()),
        },
        |_| lookup.lookup(&external).map(|v| v.slot()),
        |id| lookup.lookup(&object(id, 99, -17)).map(|v| v.value()),
    )
    .unwrap();
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 2]).with_initial(0); 2]).unwrap();
    let mut assignment = p.initial_assignment();
    p.apply(&mut assignment, &rebased).unwrap().commit();
    assert_eq!(assignment.values(), &[Some(0), Some(2)]);
    assert!(Arc::ptr_eq(&lookup.lookup(&external).unwrap(), &canonical));
    assert!(matches!(
        lookup.remove(&external),
        Err(WorkingLookupError::DifferentWorkingObject(_))
    ));
    lookup.remove(&canonical).unwrap();
    assert!(matches!(
        lookup.lookup(&external),
        Err(WorkingLookupError::UnknownId(_))
    ));
    assert!(
        rebase_basic_move(
            &Move::Change {
                variable: 0,
                value: Some("job".to_string())
            },
            |_| lookup.lookup(&external).map(|v| v.slot()),
            |id| lookup.lookup(&object(id, 99, -17)).map(|v| v.value())
        )
        .is_err()
    );
}
#[test]
fn trait_view_reset_is_atomic_and_real_inherited_id_diagnostics_do_not_publish() {
    let mut view = WorkingObjectLookup::<dyn JobView, String>::new(
        WorkingLookupStrategy::IdOrNone,
        WorkingLookupSchema::id("id", |v: &(dyn JobView + 'static)| Some(v.id())),
    );
    let current = object("old", 0, 1);
    view.add(current.clone()).unwrap();
    assert!(
        view.reset_with([object("duplicate", 1, 2), object("duplicate", 2, 3)])
            .is_err()
    );
    assert!(Arc::ptr_eq(
        &view.lookup(&object("old", 99, 99)).unwrap(),
        &current
    ));
    assert_eq!(view.len(), 1);
    #[derive(Debug)]
    struct Parent {
        key: Option<i32>,
    }
    impl Parent {
        fn key(&self) -> Option<i32> {
            self.key
        }
    }
    #[derive(Debug)]
    struct Child {
        parent: Parent,
        label: &'static str,
    }
    let mut typed = WorkingObjectLookup::new(
        WorkingLookupStrategy::IdOrNone,
        WorkingLookupSchema::id("inheritedKey", |child: &Child| child.parent.key()),
    );
    let good = Arc::new(Child {
        parent: Parent { key: Some(7) },
        label: "registered",
    });
    typed.add_debug(good.clone()).unwrap();
    let bad = Arc::new(Child {
        parent: Parent { key: None },
        label: "actual bad child",
    });
    let error = typed.add_debug(bad.clone()).unwrap_err();
    assert!(matches!(
        error.cause,
        WorkingLookupError::NullId {
            member: "inheritedKey",
            ..
        }
    ));
    assert!(error.object.contains(bad.label));
    assert!(error.to_string().contains(std::any::type_name::<Child>()));
    assert_eq!(typed.len(), 1);
    assert!(Arc::ptr_eq(&typed.lookup(&good).unwrap(), &good));
}
