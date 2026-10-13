// Pinned Community LookupManagerTest/LookupStrategyIdOrNoneTest and TEST fixtures.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::sync::Arc;
use timetuck::*;
trait InterfaceEntity: std::fmt::Debug {
    fn id(&self) -> String;
    fn value(&self) -> Option<i32>;
    fn set_value(&mut self, value: Option<i32>);
}
#[derive(Debug)]
struct Entity {
    id: String,
}
impl InterfaceEntity for Entity {
    fn id(&self) -> String {
        self.id.clone()
    }
    fn value(&self) -> Option<i32> {
        None
    }
    fn set_value(&mut self, _: Option<i32>) {}
}
#[test]
fn lookup_interface_entity() {
    let mut concrete = Entity { id: "0".into() };
    concrete.set_value(Some(1));
    assert_eq!(concrete.value(), None);
    let o: Arc<dyn InterfaceEntity> = Arc::new(concrete);
    let p: Arc<dyn InterfaceEntity> = Arc::new(Entity { id: "1".into() });
    let mut lookup = WorkingObjectLookup::<dyn InterfaceEntity, String>::new(
        WorkingLookupStrategy::IdOrNone,
        WorkingLookupSchema::id("id", |entity: &(dyn InterfaceEntity + 'static)| {
            Some(entity.id())
        }),
    );
    lookup.reset();
    for fact in [&o, &p] {
        lookup.add(fact.clone()).unwrap();
    }
    let external_o: Arc<dyn InterfaceEntity> = Arc::new(Entity { id: "0".into() });
    let external_p: Arc<dyn InterfaceEntity> = Arc::new(Entity { id: "1".into() });
    assert!(Arc::ptr_eq(&lookup.lookup(&external_o).unwrap(), &o));
    assert!(Arc::ptr_eq(&lookup.lookup(&external_p).unwrap(), &p));
    lookup.remove(&o).unwrap();
    lookup.remove(&p).unwrap();
}
#[derive(Debug)]
struct Parent {
    id: Option<i32>,
}
impl Parent {
    fn id(&self) -> Option<i32> {
        self.id
    }
}
struct Child {
    parent: Parent,
}
impl std::fmt::Debug for Child {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.parent.id() {
            Some(id) => write!(f, "id={id}"),
            None => f.write_str("id=null"),
        }
    }
}
#[test]
fn add_with_null_id_in_superclass() {
    let object = Arc::new(Child {
        parent: Parent { id: None },
    });
    let mut lookup = WorkingObjectLookup::new(
        WorkingLookupStrategy::IdOrNone,
        WorkingLookupSchema::id("id", |child: &Child| child.parent.id()),
    );
    let error = lookup.add_debug(object.clone()).unwrap_err();
    let message = error.to_string();
    assert!(message.contains("must not be null"));
    assert!(message.contains(std::any::type_name::<Child>()));
    assert!(message.contains(&format!("{object:?}")));
}
