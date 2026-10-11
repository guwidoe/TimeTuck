//! Original dense/sparse validation and initial fact reuse regressions.
use std::{
    cell::Cell,
    hash::{Hash, Hasher},
    rc::Rc,
};
use timetuck::*;

fn model(indexed: bool, count: usize, assigned: usize) -> ListProblem<usize> {
    let rows = vec![
        ListEntity::new((0..assigned).collect()),
        ListEntity::new(vec![]),
    ];
    if indexed {
        ListProblem::new_indexed((0..count).collect(), rows)
    } else {
        ListProblem::new((0..count).collect(), rows)
    }
    .unwrap()
}
#[test]
fn dense_sparse_tiny_and_word_boundaries_preserve_validation_errors() {
    for (n, assigned) in [(8, 8), (64, 64), (65, 65), (257, 257), (4096, 3)] {
        let direct = model(false, n, assigned);
        let indexed = model(true, n, assigned);
        let mut cases = vec![
            direct.initial_assignment(),
            ListAssignment::new(vec![vec![], vec![]]),
            ListAssignment::new(vec![]),
        ];
        for value in [0, assigned - 1, n - 1, n] {
            let mut rows = direct.initial_assignment().lists().to_vec();
            rows[1].push(value);
            cases.push(ListAssignment::new(rows));
        }
        for a in cases {
            assert_eq!(indexed.validate(&a), direct.validate(&a));
            assert_eq!(indexed.is_complete(&a), direct.is_complete(&a));
        }
    }
    for indexed in [false, true] {
        let rows = vec![
            ListEntity::new(vec![0]),
            ListEntity::new(vec![1]).with_value_range(vec![1]),
            ListEntity::new(vec![2]).with_pinned_prefix(1),
        ];
        let p = if indexed {
            ListProblem::new_indexed((0..128).collect(), rows)
        } else {
            ListProblem::new((0..128).collect(), rows)
        }
        .unwrap();
        for (rows, error) in [
            (
                vec![vec![0], vec![0], vec![]],
                ListError::ValueOutsideEntityRange { entity: 1 },
            ),
            (
                vec![vec![0, 0], vec![0], vec![]],
                ListError::DuplicateAssignedValue,
            ),
            (
                vec![vec![128, 0], vec![0], vec![]],
                ListError::ValueOutsideDomain,
            ),
            (
                vec![vec![0], vec![1], vec![0]],
                ListError::PinChanged { entity: 2 },
            ),
        ] {
            assert_eq!(p.validate(&ListAssignment::new(rows)), Err(error));
        }
    }
}
#[derive(Clone, Debug)]
struct Value {
    id: usize,
    payload: usize,
    hashes: Rc<Cell<usize>>,
    collision: bool,
}
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for Value {}
impl Hash for Value {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.hashes.set(self.hashes.get() + 1);
        if self.collision {
            0usize.hash(h)
        } else {
            self.id.hash(h)
        }
    }
}
#[test]
fn initial_scoring_reuses_positions_and_preserves_canonical_facts_and_payload_neighbors() {
    for collision in [false, true] {
        let hashes = Rc::new(Cell::new(0));
        let make = |id, payload| Value {
            id,
            payload,
            hashes: hashes.clone(),
            collision,
        };
        let p = ListProblem::new_indexed(
            (0..128).map(|i| make(i, 1000 + i)).collect(),
            vec![ListEntity::new(
                (0..128).map(|i| make(i, 2000 + i)).collect(),
            )],
        )
        .unwrap();
        let mut n = ListConstraintNetwork::<Value, SimpleScore>::new(1, 128);
        for i in 0..128 {
            n.add_constraint(
                format!("c{i}"),
                vec![ListScope::Value(i), ListScope::Value(i)],
                move |facts| {
                    for fact in facts {
                        let ListFact::Value { value, element } = fact else {
                            unreachable!()
                        };
                        assert_eq!(value.payload, 1000 + i);
                        let e = element.as_ref().unwrap();
                        assert_eq!(
                            e.position,
                            ListPosition {
                                entity: 0,
                                index: i
                            }
                        );
                        assert_eq!(
                            e.previous.map(|v| v.payload),
                            i.checked_sub(1).map(|i| 2000 + i)
                        );
                        assert_eq!(e.next.map(|v| v.payload), (i + 1 < 128).then_some(2001 + i));
                    }
                    SimpleScore::new(i as i64)
                },
            )
            .unwrap();
        }
        hashes.set(0);
        let d = ListScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
        assert_eq!(
            hashes.get(),
            128,
            "each assigned identity hashed once during validated initialization"
        );
        let expected = SimpleScore::new((0..128).sum());
        assert_eq!(d.score(), expected);
        d.assert_consistent().unwrap();
        hashes.set(0);
        assert_eq!(n.calculate(&p, d.assignment()).unwrap(), expected);
        assert_eq!(hashes.get(), 128);
        let mixed = MixedProblem::new(
            FiniteProblem::new(vec![Variable::new(vec![1]).with_initial(1)]).unwrap(),
            p,
        );
        let mut mn = MixedConstraintNetwork::<usize, Value, SimpleScore>::new(1, 1, 128);
        for i in 0..128 {
            mn.add_constraint(
                format!("c{i}"),
                vec![MixedScope::ListValue(i)],
                move |facts| {
                    let MixedFact::ListValue { element, .. } = &facts[0] else {
                        unreachable!()
                    };
                    SimpleScore::new(element.as_ref().unwrap().position.index as i64)
                },
            )
            .unwrap();
        }
        hashes.set(0);
        let md = MixedScoreDirector::new(&mixed, &mn, mixed.initial_assignment()).unwrap();
        assert_eq!(hashes.get(), 128);
        assert_eq!(md.score(), expected);
        md.assert_consistent().unwrap();
    }
}
#[test]
fn invalid_assignments_fail_before_callbacks_and_dependency_checks_remain_last() {
    let mut deps = vec![vec![]; 128];
    deps[0] = vec![1];
    let p = model(true, 128, 128).with_dependencies(deps).unwrap();
    let mut n = ListConstraintNetwork::<usize, SimpleScore>::new(2, 128);
    for i in 0..128 {
        n.add_constraint(format!("c{i}"), vec![ListScope::Value(i)], |_| {
            panic!("invalid assignment reached scoring")
        })
        .unwrap();
    }
    let cyclic = p.initial_assignment();
    assert!(matches!(
        p.validate(&cyclic),
        Err(ListError::DependencyCycle { .. })
    ));
    assert!(matches!(
        ListScoreDirector::new(&p, &n, cyclic),
        Err(ListScoringError::Model(ListError::DependencyCycle { .. }))
    ));
    let mut bad = p.initial_assignment().lists().to_vec();
    bad[0].push(0);
    assert!(matches!(
        ListScoreDirector::new(&p, &n, ListAssignment::new(bad)),
        Err(ListScoringError::Model(ListError::DuplicateAssignedValue))
    ));
    let mixed = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![1]).with_initial(1)]).unwrap(),
        p,
    );
    let mn = MixedConstraintNetwork::<usize, usize, SimpleScore>::new(1, 2, 128);
    let invalid = MixedAssignment::new(Assignment::new(vec![Some(2)]), ListAssignment::new(vec![]));
    assert!(matches!(
        MixedScoreDirector::new(&mixed, &mn, invalid),
        Err(MixedScoringError::Model(MixedError::Basic(_)))
    ));
}

#[test]
fn sparse_registry_initialization_retains_full_lookup_and_disabled_scope_behavior() {
    let p = model(true, 4096, 3);
    for enabled in [false, true] {
        let mut n = ListConstraintNetwork::<usize, SimpleScore>::new(2, 4096);
        for i in 0..512 {
            n.add_weighted_constraint(
                format!("c{i}"),
                vec![ListScope::Value(i)],
                SimpleScore::new(i64::from(enabled)),
                move |facts| {
                    assert!(enabled);
                    let ListFact::Value { element, .. } = &facts[0] else {
                        unreachable!()
                    };
                    assert_eq!(
                        element.as_ref().map(|e| e.position),
                        (i < 3).then_some(ListPosition {
                            entity: 0,
                            index: i
                        })
                    );
                    element.as_ref().map_or(0, |e| e.position.index as i64 + 1)
                },
            )
            .unwrap();
        }
        let d = ListScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
        assert_eq!(
            d.score(),
            SimpleScore::new(if enabled { 6 } else { 0 }).with_structural(-4093)
        );
        d.assert_consistent().unwrap();
    }
}
