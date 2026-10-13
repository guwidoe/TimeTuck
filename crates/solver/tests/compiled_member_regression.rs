// Original native executable/caching controls; no source method credit.
use std::cell::Cell;
use std::rc::Rc;
use timetuck::*;
#[derive(Clone, Default)]
struct Entity {
    value: i64,
    calls: Cell<usize>,
}
fn read(e: &Entity) -> i64 {
    e.calls.set(e.calls.get() + 1);
    e.value
}
fn alternate(e: &Entity) -> i64 {
    -e.value
}
fn write(e: &mut Entity, v: i64) {
    e.value = v;
}
fn registration(read: fn(&Entity) -> i64) -> CompiledMemberRegistration<Entity, i64> {
    CompiledMemberRegistration::new(
        "getValue",
        CompiledMemberKind::Method,
        vec![CompiledMemberAnnotation::PlanningVariable {
            range_ids: vec!["range".into()],
        }],
        CompiledMemberBody::Read(read),
    )
    .with_setter(write)
}
#[test]
fn schema_cache_reuses_real_bindings_and_separates_code_and_annotations() {
    let mut factory = CompiledMemberFactory::default();
    let a = factory
        .build(registration(read), CompiledMemberRole::Read)
        .unwrap();
    let same = factory
        .build(registration(read), CompiledMemberRole::Read)
        .unwrap();
    assert!(Rc::ptr_eq(&a, &same));
    let b = factory
        .build(registration(alternate), CompiledMemberRole::Read)
        .unwrap();
    assert!(!Rc::ptr_eq(&a, &b));
    let mut e = Entity {
        value: 7,
        ..Default::default()
    };
    assert_eq!(a.getter_function().unwrap()(&e), 7);
    assert_eq!(b.read(&e, None).unwrap(), -7);
    a.write(&mut e, 9).unwrap();
    assert_eq!(b.read(&e, None).unwrap(), -9);
    assert_eq!(e.calls.get(), 1);
    let declaration: CompiledMemberRegistration<Entity, i64> = CompiledMemberRegistration::new(
        "getValue",
        CompiledMemberKind::Method,
        vec![CompiledMemberAnnotation::PlanningId],
        CompiledMemberBody::Read(read),
    );
    let different = factory
        .build(declaration, CompiledMemberRole::Read)
        .unwrap();
    assert!(!Rc::ptr_eq(a.code_type(), different.code_type()));
    assert_eq!(factory.binding_count(), 3);
    let other = CompiledMemberFactory::default()
        .build(registration(read), CompiledMemberRole::Read)
        .unwrap();
    assert!(!Rc::ptr_eq(&a, &other));
}
#[test]
fn real_signature_validation_precedes_invocation_and_cache_publication() {
    fn two(e: &Entity, _: &(), _: &()) -> i64 {
        read(e)
    }
    fn not_unit(e: &mut Entity) -> i64 {
        e.value = 99;
        99
    }
    let mut factory = CompiledMemberFactory::default();
    let declaration = CompiledMemberRegistration::new(
        "getValue",
        CompiledMemberKind::Method,
        vec![],
        CompiledMemberBody::ReadTwo(two),
    );
    assert!(
        factory
            .build(declaration, CompiledMemberRole::ReadWithParameter)
            .is_err()
    );
    let declaration: CompiledMemberRegistration<Entity, i64> = CompiledMemberRegistration::new(
        "update",
        CompiledMemberKind::Method,
        vec![],
        CompiledMemberBody::Unit(not_unit),
    );
    assert!(
        factory
            .build(declaration, CompiledMemberRole::Unit)
            .is_err()
    );
    assert!(
        factory
            .build(registration(read), CompiledMemberRole::Unit)
            .is_err()
    );
    assert_eq!(factory.binding_count(), 0);
}
#[test]
fn compiled_parameter_domains_and_setters_feed_actual_solver() {
    struct Solution {
        range: Vec<i64>,
    }
    fn range(_: &Entity, s: &Solution) -> Vec<i64> {
        s.range.clone()
    }
    let mut factory = CompiledMemberFactory::default();
    let domain = factory
        .build(
            CompiledMemberRegistration::new(
                "getRange",
                CompiledMemberKind::Method,
                vec![CompiledMemberAnnotation::ValueRangeProvider { id: "range".into() }],
                CompiledMemberBody::ReadOne(range),
            ),
            CompiledMemberRole::ReadWithParameter,
        )
        .unwrap();
    let value = factory
        .build(registration(read), CompiledMemberRole::Read)
        .unwrap();
    let mut solution = Solution {
        range: vec![1, 2, 3],
    };
    let mut entity = Entity {
        value: 1,
        ..Default::default()
    };
    let run = |entity: &Entity, solution: &Solution| {
        let p = FiniteProblem::new(vec![
            Variable::new(domain.read(entity, Some(solution)).unwrap())
                .with_initial(value.read(entity, None).unwrap()),
        ])
        .unwrap();
        let mut n = ConstraintNetwork::new(1);
        n.add_constraint("value", vec![0], |v| SimpleScore::new(*v[0].unwrap()))
            .unwrap();
        solve(
            &p,
            &n,
            SolverConfig {
                local_search: vec![LocalSearchConfig {
                    max_steps: 3,
                    ..Default::default()
                }],
                ..Default::default()
            },
        )
        .unwrap()
    };
    let result = run(&entity, &solution);
    assert_eq!(result.score, Some(SimpleScore::new(3)));
    value
        .write(
            &mut entity,
            *result.assignment.values()[0].as_ref().unwrap(),
        )
        .unwrap();
    assert_eq!(entity.value, 3);
    solution.range = vec![3, 4];
    let result = run(&entity, &solution);
    assert_eq!(result.score, Some(SimpleScore::new(4)));
    value
        .write(
            &mut entity,
            *result.assignment.values()[0].as_ref().unwrap(),
        )
        .unwrap();
    assert_eq!(entity.value, 4);
}
