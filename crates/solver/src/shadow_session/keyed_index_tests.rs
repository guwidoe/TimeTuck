//! Original dynamic registration and callback-free saved-candidate restoration.
use crate::*;
use std::{
    cell::Cell,
    hash::{Hash, Hasher},
    sync::Arc,
};
#[derive(Clone)]
struct Object {
    id: usize,
    value: i64,
    source: Option<Arc<Object>>,
}
#[derive(Clone)]
struct Key<'a> {
    id: usize,
    hashes: &'a Cell<usize>,
    fail: &'a Cell<bool>,
}
impl PartialEq for Key<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for Key<'_> {}
impl Hash for Key<'_> {
    fn hash<H: Hasher>(&self, h: &mut H) {
        assert!(!self.fail.get(), "unexpected hash during restore");
        self.hashes.set(self.hashes.get() + 1);
        self.id.hash(h)
    }
}
fn row(id: usize, value: i64, source: usize) -> Object {
    Object {
        id,
        value,
        source: Some(Arc::new(Object {
            id: source,
            value: 0,
            source: None,
        })),
    }
}
#[test]
fn indexed_dynamic_programs_restore_saved_registration_without_hashing() {
    struct Model;
    let schema = PlanningSchema::new::<Model>(vec![PlanningEntityDefinition::of::<Object>(vec![
        PlanningVariableDefinition::basic::<i64>("value", false),
        PlanningVariableDefinition::shadow::<i64>("output"),
    ])])
    .unwrap();
    let fields = schema.entity::<Object>().unwrap();
    let value = fields.variable("value").unwrap();
    let output = fields.variable("output").unwrap();
    let mut paths = SourcePathRegistry::new(&schema);
    paths
        .one("source", SourceMemberRole::Fact, |r: &Object| {
            r.source.as_deref()
        })
        .unwrap();
    paths
        .one(
            "value",
            SourceMemberRole::Variable,
            |_: &Object| -> Option<&i64> { panic!("terminal getter") },
        )
        .unwrap();
    let path = &paths.compile::<Object>("output", "source.value").unwrap();
    let hashes = &Cell::new(0);
    let fail = &Cell::new(false);
    let factories = &Cell::new(0);
    let projects = &Cell::new(0);
    let reads = &Cell::new(0);
    let mut graph = StreamGraphBuilder::<SimpleScore>::new();
    let source = graph.source::<usize, Vec<Object>>();
    let out = graph
        .map_dynamic_shadows(
            source.port(),
            |world| world.iter().map(|r| r.id).collect::<Vec<_>>(),
            |keys| {
                factories.set(factories.get() + 1);
                let mut builder = ShadowSessionBuilder::<Key<'_>, Vec<Object>>::new(&schema)
                    .with_keyed_source_index();
                let mut handles = vec![];
                for &id in keys {
                    let handle =
                        builder.derive(output, Key { id, hashes, fail }, move |_, deps| {
                            Some(
                                deps.for_field::<i64>(value)
                                    .unwrap()
                                    .into_iter()
                                    .flatten()
                                    .copied()
                                    .sum::<i64>(),
                            )
                        })?;
                    builder.bind_source_paths(
                        &handle,
                        vec![path],
                        move |w| w.iter().find(|r| r.id == id).unwrap(),
                        move |v| {
                            v.downcast_ref::<Object>().map(|r| Key {
                                id: r.id,
                                hashes,
                                fail,
                            })
                        },
                    )?;
                    handles.push(handle);
                }
                for &id in keys {
                    builder.input(value, Key { id, hashes, fail }, move |w| {
                        reads.set(reads.get() + 1);
                        Some(w.iter().find(|r| r.id == id).unwrap().value)
                    })?;
                }
                ShadowProgram::new(builder, move |_, session| {
                    projects.set(projects.get() + 1);
                    handles
                        .iter()
                        .map(|h| Ok(*session.read(h)?.value.unwrap()))
                        .sum::<Result<i64, ShadowSessionError>>()
                })
            },
        )
        .unwrap();
    let dataset = graph.dataset(&out).unwrap();
    graph.score("sum", &out, |v| SimpleScore::new(*v)).unwrap();
    let mut graph = graph.build();
    let mut tx = graph.transaction();
    tx.apply(
        &source,
        TupleChange::put(7, vec![row(1, 2, 2), row(2, 3, 1)]),
    )
    .unwrap();
    tx.apply(&source, TupleChange::put(8, vec![row(5, 9, 5)]))
        .unwrap();
    let saved = tx.capture().unwrap();
    drop(tx);
    for id in 20..35 {
        let mut tx = graph.transaction();
        tx.apply(&source, TupleChange::put(7, vec![row(id, id as i64, id)]))
            .unwrap();
        tx.apply(&source, TupleChange::retract(8)).unwrap();
        tx.commit().unwrap();
    }
    let before = (hashes.get(), factories.get(), projects.get(), reads.get());
    fail.set(true);
    graph.install(saved);
    assert_eq!(
        (hashes.get(), factories.get(), projects.get(), reads.get()),
        before
    );
    assert_eq!(graph.dataset_rows(&dataset).unwrap()[&7], 5);
    assert_eq!(graph.dataset_rows(&dataset).unwrap()[&8], 9);
    assert_eq!(graph.score(), SimpleScore::new(14));
    fail.set(false);
    let mut tx = graph.transaction();
    tx.apply(
        &source,
        TupleChange::put(7, vec![row(1, 12, 2), row(2, 13, 1)]),
    )
    .unwrap();
    tx.commit().unwrap();
    assert_eq!(factories.get(), before.1);
    assert_eq!(graph.dataset_rows(&dataset).unwrap()[&7], 25);
    assert_eq!(graph.score(), SimpleScore::new(34));
    graph.assert_consistent().unwrap();
}
