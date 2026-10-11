//! Original compact-change undo regression tests; no upstream source credit.
use std::{
    cell::Cell,
    hash::{Hash, Hasher},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;
#[derive(Default, Debug)]
struct Counts {
    clones: Cell<usize>,
    panic_clone: Cell<bool>,
    panic_hash: Cell<bool>,
    panic_eq: Cell<bool>,
}
#[derive(Debug)]
struct Value {
    id: usize,
    payload: usize,
    counts: Rc<Counts>,
}
impl Clone for Value {
    fn clone(&self) -> Self {
        assert!(!self.counts.panic_clone.get(), "clone");
        self.counts.clones.set(self.counts.clones.get() + 1);
        Self {
            id: self.id,
            payload: self.payload,
            counts: self.counts.clone(),
        }
    }
}
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        assert!(!self.counts.panic_eq.get(), "eq");
        self.id == other.id
    }
}
impl Eq for Value {}
impl Hash for Value {
    fn hash<H: Hasher>(&self, h: &mut H) {
        assert!(!self.counts.panic_hash.get(), "hash");
        self.id.hash(h);
    }
}
fn fixture() -> (ListProblem<Value>, Rc<Counts>) {
    let c = Rc::new(Counts::default());
    let make = |id, payload| Value {
        id,
        payload,
        counts: c.clone(),
    };
    let p = ListProblem::new_indexed(
        (0..129).map(|i| make(i, i)).collect(),
        vec![
            ListEntity::new((0..64).map(|i| make(i, 1000 + i)).collect()),
            ListEntity::new((64..128).map(|i| make(i, 1000 + i)).collect()),
            ListEntity::new(vec![make(128, 1128)]),
        ],
    )
    .unwrap()
    .allowing_unassigned();
    (p, c)
}
fn change(a: usize, i: usize, b: usize, j: usize) -> ListMove<Value> {
    ListMove::Change {
        source: ListPosition {
            entity: a,
            index: i,
        },
        destination: ListPosition {
            entity: b,
            index: j,
        },
    }
}
fn payloads(a: &ListAssignment<Value>) -> Vec<Vec<usize>> {
    a.lists()
        .iter()
        .map(|r| r.iter().map(|v| v.payload).collect())
        .collect()
}
#[test]
fn large_change_is_clone_free_until_original_rows_are_requested() {
    let (p, c) = fixture();
    for m in [
        change(0, 0, 1, 63),
        change(1, 63, 0, 0),
        change(0, 1, 0, 63),
        change(0, 2, 0, 2),
    ] {
        let mut a = p.initial_assignment();
        let expected = payloads(&a);
        c.clones.set(0);
        {
            let trial = p.apply(&mut a, &m).unwrap();
            assert_eq!(c.clones.get(), 0);
            assert_eq!(trial.original_list(2).unwrap()[0].payload, 1128);
            assert!(trial.original_list(3).is_none());
            assert_eq!(c.clones.get(), 0);
            assert_eq!(
                trial
                    .original_list(0)
                    .unwrap()
                    .iter()
                    .map(|v| v.payload)
                    .collect::<Vec<_>>(),
                expected[0]
            );
            let count = c.clones.get();
            let ptr = trial.original_list(0).unwrap().as_ptr();
            assert_eq!(trial.original_list(0).unwrap().as_ptr(), ptr);
            assert_eq!(c.clones.get(), count);
            assert_eq!(payloads(trial.original()), expected);
        }
        assert_eq!(payloads(&a), expected);
        c.clones.set(0);
        p.apply(&mut a, &m).unwrap().commit();
        assert_eq!(c.clones.get(), 0);
    }
}
#[test]
fn lazy_clone_failure_and_joint_rollback_restore_actual_payloads() {
    let (p, c) = fixture();
    let m = change(0, 0, 1, 63);
    let mut a = p.initial_assignment();
    let expected = payloads(&a);
    c.panic_clone.set(true);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let trial = p.apply(&mut a, &m).unwrap();
            trial.original_list(0);
        }))
        .is_err()
    );
    c.panic_clone.set(false);
    assert_eq!(payloads(&a), expected);
    let p = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![1, 2]).with_initial(1)]).unwrap(),
        p,
    );
    let mut a = p.initial_assignment();
    let m = MixedMove {
        basic: Move::Change {
            variable: 0,
            value: Some(2),
        },
        lists: m,
    };
    c.panic_clone.set(true);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let trial = p.apply(&mut a, &m).unwrap();
            assert_eq!(trial.basic().values(), &[Some(2)]);
            trial.original_list(1);
        }))
        .is_err()
    );
    c.panic_clone.set(false);
    assert_eq!(a.basic().values(), &[Some(1)]);
    assert_eq!(payloads(a.lists()), expected);
    c.clones.set(0);
    p.apply(&mut a, &m).unwrap().commit();
    assert_eq!(c.clones.get(), 0);
    assert_eq!(a.basic().values(), &[Some(2)]);
}
#[test]
fn primitive_and_snapshot_fallback_match_pin_range_dependency_and_index_errors() {
    for mode in 0..4 {
        let mut rows = vec![
            ListEntity::new((0..64).collect()),
            ListEntity::new((64..128).collect()),
        ];
        if mode == 0 {
            rows[0] = rows[0].clone().with_pinned_prefix(2);
        }
        if mode == 1 {
            rows[0] = rows[0].clone().with_value_range((0..64).collect());
        }
        let mut p = ListProblem::new_indexed((0..128).collect(), rows).unwrap();
        if mode == 2 {
            let mut deps = vec![vec![]; 128];
            deps[1] = vec![0];
            p = p.with_dependencies(deps).unwrap();
        }
        for (a, i, b, j) in [
            (0, 0, 1, 63),
            (1, 63, 0, 0),
            (0, 0, 0, 1),
            (0, 2, 1, 3),
            (0, 63, 1, 64),
            (0, 0, 0, 63),
            (0, 63, 0, 0),
            (1, 0, 0, 64),
            (0, 64, 1, 1),
            (2, 0, 0, 0),
        ] {
            let m = ListMove::Change {
                source: ListPosition {
                    entity: a,
                    index: i,
                },
                destination: ListPosition {
                    entity: b,
                    index: j,
                },
            };
            let mut compact = p.initial_assignment();
            let mut saved = compact.clone();
            let before = compact.clone();
            let result = match p.apply(&mut compact, &m) {
                Ok(trial) => Ok(trial.lists().to_vec()),
                Err(e) => Err(e),
            };
            let reference = match p.apply(&mut saved, &ListMove::Composite(vec![m])) {
                Ok(trial) => Ok(trial.lists().to_vec()),
                Err(e) => Err(e),
            };
            assert_eq!(result, reference);
            assert_eq!(compact, before);
            assert_eq!(saved, before);
        }
    }
}
#[test]
fn scoring_failures_unwind_compact_undo_and_value_scopes_materialize_originals() {
    let (p, c) = fixture();
    let p = p.with_dependencies(vec![vec![]; 129]).unwrap();
    let initial = p.initial_assignment();
    let expected = payloads(&initial);
    let panic_score = Cell::new(false);
    let mut net = ListConstraintNetwork::new(3, 129);
    net.add_constraint("entity", vec![ListScope::Entity(0)], |_| {
        assert!(!panic_score.get(), "score");
        SimpleScore::ZERO
    })
    .unwrap();
    let mut director = ListScoreDirector::new(&p, &net, initial).unwrap();
    let m = change(0, 0, 1, 63);
    for flag in [&c.panic_hash, &c.panic_eq, &panic_score] {
        flag.set(true);
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                let _ = director.apply(&m);
            }))
            .is_err()
        );
        flag.set(false);
        assert_eq!(payloads(director.assignment()), expected);
        director.assert_consistent().unwrap();
    }
    let mut net = ListConstraintNetwork::new(3, 129);
    net.add_constraint("value", vec![ListScope::Value(0)], |facts| {
        let ListFact::Value { element, .. } = &facts[0] else {
            unreachable!()
        };
        SimpleScore::new(element.as_ref().map_or(0, |v| v.position.entity as i64))
    })
    .unwrap();
    let mut director = ListScoreDirector::new(&p, &net, p.initial_assignment()).unwrap();
    c.clones.set(0);
    {
        let trial = director.apply(&m).unwrap();
        assert_eq!(trial.score(), SimpleScore::new(1));
        assert_eq!(c.clones.get(), 128);
    }
    assert_eq!(payloads(director.assignment()), expected);
    director.assert_consistent().unwrap();
}

