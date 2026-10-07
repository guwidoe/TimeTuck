// Adapted from pinned Community selector-based ChangeMove, SwapMove and
// CompositeMove rebase tests. Apache-2.0; see compat/provenance.json.
use std::rc::Rc;
use timetuck::*;
fn destination(variable: usize) -> Result<usize, ()> {
    Ok([2, 0, 1][variable])
}
fn same_change(actual: Move<Rc<String>>, variable: usize, value: Option<&Rc<String>>) {
    let Move::Change {
        variable: actual_variable,
        value: actual_value,
    } = actual
    else {
        panic!("expected change")
    };
    assert_eq!(actual_variable, variable);
    match (actual_value, value) {
        (Some(a), Some(b)) => assert!(Rc::ptr_eq(&a, b)),
        (None, None) => {}
        _ => panic!("incorrect null identity"),
    }
}
#[test]
fn change_rebase() {
    let values = [Rc::new("v1".to_string()), Rc::new("v2".to_string())];
    for (variable, value) in [(0, None), (0, Some(0)), (1, None), (2, Some(1))] {
        let m = Move::Change { variable, value };
        let mapped = rebase_basic_move(&m, destination, |v| Ok(values[*v].clone())).unwrap();
        same_change(mapped, [2, 0, 1][variable], value.map(|v| &values[v]));
    }
}
#[test]
fn swap_rebase() {
    for (left, right) in [(0, 1), (0, 2), (1, 2)] {
        let movement: Move<usize> = Move::Swap { left, right };
        let mapped = rebase_basic_move(&movement, destination, |_| Ok::<usize, ()>(0)).unwrap();
        let Move::Swap {
            left: actual_left,
            right: actual_right,
        } = mapped
        else {
            panic!("expected swap")
        };
        assert_eq!(actual_left, [2, 0, 1][left]);
        assert_eq!(actual_right, [2, 0, 1][right]);
    }
}
#[test]
fn composite_rebase() {
    let values = [Rc::new("v1".to_string()), Rc::new("v2".to_string())];
    let m = Move::Composite(vec![
        Move::Change {
            variable: 0,
            value: Some(1usize),
        },
        Move::Change {
            variable: 1,
            value: Some(0),
        },
    ]);
    let Move::Composite(children) =
        rebase_basic_move(&m, destination, |v| Ok(values[*v].clone())).unwrap()
    else {
        panic!("expected composite")
    };
    assert_eq!(children.len(), 2);
    let mut children = children.into_iter();
    same_change(children.next().unwrap(), 2, Some(&values[1]));
    same_change(children.next().unwrap(), 0, Some(&values[0]));
}
