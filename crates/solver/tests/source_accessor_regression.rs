//! Original selected-callable and shadow-runtime regressions; no source credit.
use std::{any::Any, sync::Arc};
use timetuck::*;
struct Solution;
#[derive(Clone, Debug)]
struct Node {
    id: usize,
    field: Option<Arc<Node>>,
    getter: Option<Arc<Node>>,
}
impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for Node {}
#[test]
fn chosen_getter_rewires_real_shadow_dependencies_and_supplier_results() {
    let schema = PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Node>(vec![
        PlanningVariableDefinition::shadow::<Node>("previous"),
        PlanningVariableDefinition::shadow::<i64>("finish"),
    ])])
    .unwrap();
    let mut registry = SourcePathRegistry::new(&schema);
    let selected = SourceAccessorOptions::default()
        .field(|n: &Node| n.field.as_deref())
        .getter(|n: &Node| n.getter.as_deref())
        .select::<Node>("previous", "previous", &SourceTypeHierarchy::default())
        .unwrap();
    registry
        .one_selected(
            "previous",
            SourceMemberRole::Previous,
            selected,
            &SourceTypeHierarchy::default(),
        )
        .unwrap();
    registry
        .one(
            "finish",
            SourceMemberRole::Declarative,
            |_: &Node| -> Option<&i64> {
                panic!("terminal getter is not used for dependency traversal")
            },
        )
        .unwrap();
    let path = registry
        .compile::<Node>("finish", "previous.finish")
        .unwrap();
    let previous = schema
        .entity::<Node>()
        .unwrap()
        .variable("previous")
        .unwrap();
    let finish = schema.entity::<Node>().unwrap().variable("finish").unwrap();
    let mut b = ShadowSessionBuilder::<usize, Vec<Arc<Node>>>::new(&schema);
    for id in 0..2 {
        b.derive(finish, id, move |_, _| Some((id as i64 + 1) * 10))
            .unwrap();
    }
    b.input(previous, 2, |w| w[2].getter.as_deref().cloned())
        .unwrap();
    let output = b
        .derive(finish, 2, |_, inputs| {
            Some(
                inputs
                    .for_field::<i64>(finish)
                    .unwrap()
                    .into_iter()
                    .flatten()
                    .sum::<i64>()
                    + 1,
            )
        })
        .unwrap();
    b.bind_source_paths(
        &output,
        vec![&path],
        |w| w[2].as_ref(),
        |node: &dyn Any| node.downcast_ref::<Node>().map(|n| n.id),
    )
    .unwrap();
    let mut session = b.build();
    let a = Arc::new(Node {
        id: 0,
        field: None,
        getter: None,
    });
    let c = Arc::new(Node {
        id: 1,
        field: None,
        getter: None,
    });
    session
        .update(&vec![
            a.clone(),
            c.clone(),
            Arc::new(Node {
                id: 2,
                field: Some(a.clone()),
                getter: Some(c.clone()),
            }),
        ])
        .unwrap();
    assert_eq!(session.read(&output).unwrap().value, Some(&21));
    session
        .update(&vec![
            a.clone(),
            c.clone(),
            Arc::new(Node {
                id: 2,
                field: Some(c),
                getter: Some(a),
            }),
        ])
        .unwrap();
    assert_eq!(session.read(&output).unwrap().value, Some(&11));
}
struct Base {
    amount: i64,
}
struct Sub {
    base: Base,
}
struct Root {
    field: Sub,
    getter: Sub,
    out: i64,
}
#[test]
fn selected_subtype_projects_to_canonical_schema_and_preserves_borrowed_identity() {
    let schema = PlanningSchema::new::<Solution>(vec![
        PlanningEntityDefinition::of::<Root>(vec![
            PlanningVariableDefinition::shadow::<Base>("previous"),
            PlanningVariableDefinition::shadow::<i64>("out"),
        ]),
        PlanningEntityDefinition::of::<Base>(vec![PlanningVariableDefinition::shadow::<i64>(
            "amount",
        )]),
    ])
    .unwrap();
    let mut hierarchy = SourceTypeHierarchy::default();
    hierarchy.register::<Sub, Base>(|s| &s.base).unwrap();
    let selected = SourceAccessorOptions::default()
        .field(|r: &Root| Some(&r.field.base))
        .getter(|r: &Root| Some(&r.getter))
        .select::<Root>("previous", "previous", &hierarchy)
        .unwrap();
    let mut registry = SourcePathRegistry::new(&schema);
    registry
        .one_selected("previous", SourceMemberRole::Previous, selected, &hierarchy)
        .unwrap();
    registry
        .one("amount", SourceMemberRole::Declarative, |b: &Base| {
            Some(&b.amount)
        })
        .unwrap();
    let path = registry.compile::<Root>("out", "previous.amount").unwrap();
    let root = Root {
        field: Sub {
            base: Base { amount: 1 },
        },
        getter: Sub {
            base: Base { amount: 9 },
        },
        out: 0,
    };
    assert_eq!(root.out, 0);
    let entities = path.value_entities(&root).unwrap();
    let entity = entities[0].downcast_ref::<Base>().unwrap();
    assert!(std::ptr::eq(entity, &root.getter.base));
    assert_eq!(entity.amount, 9);
    assert!(
        registry
            .one("previous", SourceMemberRole::Previous, |r: &Root| Some(
                &r.field.base
            ))
            .is_err()
    );
}
#[test]
fn hierarchy_cycles_duplicates_and_unprojectable_schema_types_fail_before_registration() {
    struct Top {
        base: Base,
    }
    struct Bottom {
        top: Top,
    }
    let mut hierarchy = SourceTypeHierarchy::default();
    hierarchy.register::<Bottom, Top>(|b| &b.top).unwrap();
    hierarchy.register::<Top, Base>(|t| &t.base).unwrap();
    assert!(hierarchy.is_assignable(PlanningType::of::<Bottom>(), PlanningType::of::<Base>()));
    assert_eq!(
        hierarchy.register::<Top, Base>(|t| &t.base),
        Err(SourceMemberError::DuplicateProjection)
    );
    assert_eq!(
        hierarchy.register::<Base, Bottom>(|_| panic!("cycle must not execute")),
        Err(SourceMemberError::TypeCycle)
    );
    let schema = PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Root>(vec![
        PlanningVariableDefinition::shadow::<Base>("previous"),
    ])])
    .unwrap();
    let mut registry = SourcePathRegistry::new(&schema);
    let wrong = SourceAccessorOptions::default()
        .getter(|r: &Root| Some(&r.out))
        .select::<Root>("previous", "previous", &hierarchy)
        .unwrap();
    assert!(
        registry
            .one_selected("previous", SourceMemberRole::Previous, wrong, &hierarchy)
            .is_err()
    );
    registry
        .one("previous", SourceMemberRole::Previous, |r: &Root| {
            Some(&r.getter.base)
        })
        .unwrap();
}
struct World {
    factor: i64,
    entity: Target,
}
struct Target {
    base: i64,
}
#[test]
fn solution_parameter_callback_runs_in_shadow_evaluation_with_actual_current_solution() {
    let schema = PlanningSchema::new::<World>(vec![PlanningEntityDefinition::of::<Target>(vec![
        PlanningVariableDefinition::basic::<i64>("factor", false),
        PlanningVariableDefinition::shadow::<i64>("result"),
    ])])
    .unwrap();
    let registry = SourcePathRegistry::new(&schema);
    let supplier = registry
        .solution_supplier::<Target, World, i64>("result", |e, w| Some(e.base + w.factor))
        .unwrap();
    let target = supplier.target();
    let factor = schema
        .entity::<Target>()
        .unwrap()
        .variable("factor")
        .unwrap();
    let mut b = ShadowSessionBuilder::<usize, World>::new(&schema);
    let input = b.input(factor, 0, |w| Some(w.factor)).unwrap();
    let output = b
        .derive(target, 0, move |w, _| supplier.evaluate(&w.entity, w))
        .unwrap();
    b.set_dependencies(&output, &[input.erase()]).unwrap();
    let mut session = b.build();
    session
        .update(&World {
            factor: 2,
            entity: Target { base: 7 },
        })
        .unwrap();
    assert_eq!(session.read(&output).unwrap().value, Some(&9));
    session
        .update(&World {
            factor: 20,
            entity: Target { base: 7 },
        })
        .unwrap();
    assert_eq!(session.read(&output).unwrap().value, Some(&27));
}
