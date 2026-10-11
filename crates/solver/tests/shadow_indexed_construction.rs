//! Original indexed builder registration/grouping and failure contracts.
use std::{
    cell::Cell,
    hash::{Hash, Hasher},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;
struct Model;
struct Entity {
    id: usize,
}
struct Other;
#[derive(Default)]
struct Counts {
    equals: Cell<usize>,
    hashes: Cell<usize>,
    eq_panic: Cell<bool>,
    hash_panic: Cell<bool>,
    collision: bool,
}
#[derive(Clone)]
struct Key {
    id: usize,
    counts: Rc<Counts>,
}
impl PartialEq for Key {
    fn eq(&self, other: &Self) -> bool {
        assert!(!self.counts.eq_panic.get(), "key equality panic");
        self.counts.equals.set(self.counts.equals.get() + 1);
        self.id == other.id
    }
}
impl Eq for Key {}
impl Hash for Key {
    fn hash<H: Hasher>(&self, state: &mut H) {
        assert!(!self.counts.hash_panic.get(), "key hashing panic");
        self.counts.hashes.set(self.counts.hashes.get() + 1);
        (if self.counts.collision { 0 } else { self.id }).hash(state);
    }
}
fn key(id: usize, counts: &Rc<Counts>) -> Key {
    Key {
        id,
        counts: counts.clone(),
    }
}
fn schema() -> PlanningSchema {
    PlanningSchema::new::<Model>(vec![
        PlanningEntityDefinition::of::<Entity>(vec![
            PlanningVariableDefinition::basic::<i64>("input", false),
            PlanningVariableDefinition::shadow::<i64>("output"),
        ]),
        PlanningEntityDefinition::of::<Other>(vec![PlanningVariableDefinition::basic::<i64>(
            "input", false,
        )]),
    ])
    .unwrap()
}
fn describe<W>(
    s: &ShadowSession<'_, '_, Key, W>,
    groups: Vec<Vec<ShadowNodeRef>>,
) -> Vec<Vec<(PlanningType, usize, String)>> {
    groups
        .into_iter()
        .map(|g| {
            g.iter()
                .map(|h| {
                    let field = s.variable(h).unwrap();
                    (
                        field.entity().entity_type(),
                        s.entity(h).unwrap().id,
                        field.name().to_owned(),
                    )
                })
                .collect()
        })
        .collect()
}
#[test]
fn late_and_repeated_opt_in_keep_typed_groups_and_registration_order() {
    let schema = schema();
    let entity = schema.entity::<Entity>().unwrap();
    let input = entity.variable("input").unwrap();
    let output = entity.variable("output").unwrap();
    let other = schema.entity::<Other>().unwrap().variable("input").unwrap();
    for collision in [false, true] {
        let counts = Rc::new(Counts {
            collision,
            ..Counts::default()
        });
        let mut b = ShadowSessionBuilder::<Key, ()>::new(&schema);
        let a = b.input(input, key(3, &counts), |_| Some(3i64)).unwrap();
        let o = b.input(other, key(3, &counts), |_| Some(30i64)).unwrap();
        let d = b
            .derive(output, key(3, &counts), |_, _| Some(6i64))
            .unwrap();
        assert_eq!(counts.hashes.get(), 0);
        b = b.with_keyed_source_index();
        assert_eq!(counts.hashes.get(), 3);
        b = b.with_keyed_source_index();
        assert_eq!(counts.hashes.get(), 3, "repeated opt-in reuses its index");
        assert!(matches!(
            b.input(input, key(3, &counts), |_| Some(4i64)),
            Err(ShadowSessionError::DuplicateNode)
        ));
        let e = b
            .derive(output, key(1, &counts), |_, _| Some(2i64))
            .unwrap();
        let c = b.input(input, key(1, &counts), |_| Some(1i64)).unwrap();
        let p = b.input(other, key(1, &counts), |_| Some(10i64)).unwrap();
        let filter = b.entity_filter(entity, &[]).unwrap();
        assert_eq!(filter.entity_count(), 2);
        let mut s = b.build();
        let expected = vec![
            vec![a.erase(), d.erase()],
            vec![o.erase()],
            vec![e.erase(), c.erase()],
            vec![p.erase()],
        ];
        assert_eq!(
            describe(
                &s,
                s.affected_entities(&[
                    p.erase(),
                    c.erase(),
                    a.erase(),
                    o.erase(),
                    d.erase(),
                    p.erase()
                ])
                .unwrap()
            ),
            describe(&s, expected)
        );
        s.update(&()).unwrap();
        assert_eq!(s.read(&a).unwrap().value, Some(&3));
        assert_eq!(s.read(&p).unwrap().value, Some(&10));
        s.mark_entities_dirty(&[d.erase(), d.erase()]).unwrap();
        s.update_notified(&()).unwrap();
        assert!(s.changed_entities().unwrap().is_empty());
    }
}
#[test]
fn failed_hash_eq_duplicate_and_metadata_checks_do_not_publish_registration() {
    let foreign = schema();
    let schema = schema();
    let input = schema
        .entity::<Entity>()
        .unwrap()
        .variable("input")
        .unwrap();
    let wrong = foreign
        .entity::<Entity>()
        .unwrap()
        .variable("input")
        .unwrap();
    let counts = Rc::new(Counts {
        collision: true,
        ..Counts::default()
    });
    let mut b = ShadowSessionBuilder::<Key, ()>::new(&schema).with_keyed_source_index();
    let a = b.input(input, key(0, &counts), |_| Some(7i64)).unwrap();
    counts.hash_panic.set(true);
    assert!(matches!(
        b.input(wrong, key(1, &counts), |_| Some(8i64)),
        Err(ShadowSessionError::Metadata(_))
    ));
    assert!(
        catch_unwind(AssertUnwindSafe(
            || b.input(input, key(1, &counts), |_| Some(8i64))
        ))
        .is_err()
    );
    counts.hash_panic.set(false);
    counts.eq_panic.set(true);
    assert!(
        catch_unwind(AssertUnwindSafe(
            || b.input(input, key(1, &counts), |_| Some(8i64))
        ))
        .is_err()
    );
    counts.eq_panic.set(false);
    assert!(matches!(
        b.input(input, key(0, &counts), |_| Some(8i64)),
        Err(ShadowSessionError::DuplicateNode)
    ));
    let c = b.input(input, key(1, &counts), |_| Some(9i64)).unwrap();
    let mut s = b.try_build().unwrap();
    assert_eq!(
        describe(&s, s.affected_entities(&[c.erase(), a.erase()]).unwrap()),
        describe(&s, vec![vec![a.erase()], vec![c.erase()]])
    );
    s.update(&()).unwrap();
    assert_eq!(s.read(&a).unwrap().value, Some(&7));
    assert_eq!(s.read(&c).unwrap().value, Some(&9));
}
#[test]
fn indexed_duplicate_and_group_work_is_linear_without_collisions() {
    let schema = schema();
    let entity = schema.entity::<Entity>().unwrap();
    let input = entity.variable("input").unwrap();
    let output = entity.variable("output").unwrap();
    let n = 512;
    for indexed in [false, true] {
        let counts = Rc::new(Counts::default());
        let mut b = ShadowSessionBuilder::<Key, ()>::new(&schema);
        if indexed {
            b = b.with_keyed_source_index();
        }
        let mut handles = vec![];
        for i in 0..n {
            b.input(input, key(i, &counts), |_| Some(1i64)).unwrap();
        }
        for i in 0..n {
            handles.push(
                b.derive(output, key(i, &counts), |_, _| Some(2i64))
                    .unwrap(),
            );
        }
        if indexed {
            assert_eq!(counts.equals.get(), 0);
            assert_eq!(counts.hashes.get(), 2 * n);
        } else {
            assert_eq!(counts.equals.get(), n * (n - 1));
            assert_eq!(counts.hashes.get(), 0);
        }
        counts.equals.set(0);
        counts.hashes.set(0);
        let s = b.build();
        if indexed {
            assert_eq!(counts.equals.get(), n);
            assert_eq!(counts.hashes.get(), 2 * n);
        } else {
            assert_eq!(counts.equals.get(), n * n);
            assert_eq!(counts.hashes.get(), 0);
        }
        for h in handles {
            assert_eq!(s.entity_nodes(&h.erase()).unwrap().len(), 2);
        }
    }
}
#[test]
fn indexed_fixed_source_validation_preserves_missing_sources_and_cycles() {
    let schema = schema();
    let input = schema
        .entity::<Entity>()
        .unwrap()
        .variable("input")
        .unwrap();
    let output = schema
        .entity::<Entity>()
        .unwrap()
        .variable("output")
        .unwrap();
    let mut paths = SourcePathRegistry::new(&schema);
    paths
        .one(
            "input",
            SourceMemberRole::Variable,
            |_: &Entity| -> Option<&i64> { panic!("terminal") },
        )
        .unwrap();
    paths
        .one(
            "output",
            SourceMemberRole::Declarative,
            |_: &Entity| -> Option<&i64> { panic!("terminal") },
        )
        .unwrap();
    let fixed = paths.compile::<Entity>("output", "input").unwrap();
    let cyclic = paths.compile::<Entity>("output", "output").unwrap();
    for indexed in [false, true] {
        let mut b = ShadowSessionBuilder::<usize, Entity>::new(&schema);
        if indexed {
            b = b.with_keyed_source_index();
        }
        let d = b.derive(output, 0, |_, _| Some(4i64)).unwrap();
        b.bind_source_paths(
            &d,
            vec![&fixed],
            |w| w,
            |v| v.downcast_ref::<Entity>().map(|v| v.id),
        )
        .unwrap();
        assert!(matches!(
            b.validate_fixed_dependencies(),
            Err(ShadowSessionError::UnregisteredSource)
        ));
        b.input(input, 0, |_| Some(2i64)).unwrap();
        b.validate_fixed_dependencies().unwrap();
        let mut s = b.try_build().unwrap();
        s.update(&Entity { id: 0 }).unwrap();
        assert_eq!(s.read(&d).unwrap().value, Some(&4));
        let mut b = ShadowSessionBuilder::<usize, Entity>::new(&schema);
        let d = b.derive(output, 0, |_, _| Some(4i64)).unwrap();
        b.bind_source_paths(
            &d,
            vec![&cyclic],
            |w| w,
            |v| v.downcast_ref::<Entity>().map(|v| v.id),
        )
        .unwrap();
        if indexed {
            b = b.with_keyed_source_index();
        }
        assert!(
            matches!(b.try_build(), Err(ShadowSessionError::FixedDependencyCycle(names)) if names==["output"])
        );
    }
}
