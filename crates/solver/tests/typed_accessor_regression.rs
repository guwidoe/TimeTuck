use std::{cell::Cell, rc::Rc, sync::Arc};
use timetuck::*;
#[derive(Debug, Eq, PartialEq)]
struct Value(i32);
struct Entity {
    value: Arc<Value>,
}
#[test]
fn property_identity_execution_and_error_before_callbacks() {
    let count = Rc::new(Cell::new(0));
    let read = count.clone();
    let write = count.clone();
    let a = TypedPropertyAccessor::bean(
        "value",
        "getValue",
        vec![],
        move |e: &Entity| {
            read.set(read.get() + 1);
            e.value.clone()
        },
        move |e: &mut Entity, v| {
            write.set(write.get() + 1);
            e.value = v;
        },
    )
    .unwrap();
    assert_eq!(count.get(), 0);
    assert_eq!(a.kind(), TypedAccessorKind::BeanProperty);
    assert!(a.annotations().is_empty());
    for n in [1, 17, -29] {
        let v = Arc::new(Value(n));
        let mut e = Entity { value: v.clone() };
        assert!(Arc::ptr_eq(&a.getter_function()(&e).unwrap(), &v));
        let replacement = Arc::new(Value(n));
        assert!(!Arc::ptr_eq(&replacement, &v));
        a.set(&mut e, replacement.clone()).unwrap();
        assert!(Arc::ptr_eq(&e.value, &replacement));
        assert!(Arc::ptr_eq(&a.get(&e).unwrap(), &replacement));
    }
    assert_eq!(count.get(), 9);
    let read = count.clone();
    let result = TypedPropertyAccessor::bean(
        "value",
        "value",
        vec![],
        move |e: &Entity| {
            read.set(read.get() + 1);
            e.value.clone()
        },
        |e: &mut Entity, v| e.value = v,
    );
    assert!(result.is_err());
    assert_eq!(count.get(), 9);
}
#[test]
fn borrowed_solution_projection_keeps_owner_address_and_current_data() {
    struct Base {
        values: Vec<Arc<Value>>,
    }
    struct Child {
        base: Base,
        extra: usize,
    }
    let calls = Rc::new(Cell::new(0));
    let count = calls.clone();
    let getter = TypedParameterAccessor::new("range", vec![], move |_: &(), s: &Base| {
        count.set(count.get() + 1);
        (s as *const Base as usize, s.values.clone())
    });
    let mut solution = Child {
        base: Base {
            values: vec![Arc::new(Value(3))],
        },
        extra: 7,
    };
    let pointer = &solution.base as *const Base as usize;
    assert!(getter.get(&(), None).is_err());
    assert!(getter.getter_function().is_err());
    assert_eq!(calls.get(), 0);
    let (observed, values) = getter.get(&(), Some(&solution.base)).unwrap();
    assert_eq!(observed, pointer);
    assert!(Arc::ptr_eq(&values[0], &solution.base.values[0]));
    solution.base.values = vec![Arc::new(Value(88)), Arc::new(Value(99))];
    let (observed, values) = getter.get(&(), Some(&solution.base)).unwrap();
    assert_eq!(observed, pointer);
    assert_eq!(values, solution.base.values);
    assert_eq!(calls.get(), 2);
    assert_eq!(solution.extra, 7);
}
