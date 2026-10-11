//! Original order-certificate regressions; no upstream source credit.
use std::{
    cell::Cell,
    hash::{Hash, Hasher},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;

#[test]
fn reconfiguration_clone_and_backward_valid_routes_preserve_complete_diagnostics() {
    let base = ListProblem::new_indexed(
        (0..96).collect(),
        vec![ListEntity::new(vec![0, 1, 2]), ListEntity::new(vec![5])],
    )
    .unwrap();
    let none = base.clone().with_dependencies(vec![vec![]; 96]).unwrap();
    let backward = ListAssignment::new(vec![vec![2, 1, 0], vec![5]]);
    assert!(none.validate(&backward).is_ok()); // Backward proof failure is not a cycle.
    let mut deps = vec![vec![]; 96];
    deps[1] = vec![0];
    deps[4] = vec![1];
    deps[5] = vec![4];
    let fixed = none.clone().with_dependencies(deps.clone()).unwrap();
    assert!(fixed.validate(&fixed.initial_assignment()).is_ok());
    assert_eq!(fixed.inconsistent_values(&backward).unwrap(), vec![0, 1, 5]);
    assert_eq!(
        fixed.clone().inconsistent_values(&backward),
        fixed.inconsistent_values(&backward)
    );
    assert!(none.validate(&backward).is_ok());
    assert!(
        fixed
            .clone()
            .with_dependencies(vec![vec![]; 96])
            .unwrap()
            .validate(&backward)
            .is_ok()
    );
    assert_eq!(
        fixed.inconsistent_values(&ListAssignment::new(vec![vec![96], vec![]])),
        Err(ListError::ValueOutsideDomain)
    );
    deps[0] = vec![1];
    assert!(
        matches!(base.clone().with_dependencies(deps), Err(ListError::Dependency(DependencyError::FixedCycle(v))) if v == vec![0,1])
    );
    let mut unknown = vec![vec![]; 96];
    unknown[0] = vec![96];
    assert!(matches!(
        base.with_dependencies(unknown),
        Err(ListError::Dependency(DependencyError::UnknownDependency {
            node: 0,
            dependency: 96
        }))
    ));
    assert_eq!(
        fixed.prepare_assignment(&backward).unwrap().lists(),
        &[vec![2], vec![]]
    );
}

#[derive(Clone, Debug)]
struct Value {
    id: usize,
    payload: usize,
    panic: Rc<Cell<bool>>,
    calls: Rc<Cell<usize>>,
}
impl PartialEq for Value {
    fn eq(&self, rhs: &Self) -> bool {
        self.id == rhs.id
    }
}
impl Eq for Value {}
impl Hash for Value {
    fn hash<H: Hasher>(&self, state: &mut H) {
        assert!(!self.panic.get(), "value hash");
        self.calls.set(self.calls.get() + 1);
        self.id.hash(state);
    }
}

#[test]
fn certified_and_fallback_trials_preserve_payloads_callback_order_and_hash_rollback() {
    let panic = Rc::new(Cell::new(false));
    let calls = Rc::new(Cell::new(0));
    let v = |id, payload| Value {
        id,
        payload,
        panic: panic.clone(),
        calls: calls.clone(),
    };
    let mut deps = vec![vec![]; 96];
    deps[1] = vec![0];
    let p = ListProblem::new_indexed(
        (0..96).map(|id| v(id, id)).collect(),
        vec![
            ListEntity::new(vec![v(0, 100), v(1, 101)]),
            ListEntity::new(vec![v(2, 102), v(3, 103)]),
        ],
    )
    .unwrap()
    .allowing_unassigned()
    .with_dependencies(deps)
    .unwrap();
    let a = p.initial_assignment();
    let score_calls = Cell::new(0);
    let mut network = ListConstraintNetwork::new(2, 96);
    network
        .add_constraint("payload", vec![ListScope::Entity(0)], |facts| {
            score_calls.set(score_calls.get() + 1);
            let ListFact::Entity(row) = facts[0] else {
                unreachable!()
            };
            SimpleScore::new(row.iter().map(|v: &Value| v.payload as i64).sum())
        })
        .unwrap();
    let mut director = ListScoreDirector::new(&p, &network, a.clone()).unwrap();
    let swap = |entity| ListMove::Swap {
        left: ListPosition { entity, index: 0 },
        right: ListPosition { entity, index: 1 },
    };
    let before = score_calls.get();
    assert!(
        matches!(director.apply(&swap(0)), Err(ListScoringError::Model(ListError::DependencyCycle { values })) if values == vec![0,1])
    );
    assert_eq!(score_calls.get(), before);
    {
        let trial = director.apply(&swap(1)).unwrap();
        assert_eq!(trial.assignment().lists()[1][0].payload, 103);
    }
    assert_eq!(director.assignment().lists()[1][0].payload, 102);
    let assign = ListMove::Assign {
        value: v(6, 106),
        destination: ListPosition {
            entity: 1,
            index: 2,
        },
    };
    {
        let trial = director.apply(&assign).unwrap();
        assert_eq!(trial.assignment().lists()[1][2].payload, 106);
    }
    panic.set(true);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = director.apply(&assign);
        }))
        .is_err()
    );
    panic.set(false);
    assert_eq!(director.assignment(), &a);
    director.assert_consistent().unwrap();
    let mut committed = a.clone();
    p.apply(&mut committed, &assign).unwrap().commit();
    assert_eq!(committed.lists()[1][2].payload, 106);
    assert!(p.validate(&committed).is_ok());
}
