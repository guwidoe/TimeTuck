// Adapted AbstractSolutionClonerTest.cloneListVariableSolution, Apache-2.0.
#[path = "fixtures/list_clone_plan_fixture.rs"]
mod fixture;
use fixture::*;
use std::sync::Arc;
#[test]
fn clone_list_variable_solution() {
    let original = source(false);
    let copied = plan(false, false).copy(&original).unwrap();
    let clone = &copied.solution;
    assert!(!std::ptr::eq(&original, clone));
    assert_eq!(clone.values.as_ref(), original.values.as_ref());
    assert_eq!(clone.score, original.score);
    assert_eq!(clone.owners.len(), 2);
    assert!(!Arc::ptr_eq(&clone.owners, &original.owners));
    for (i, (code, names)) in [("a", vec!["1", "3"]), ("b", vec!["2"])]
        .into_iter()
        .enumerate()
    {
        let old = &original.owners[i];
        let new = &clone.owners[i];
        assert!(!Arc::ptr_eq(old, new));
        assert_eq!(new.code, code);
        let old_list = old.list.lock().unwrap().clone();
        let new_list = new.list.lock().unwrap().clone();
        assert!(!Arc::ptr_eq(&old_list, &new_list));
        assert_eq!(new_list.len(), names.len());
        assert_eq!(new_list.as_ref(), old_list.as_ref());
        for (v, name) in new_list.iter().zip(names) {
            assert_eq!(v.code, name);
        }
    }
    assert!(!Arc::ptr_eq(&clone.owners[0], &original.owners[0]));
    Arc::make_mut(&mut original.owners[0].list.lock().unwrap()).remove(0);
    let old = original.owners[0].list.lock().unwrap().clone();
    assert_eq!(old.len(), 1);
    assert_eq!(old[0].code, "3");
    let new = clone.owners[0].list.lock().unwrap().clone();
    assert_eq!(new.len(), 2);
    assert_eq!(new[0].code, "1");
}
