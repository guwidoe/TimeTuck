//! Original indexed-domain differential checks; no upstream source credit.
use std::{
    cell::Cell,
    hash::{Hash, Hasher},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;
fn variables() -> Vec<Variable<usize>> {
    vec![
        Variable::new((0..128).rev().collect()).with_initial(100),
        Variable::new((64..192).rev().collect())
            .with_initial(90)
            .allowing_unassigned(),
        Variable::new((0..128).rev().collect()).allowing_unassigned(),
        Variable::new((32..160).rev().collect())
            .with_initial(100)
            .with_pinned(true),
    ]
}
#[test]
fn ordered_neighborhoods_borrowed_checks_and_retained_trials_match_direct_domains() {
    let plain = FiniteProblem::new(variables()).unwrap();
    let indexed = FiniteProblem::new_indexed(variables()).unwrap();
    let before = plain.initial_assignment();
    assert_eq!(plain.search_space_size(), indexed.search_space_size());
    let collect = |p: &FiniteProblem<usize>| {
        change_moves(p, &before)
            .unwrap()
            .chain(swap_moves(p, &before).unwrap())
            .chain(pillar_swap_moves(p, &before).unwrap())
            .collect::<Vec<_>>()
    };
    let mut moves = collect(&plain);
    assert_eq!(collect(&indexed), moves);
    moves.extend([
        Move::Composite(vec![
            Move::Change {
                variable: 0,
                value: Some(9999),
            },
            Move::Change {
                variable: 0,
                value: Some(100),
            },
        ]),
        Move::Change {
            variable: 1,
            value: Some(0),
        },
        Move::Change {
            variable: 3,
            value: Some(101),
        },
        Move::Swap { left: 99, right: 0 },
        Move::Change {
            variable: 0,
            value: None,
        },
    ]);
    let plain_check = plain.move_checker(&before).unwrap();
    let indexed_check = indexed.move_checker(&before).unwrap();
    let mut net = ConstraintNetwork::<usize, SimpleScore>::new(4);
    net.add_constraint("sum", vec![0, 1, 2, 3], |values| {
        SimpleScore::new(values.iter().flatten().map(|v| **v as i64).sum())
    })
    .unwrap();
    let mut a = ScoreDirector::new(&plain, &net, before.clone()).unwrap();
    let mut b = ScoreDirector::new(&indexed, &net, before.clone()).unwrap();
    for movement in moves {
        assert_eq!(
            plain_check.is_doable(&movement),
            indexed_check.is_doable(&movement)
        );
        match (a.apply(&movement), b.apply(&movement)) {
            (Ok(a), Ok(b)) => {
                assert_eq!(a.assignment(), b.assignment());
                assert_eq!(a.score(), b.score());
            }
            (Err(a), Err(b)) => assert_eq!(a, b),
            _ => panic!("different validity"),
        }
        assert_eq!(a.assignment(), &before);
        assert_eq!(b.assignment(), &before);
    }
    a.assert_consistent().unwrap();
    b.assert_consistent().unwrap();
    let p = MixedProblem::new(
        indexed,
        ListProblem::new(vec![0, 1], vec![ListEntity::new(vec![0, 1])]).unwrap(),
    );
    let before = p.initial_assignment();
    let net = MixedConstraintNetwork::<_, _, SimpleScore>::new(4, 1, 2);
    let mut d = MixedScoreDirector::new(&p, &net, before.clone()).unwrap();
    {
        let trial = d
            .apply(&MixedMove {
                basic: Move::Change {
                    variable: 0,
                    value: Some(127),
                },
                lists: ListMove::SubListChange {
                    entity: 0,
                    start: 0,
                    end: 2,
                    destination: ListPosition {
                        entity: 0,
                        index: 0,
                    },
                    reverse: true,
                },
            })
            .unwrap();
        assert_eq!(trial.assignment().basic().values()[0], Some(127));
        assert_eq!(trial.assignment().lists().lists()[0], vec![1, 0]);
    }
    assert_eq!(d.assignment(), &before);
    d.assert_consistent().unwrap();
}
#[test]
fn construction_errors_and_owned_diagnostics_keep_variable_and_error_order() {
    for vars in [
        vec![
            Variable::new((0..128).collect()).with_initial(999),
            Variable::new(vec![1, 1]),
        ],
        vec![Variable::new((0..128).chain([127]).collect()).with_initial(999)],
        vec![
            Variable::new(vec![]).allowing_unassigned(),
            Variable::new((0..128).chain([0]).collect()),
        ],
        vec![
            Variable::new((0..128).collect())
                .with_initial(999)
                .with_range_diagnostics("A", "value", |v| format!("id {v}")),
        ],
    ] {
        assert_eq!(
            FiniteProblem::new(vars.clone()).unwrap_err(),
            FiniteProblem::new_indexed(vars).unwrap_err()
        );
    }
    let vars = vec![
        Variable::new((0..128).collect())
            .with_initial(0)
            .with_pinned(true)
            .with_range_diagnostics("A", "value", |v| format!("id {v}")),
    ];
    let p = FiniteProblem::new_indexed(vars.clone()).unwrap();
    let q = FiniteProblem::new(vars).unwrap();
    for a in [
        Assignment::new(vec![]),
        Assignment::new(vec![Some(999)]),
        Assignment::new(vec![Some(1)]),
        Assignment::new(vec![None]),
    ] {
        assert_eq!(p.validate(&a), q.validate(&a));
    }
}
#[derive(Clone, Debug)]
struct Value {
    id: usize,
    payload: &'static str,
    collision: bool,
    hashes: Rc<Cell<usize>>,
    bomb: Rc<Cell<bool>>,
}
impl PartialEq for Value {
    fn eq(&self, rhs: &Self) -> bool {
        self.id == rhs.id
    }
}
impl Eq for Value {}
impl Hash for Value {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.hashes.set(self.hashes.get() + 1);
        assert!(!(self.bomb.get() && self.id == 200), "hash failure");
        if self.collision {
            0.hash(h)
        } else {
            self.id.hash(h)
        }
    }
}
#[test]
fn collisions_payloads_hash_panics_and_clone_repair_metadata_are_preserved() {
    let hashes = Rc::new(Cell::new(0));
    let bomb = Rc::new(Cell::new(false));
    let value = |id, payload| Value {
        id,
        payload,
        collision: false,
        hashes: hashes.clone(),
        bomb: bomb.clone(),
    };
    let colliding = |id| Value {
        collision: true,
        ..value(id, "collision")
    };
    let collisions = FiniteProblem::new_indexed(vec![
        Variable::new((0..128).map(colliding).collect()).with_initial(colliding(127)),
    ])
    .unwrap();
    hashes.set(0);
    for id in 0..128 {
        collisions
            .validate(&Assignment::new(vec![Some(colliding(id))]))
            .unwrap();
    }
    assert_eq!(
        hashes.get(),
        0,
        "crowded hash chains fall back to direct lookup"
    );
    let duplicate = vec![Variable::new((0..128).chain([63]).map(colliding).collect())];
    assert_eq!(
        FiniteProblem::new(duplicate.clone()).unwrap_err(),
        FiniteProblem::new_indexed(duplicate).unwrap_err()
    );
    let bad_initial = vec![
        Variable::new((0..128).map(colliding).collect()).with_initial(colliding(999)),
        Variable::new(vec![colliding(0), colliding(0)]),
    ];
    assert_eq!(
        FiniteProblem::new(bad_initial.clone()).unwrap_err(),
        FiniteProblem::new_indexed(bad_initial).unwrap_err()
    );
    let vars = vec![
        Variable::new((0..256).rev().map(|id| value(id, "domain")).collect())
            .with_initial(value(1, "original"))
            .with_range_diagnostics("A", "value", |v| v.id.to_string()),
        Variable::new((0..256).map(|id| value(id, "domain")).collect())
            .with_initial(value(2, "original")),
    ];
    let schema = PillarSchema::new(vec![vec![0], vec![1]]).unwrap();
    let p = FiniteProblem::new_indexed(vars)
        .unwrap()
        .with_pillar_schema(schema.clone())
        .unwrap();
    let before = p.initial_assignment();
    hashes.set(0);
    let clone = p.clone();
    assert_eq!(hashes.get(), 0);
    assert_eq!(clone.pillar_schema(), Some(&schema));
    assert_eq!(clone.variables()[0].domain()[0].id, 255);
    let resumed = p.with_initial_assignment(&before).unwrap();
    let plan = BasicRuinPlan::new(&resumed, &before, &[0]).unwrap();
    let repair = plan.repair_problem();
    assert_eq!(repair.pillar_schema(), Some(&schema));
    assert!(repair.variables()[1].is_pinned());
    hashes.set(0);
    repair.validate(&repair.initial_assignment()).unwrap();
    assert!(hashes.get() > 0, "repair retains indexed lookup");
    let invalid = Assignment::new(vec![
        Some(value(999, "candidate")),
        Some(value(2, "original")),
    ]);
    assert!(p.validate(&invalid).unwrap_err().range_report().is_some());
    let net = ConstraintNetwork::<_, SimpleScore>::new(2);
    let mut d = ScoreDirector::new(&p, &net, before.clone()).unwrap();
    let movement = Move::Change {
        variable: 0,
        value: Some(value(200, "candidate")),
    };
    bomb.set(true);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = d.apply(&movement);
        }))
        .is_err()
    );
    bomb.set(false);
    assert_eq!(
        d.assignment().values()[0].as_ref().unwrap().payload,
        "original"
    );
    d.assert_consistent().unwrap();
    {
        let trial = d.apply(&movement).unwrap();
        assert_eq!(
            trial.assignment().values()[0].as_ref().unwrap().payload,
            "candidate"
        );
    }
    assert_eq!(
        d.assignment().values()[0].as_ref().unwrap().payload,
        "original"
    );
}
#[test]
fn small_indexed_domains_and_eq_only_models_keep_direct_lookup() {
    let hashes = Rc::new(Cell::new(0));
    let bomb = Rc::new(Cell::new(false));
    let value = |id| Value {
        id,
        payload: "small",
        collision: false,
        hashes: hashes.clone(),
        bomb: bomb.clone(),
    };
    let p = FiniteProblem::new_indexed(vec![
        Variable::new((0..8).map(value).collect()).with_initial(value(7)),
    ])
    .unwrap();
    p.validate(&p.initial_assignment()).unwrap();
    assert_eq!(hashes.get(), 0);
    #[derive(Clone, Debug, Eq, PartialEq)]
    struct EqOnly(usize);
    let p = FiniteProblem::new(vec![
        Variable::new(vec![EqOnly(1), EqOnly(2)]).with_initial(EqOnly(1)),
    ])
    .unwrap();
    assert!(
        p.is_move_doable(
            &p.initial_assignment(),
            &Move::Change {
                variable: 0,
                value: Some(EqOnly(2))
            }
        )
        .unwrap()
    );
}
