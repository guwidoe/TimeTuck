// Adapted from Community SelectorBasedNoChangeMoveTest, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::borrow::Cow;
use timetuck::*;

#[test]
fn is_move_doable() {
    let movement = Move::<i32>::Composite(vec![]);
    // The source passes null: no score director/context may be accessed.
    assert!(
        !movement
            .try_selector_is_doable(|_| -> Result<bool, ()> { panic!("no context") })
            .unwrap()
    );
}

#[test]
fn rebase() {
    let movement = Move::<i32>::Composite(vec![]);
    let rebased = movement
        .rebase_preserving_no_change(
            |_| -> Result<usize, ()> { panic!("empty rebasing lookup") },
            |_| -> Result<i32, ()> { panic!("empty rebasing lookup") },
        )
        .unwrap();
    assert!(matches!(rebased, Cow::Borrowed(_)));
    assert!(std::ptr::eq(rebased.as_ref(), &movement));
}