#[test]
fn stream_updates_use_lazy_original_rows_in_list_and_joint_transactions() {
    let p = ListProblem::new_indexed(
        (0..128).collect(),
        vec![
            ListEntity::new((0..64).collect()),
            ListEntity::new((64..128).collect()),
        ],
    )
    .unwrap();
    let m = ListMove::Change {
        source: ListPosition {
            entity: 0,
            index: 0,
        },
        destination: ListPosition {
            entity: 1,
            index: 63,
        },
    };
    let fail = Cell::new(false);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let sources = ListStreamSources::new(&mut b);
    b.score("positions", sources.values.port(), |v| {
        assert!(!fail.get(), "stream score");
        SimpleScore::new(
            v.position
                .map_or(0, |p| (v.value as i64 + 1) * p.entity as i64),
        )
    })
    .unwrap();
    let mut d =
        ListStreamScoreDirector::new(&p, b.build(), sources, p.initial_assignment()).unwrap();
    let before = d.score();
    {
        let trial = d.apply(&m).unwrap();
        assert_ne!(trial.score(), before);
    }
    d.assert_consistent().unwrap();
    assert_eq!(d.assignment(), &p.initial_assignment());
    fail.set(true);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = d.apply(&m);
        }))
        .is_err()
    );
    fail.set(false);
    d.assert_consistent().unwrap();
    assert_eq!(d.assignment(), &p.initial_assignment());
    d.apply(&m).unwrap().commit();
    d.assert_consistent().unwrap();
    let p = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![1, 2]).with_initial(1)]).unwrap(),
        p,
    );
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let sources = MixedStreamSources::new(&mut b);
    b.score("positions", sources.lists.values.port(), |v| {
        SimpleScore::new(
            v.position
                .map_or(0, |p| (v.value as i64 + 1) * p.entity as i64),
        )
    })
    .unwrap();
    let mut d =
        MixedStreamScoreDirector::new(&p, b.build(), sources, p.initial_assignment()).unwrap();
    let m = MixedMove {
        basic: Move::Change {
            variable: 0,
            value: Some(2),
        },
        lists: m,
    };
    {
        let _trial = d.apply(&m).unwrap();
    }
    d.assert_consistent().unwrap();
    assert_eq!(d.assignment(), &p.initial_assignment());
    d.apply(&m).unwrap().commit();
    d.assert_consistent().unwrap();
    assert_eq!(d.assignment().basic().values(), &[Some(2)]);
}
