//! Original direct-handle resolution, signature and mixed-source regressions.
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use timetuck::*;
struct Model;
struct Entity;
fn schema() -> PlanningSchema {
    PlanningSchema::new::<Model>(vec![PlanningEntityDefinition::of::<Entity>(vec![
        PlanningVariableDefinition::basic::<i64>("input", true),
        PlanningVariableDefinition::shadow::<i64>("output"),
    ])])
    .unwrap()
}
#[test]
fn native_dynamic_bindings_support_nonclone_entity_keys() {
    #[derive(Eq, PartialEq)]
    struct Key(String);
    let schema = schema();
    let entity = schema.entity::<Entity>().unwrap();
    let mut builder = ShadowSessionBuilder::<Key, [i64; 2]>::new(&schema);
    let a = builder
        .input(entity.variable("input").unwrap(), Key("a".into()), |w| {
            Some(w[0])
        })
        .unwrap();
    let b = builder
        .input(entity.variable("input").unwrap(), Key("b".into()), |w| {
            Some(w[1])
        })
        .unwrap();
    let field = entity.variable("input").unwrap();
    let out = builder
        .derive(
            entity.variable("output").unwrap(),
            Key("output".into()),
            move |_, inputs| {
                Some(
                    inputs
                        .for_field::<i64>(field)
                        .unwrap()
                        .into_iter()
                        .flatten()
                        .sum::<i64>(),
                )
            },
        )
        .unwrap();
    let refs = [a.erase(), b.erase()];
    builder
        .bind_dependencies(&out, move |w| Ok(vec![refs[usize::from(w[0] < 0)].clone()]))
        .unwrap();
    let mut session = builder.build();
    session.update(&[3, 7]).unwrap();
    assert_eq!(session.read(&out).unwrap().value, Some(&3));
    let saved = session.checkpoint().unwrap();
    session.mark_dirty(&a).unwrap();
    session.mark_dirty(&out).unwrap();
    session.update_notified(&[-3, 7]).unwrap();
    assert_eq!(session.read(&out).unwrap().value, Some(&7));
    session.restore(&saved, &[3, 7]).unwrap();
    assert_eq!(session.read(&out).unwrap().value, Some(&3));
}
#[test]
fn native_signatures_keep_order_and_multiplicity_separate_from_deduplicated_edges() {
    let schema = schema();
    let entity = schema.entity::<Entity>().unwrap();
    let field = entity.variable("input").unwrap();
    let calls = Cell::new(0);
    let mut builder = ShadowSessionBuilder::<usize, usize>::new(&schema);
    let a = builder.input(field, 0, |_| Some(4i64)).unwrap();
    let b = builder.input(field, 1, |_| Some(9i64)).unwrap();
    let out = builder
        .derive(entity.variable("output").unwrap(), 0, |_, inputs| {
            calls.set(calls.get() + 1);
            let values = inputs.for_field::<i64>(field).unwrap();
            assert_eq!(values, [Some(&4), Some(&9)]);
            Some(values.into_iter().flatten().sum::<i64>())
        })
        .unwrap();
    let refs = [a.erase(), b.erase()];
    builder
        .bind_dependencies(&out, move |mode| {
            Ok(match mode {
                0 => vec![refs[0].clone(), refs[1].clone(), refs[0].clone()],
                1 => vec![refs[1].clone(), refs[0].clone(), refs[0].clone()],
                _ => vec![refs[0].clone(), refs[1].clone()],
            })
        })
        .unwrap();
    let mut session = builder.build();
    session.update(&0).unwrap();
    let saved = session.checkpoint().unwrap();
    session.update(&1).unwrap();
    assert_eq!(
        calls.get(),
        2,
        "reordered signature forces supplier despite equal edges"
    );
    assert!(session.changed_entities().unwrap().is_empty());
    assert_eq!(
        session.restore(&saved, &1),
        Err(ShadowSessionError::SnapshotMismatch)
    );
    session.mark_dirty(&out).unwrap();
    session.update_notified(&2).unwrap();
    assert_eq!(calls.get(), 3, "duplicate removal changes the signature");
    assert_eq!(session.read(&out).unwrap().value, Some(&13));
    session.restore(&saved, &0).unwrap();
    assert_eq!(calls.get(), 3, "checked restore does not run suppliers");
}
#[test]
fn native_identity_and_registration_bounds_survive_failed_refresh_and_restore() {
    let schema = schema();
    let field = schema
        .entity::<Entity>()
        .unwrap()
        .variable("output")
        .unwrap();
    let selected = RefCell::new(None::<ShadowNodeRef>);
    let mut builder = ShadowSessionBuilder::<usize, ()>::new(&schema);
    let first = builder.derive(field, 0, |_, _| Some(7i64)).unwrap();
    let out = builder
        .derive(field, 1, |_, values| {
            values
                .for_field::<i64>(field)
                .unwrap()
                .first()
                .copied()
                .flatten()
                .copied()
        })
        .unwrap();
    *selected.borrow_mut() = Some(first.erase());
    builder
        .bind_dependencies(&out, |_| Ok(vec![selected.borrow().clone().unwrap()]))
        .unwrap();
    let later = builder.derive(field, 2, |_, _| Some(9i64)).unwrap();
    let mut session = builder.build();
    session.update(&()).unwrap();
    let saved = session.checkpoint().unwrap();
    let mut other = ShadowSessionBuilder::<usize, ()>::new(&schema);
    let foreign = other.derive(field, 0, |_, _| Some(7i64)).unwrap();
    let mut other_session = other.build();
    other_session.update(&()).unwrap();
    let foreign_saved = other_session.checkpoint().unwrap();
    for (invalid, expected) in [
        (later.erase(), ShadowSessionError::UnknownNode),
        (foreign.erase(), ShadowSessionError::ForeignNode),
    ] {
        *selected.borrow_mut() = Some(invalid);
        assert_eq!(session.restore(&saved, &()), Err(expected.clone()));
        assert_eq!(session.read(&out).unwrap().value, Some(&7));
        assert_eq!(session.update(&()).err(), Some(expected.clone()));
        assert_eq!(session.read(&out).unwrap().value, Some(&7));
        session.mark_dirty(&out).unwrap();
        assert_eq!(session.update_notified(&()).err(), Some(expected));
        assert!(matches!(
            session.read(&out),
            Err(ShadowSessionError::NotCurrent)
        ));
        *selected.borrow_mut() = Some(first.erase());
        session.update_notified(&()).unwrap();
        assert_eq!(session.read(&out).unwrap().value, Some(&7));
    }
    assert_eq!(
        session.restore(&foreign_saved, &()),
        Err(ShadowSessionError::ForeignNode)
    );
    session.restore(&saved, &()).unwrap();
    assert_eq!(session.read(&out).unwrap().value, Some(&7));
}
#[test]
fn native_and_compiled_path_bindings_share_propagation_diagnostics_and_rollback() {
    struct Root {
        id: usize,
        group: Vec<Rc<Root>>,
    }
    struct World {
        roots: Vec<Rc<Root>>,
        values: [i64; 2],
        cycle: bool,
    }
    let schema = PlanningSchema::new::<Model>(vec![PlanningEntityDefinition::of::<Root>(vec![
        PlanningVariableDefinition::basic::<i64>("input", true),
        PlanningVariableDefinition::shadow::<i64>("output"),
    ])])
    .unwrap();
    let fields = schema.entity::<Root>().unwrap();
    let input = fields.variable("input").unwrap();
    let output = fields.variable("output").unwrap();
    let mut paths = SourcePathRegistry::new(&schema);
    paths
        .many("group", SourceMemberRole::Fact, |root: &Root| {
            root.group.iter().map(Rc::as_ref).collect()
        })
        .unwrap();
    paths
        .one(
            "input",
            SourceMemberRole::Variable,
            |_: &Root| -> Option<&i64> {
                panic!("terminal getter");
            },
        )
        .unwrap();
    let path = paths.compile::<Root>("output", "group[].input").unwrap();
    let mut builder = ShadowSessionBuilder::<usize, World>::new(&schema);
    let a = builder.input(input, 0, |w| Some(w.values[0])).unwrap();
    let _b = builder.input(input, 1, |w| Some(w.values[1])).unwrap();
    let keyed = builder
        .derive(output, 2, move |_, values| {
            Some(
                values
                    .for_field::<i64>(input)
                    .unwrap()
                    .into_iter()
                    .flatten()
                    .sum::<i64>(),
            )
        })
        .unwrap();
    builder
        .bind_source_paths(
            &keyed,
            vec![&path],
            |w| w.roots[2].as_ref(),
            |value| value.downcast_ref::<Root>().map(|root| root.id),
        )
        .unwrap();
    let native = builder
        .derive(output, 3, move |_, values| {
            Some(
                values
                    .for_field::<i64>(input)
                    .unwrap()
                    .into_iter()
                    .chain(values.for_field::<i64>(output).unwrap())
                    .flatten()
                    .sum::<i64>(),
            )
        })
        .unwrap();
    let refs = [a.erase(), keyed.erase(), native.erase()];
    builder
        .bind_dependencies(&native, move |world| {
            Ok(if world.cycle {
                vec![refs[2].clone()]
            } else {
                vec![refs[0].clone(), refs[1].clone()]
            })
        })
        .unwrap();
    let mut session = builder.build();
    let zero = Rc::new(Root {
        id: 0,
        group: vec![],
    });
    let one = Rc::new(Root {
        id: 1,
        group: vec![],
    });
    let mut world = World {
        roots: vec![
            zero.clone(),
            one.clone(),
            Rc::new(Root {
                id: 2,
                group: vec![zero, one],
            }),
        ],
        values: [3, 7],
        cycle: false,
    };
    update_consistent_shadows(&mut session, &world).unwrap();
    assert_eq!(session.read(&keyed).unwrap().value, Some(&10));
    assert_eq!(session.read(&native).unwrap().value, Some(&13));
    let saved = session.checkpoint().unwrap();
    world.values[0] = 5;
    session.mark_dirty(&a).unwrap();
    session.update_notified(&world).unwrap();
    assert_eq!(session.read(&keyed).unwrap().value, Some(&12));
    assert_eq!(session.read(&native).unwrap().value, Some(&17));
    world.cycle = true;
    assert!(matches!(
        update_consistent_shadows(&mut session, &world),
        Err(ConsistentShadowUpdateError::Inconsistent(_))
    ));
    assert_eq!(session.read(&native).unwrap().value, Some(&17));
    world.cycle = false;
    let good = world.roots[2].clone();
    world.roots[2] = Rc::new(Root {
        id: 2,
        group: vec![Rc::new(Root {
            id: 99,
            group: vec![],
        })],
    });
    let report = missing_shadow_sources(&session, &world).unwrap();
    assert_eq!(report.len(), 1);
    assert_eq!((report[0].target.entity, report[0].source.entity), (2, 99));
    assert_eq!(report[0].source.variable, "input");
    assert!(
        matches!(update_consistent_shadows(&mut session, &world), Err(ConsistentShadowUpdateError::MissingSources(missing)) if missing == report)
    );
    assert_eq!(session.read(&native).unwrap().value, Some(&17));
    world.roots[2] = good;
    world.values[0] = 3;
    session.restore(&saved, &world).unwrap();
    assert_eq!(session.read(&native).unwrap().value, Some(&13));
}
