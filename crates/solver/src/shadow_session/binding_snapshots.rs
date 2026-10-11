//! Original checkpoint promotion and sparse/manual topology regression.
use super::*;
use crate::{PlanningEntityDefinition, PlanningVariableDefinition};
use std::panic::{AssertUnwindSafe, catch_unwind};
#[test]
fn restored_flat_checkpoints_reuse_promotion_after_panics_and_manual_rewires() {
    struct Model;
    struct Entity;
    let schema = PlanningSchema::new::<Model>(vec![PlanningEntityDefinition::of::<Entity>(vec![
        PlanningVariableDefinition::basic::<i64>("input", false),
        PlanningVariableDefinition::shadow::<i64>("output"),
    ])])
    .unwrap();
    let fields = schema.entity::<Entity>().unwrap();
    let mut b = ShadowSessionBuilder::<usize, (bool, bool)>::new(&schema);
    let inputs: Vec<_> = (0..40)
        .map(|i| {
            b.input(fields.variable("input").unwrap(), i, move |_| {
                Some(i as i64)
            })
            .unwrap()
        })
        .collect();
    let first = inputs[0].clone();
    let last = inputs[39].clone();
    let out = b
        .derive(fields.variable("output").unwrap(), 0, move |w, deps| {
            assert!(!w.1, "supplier panic");
            Some(*deps.get(if w.0 { &last } else { &first }).unwrap().unwrap())
        })
        .unwrap();
    let first = inputs[0].erase();
    let last = inputs[39].erase();
    b.bind_dependencies(&out, move |w| {
        Ok(vec![if w.0 { last.clone() } else { first.clone() }])
    })
    .unwrap();
    let source = out.clone();
    let tail = b
        .derive(fields.variable("output").unwrap(), 1, move |_, deps| {
            Some(deps.get(&source).unwrap().unwrap() + 1)
        })
        .unwrap();
    b.set_dependencies(&tail, &[out.erase()]).unwrap();
    let mut s = b.build();
    s.update(&(false, false)).unwrap();
    let saved = s.checkpoint().unwrap();
    let BindingState::Flat { sparse, .. } = saved.bindings.as_ref() else {
        panic!("initial flat state")
    };
    assert!(sparse.get().is_none());
    s.mark_dirty(&out).unwrap();
    assert!(catch_unwind(AssertUnwindSafe(|| s.update_notified(&(true, true)))).is_err());
    assert!(Rc::ptr_eq(&s.bindings, &saved.bindings));
    let promoted = sparse.get().unwrap() as *const SparseBindings;
    s.update_notified(&(true, false)).unwrap();
    assert_eq!(s.read(&tail).unwrap().value, Some(&40));
    for _ in 0..8 {
        s.install(&saved);
        s.mark_dirty(&out).unwrap();
        s.update_notified(&(true, false)).unwrap();
        assert!(std::ptr::eq(saved.bindings.sparse(), promoted));
        assert_eq!(s.read(&tail).unwrap().value, Some(&40));
        s.set_dependencies(&tail, &[tail.erase()]).unwrap();
        s.update_notified(&(true, false)).unwrap();
        assert!(s.read(&tail).unwrap().inconsistent);
        s.set_dependencies(&tail, &[out.erase()]).unwrap();
        s.update_notified(&(true, false)).unwrap();
        assert_eq!(s.read(&tail).unwrap().value, Some(&40));
    }
    s.restore(&saved, &(false, false)).unwrap();
    assert_eq!(s.read(&tail).unwrap().value, Some(&1));
    assert!(std::ptr::eq(saved.bindings.sparse(), promoted));
}
