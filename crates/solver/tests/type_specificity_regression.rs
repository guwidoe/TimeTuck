use std::{cell::Cell, rc::Rc};
use timetuck::*;
struct Root {
    domain: Vec<i64>,
}
struct Child {
    root: Root,
    domain: Vec<i64>,
}
struct GrandChild {
    child: Child,
}
fn builder() -> TypeSpecificDispatchBuilder<Vec<i64>> {
    let mut b = TypeSpecificDispatchBuilder::default();
    b.project(|c: &Child| &c.root).unwrap();
    b.project(|c: &GrandChild| &c.child).unwrap();
    b
}
#[test]
fn specificity_invokes_current_concrete_handler_and_feeds_real_solver() {
    let calls = Rc::new(Cell::new(0));
    let observed = calls.clone();
    let mut b = builder();
    b.register(|r: &Root| r.domain.clone()).unwrap();
    b.register(move |c: &Child| {
        observed.set(observed.get() + 1);
        c.domain.clone()
    })
    .unwrap();
    let dispatcher = b.compile().unwrap();
    let mut value = GrandChild {
        child: Child {
            root: Root { domain: vec![-50] },
            domain: vec![2, 7],
        },
    };
    let solve_domain = |domain: Vec<i64>| {
        let problem = FiniteProblem::new(vec![Variable::new(domain)]).unwrap();
        let mut score = ConstraintNetwork::new(1);
        score
            .add_constraint("value", vec![0], |values| {
                SimpleScore::new(values[0].copied().unwrap_or(0))
            })
            .unwrap();
        solve(
            &problem,
            &score,
            SolverConfig {
                local_search: vec![LocalSearchConfig {
                    max_steps: 3,
                    ..Default::default()
                }],
                ..Default::default()
            },
        )
        .unwrap()
        .score
    };
    assert_eq!(
        solve_domain(dispatcher.dispatch(&value).unwrap()),
        Some(SimpleScore::new(7))
    );
    value.child.domain = vec![1, 11];
    assert_eq!(
        solve_domain(dispatcher.dispatch(&value).unwrap()),
        Some(SimpleScore::new(11))
    );
    assert_eq!(calls.get(), 2);
    assert_eq!(dispatcher.dispatch(&value.child.root), Some(vec![-50]));
    assert!(dispatcher.dispatch(&"unrelated").is_none());
    assert_eq!(calls.get(), 2);
}
#[test]
fn registration_order_and_missing_intermediate_types_preserve_total_order() {
    let mut h = SourceTypeHierarchy::default();
    h.register(|c: &Child| &c.root).unwrap();
    h.register(|c: &GrandChild| &c.child).unwrap();
    let a = PlanningType::of::<Root>();
    let b = PlanningType::of::<Child>();
    let c = PlanningType::of::<GrandChild>();
    for input in [[a, b, c], [c, b, a], [b, a, c]] {
        let order = TypeSpecificityOrder::new(&h, input).unwrap();
        assert_eq!(order.ordered_types(), &[c, b, a]);
        for x in input {
            for y in input {
                assert_eq!(
                    order.compare(x, y).unwrap(),
                    order.compare(y, x).unwrap().reverse()
                );
            }
        }
    }
    let order = TypeSpecificityOrder::new(&h, [a, c]).unwrap();
    assert_eq!(order.ordered_types(), &[c, a]);
    assert!(order.compare(a, PlanningType::of::<String>()).is_err());
    assert!(TypeSpecificityOrder::new(&h, [a, a]).is_err());
}
#[test]
fn failed_duplicates_and_cycles_leave_existing_dispatch_executable() {
    let mut b = builder();
    b.register(|r: &Root| r.domain.clone()).unwrap();
    assert!(
        b.register(|_: &Root| panic!("duplicate must never replace callback"))
            .is_err()
    );
    assert!(b.project(|c: &Child| c).is_err());
    let d = b.compile().unwrap();
    let value = GrandChild {
        child: Child {
            root: Root { domain: vec![4, 9] },
            domain: vec![],
        },
    };
    assert_eq!(d.dispatch(&value), Some(vec![4, 9]));
}
