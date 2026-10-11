//! Original ownership/rollback checks; no upstream method credit.
use std::{
    cell::Cell,
    hash::{Hash, Hasher},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;
#[derive(Debug)]
struct Value {
    id: usize,
    payload: String,
    clones: Rc<Cell<usize>>,
    limit: Rc<Cell<usize>>,
}
impl PartialEq for Value {
    fn eq(&self, rhs: &Self) -> bool {
        self.id == rhs.id
    }
}
impl Eq for Value {}
impl Hash for Value {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.id.hash(h)
    }
}
impl Clone for Value {
    fn clone(&self) -> Self {
        let n = self.clones.get() + 1;
        self.clones.set(n);
        assert!(n <= self.limit.get(), "clone failure");
        Self {
            id: self.id,
            payload: self.payload.clone(),
            clones: self.clones.clone(),
            limit: self.limit.clone(),
        }
    }
}
fn range(entity: usize, start: usize, end: usize) -> ListRange {
    ListRange { entity, start, end }
}
fn swap<V>(left: ListRange, right: ListRange, reverse: bool) -> ListMove<V> {
    ListMove::SubListSwap {
        left,
        right,
        reverse,
    }
}
fn movement<V>() -> ListMove<V> {
    ListMove::Composite(
        (0..16)
            .map(|_| {
                ListMove::Composite(vec![
                    swap(range(0, 1, 4), range(0, 6, 10), true),
                    swap(range(0, 2, 6), range(1, 3, 7), true),
                    swap(range(0, 1, 5), range(1, 2, 4), false),
                    swap(range(0, 1, 3), range(1, 2, 6), false),
                    ListMove::SubListChange {
                        entity: 0,
                        start: 1,
                        end: 4,
                        destination: ListPosition {
                            entity: 0,
                            index: 8,
                        },
                        reverse: true,
                    },
                ])
            })
            .collect(),
    )
}
fn ids(a: &ListAssignment<Value>) -> Vec<Vec<usize>> {
    a.lists()
        .iter()
        .map(|r| r.iter().map(|v| v.id).collect())
        .collect()
}
// Allocation-heavy reference using the original pre-edit spans, independently
// replacing their positions. No production rotation or swap helper is used.
fn reference(rows: &mut [Vec<usize>], movement: &ListMove<usize>, recorded: &mut Vec<usize>) {
    let mut record = |v| {
        if !recorded.contains(&v) {
            recorded.push(v)
        }
    };
    match movement {
        ListMove::Composite(steps) => {
            for m in steps {
                reference(rows, m, recorded)
            }
        }
        ListMove::SubListSwap {
            left,
            right,
            reverse,
        } => {
            let mut a = rows[left.entity][left.start..left.end].to_vec();
            let mut b = rows[right.entity][right.start..right.end].to_vec();
            for &v in a.iter().chain(&b) {
                record(v)
            }
            if *reverse {
                a.reverse();
                b.reverse()
            }
            for (e, row) in rows.iter_mut().enumerate() {
                let mut out = vec![];
                for (i, &v) in row.iter().enumerate() {
                    if e == left.entity && i == left.start {
                        out.extend(&b)
                    }
                    if e == right.entity && i == right.start {
                        out.extend(&a)
                    }
                    if !(e == left.entity && (left.start..left.end).contains(&i)
                        || e == right.entity && (right.start..right.end).contains(&i))
                    {
                        out.push(v)
                    }
                }
                *row = out;
            }
        }
        ListMove::SubListChange {
            entity,
            start,
            end,
            destination,
            reverse,
        } => {
            let mut values = rows[*entity][*start..*end].to_vec();
            for &v in &values {
                record(v)
            }
            if *reverse {
                values.reverse()
            }
            rows[*entity].drain(*start..*end);
            rows[destination.entity].splice(destination.index..destination.index, values);
        }
        _ => panic!("unexpected reference leaf"),
    }
}
#[test]
fn composite_spans_move_payloads_and_clone_only_undo_rows_in_list_and_mixed_trials() {
    let clones = Rc::new(Cell::new(0));
    let limit = Rc::new(Cell::new(usize::MAX));
    let value = |id, source| Value {
        id,
        payload: format!("{source}:{id}"),
        clones: clones.clone(),
        limit: limit.clone(),
    };
    let p = ListProblem::new_indexed(
        (0..28).map(|id| value(id, "registry")).collect(),
        vec![
            ListEntity::new((0..12).map(|id| value(id, "assigned")).collect())
                .with_pinned_prefix(1),
            ListEntity::new((12..24).map(|id| value(id, "assigned")).collect()),
            ListEntity::new((24..28).map(|id| value(id, "assigned")).collect()).with_pinned(true),
        ],
    )
    .unwrap();
    let before = p.initial_assignment();
    let mut expected = ids(&before);
    let mut recorded = vec![];
    reference(&mut expected, &movement(), &mut recorded);
    let m = movement();
    assert_eq!(
        p.planning_values(&before, &m)
            .unwrap()
            .iter()
            .map(|v| v.id)
            .collect::<Vec<_>>(),
        recorded
    );
    let mut net = ListConstraintNetwork::<Value, SimpleScore>::new(3, 28);
    let panic_score = Rc::new(Cell::new(false));
    let flag = panic_score.clone();
    net.add_constraint("order", vec![ListScope::Entity(0)], move |f| {
        assert!(!flag.get(), "score failure");
        let ListFact::Entity(row) = &f[0] else {
            panic!()
        };
        SimpleScore::new(row.iter().enumerate().map(|(i, v)| (i * v.id) as i64).sum())
    })
    .unwrap();
    let mut d = ListScoreDirector::new(&p, &net, before.clone()).unwrap();
    clones.set(0);
    limit.set(24);
    {
        let trial = d.apply(&m).unwrap();
        assert_eq!(ids(trial.assignment()), expected);
        assert_eq!(clones.get(), 24);
        for row in trial.assignment().lists() {
            for v in row {
                assert_eq!(v.payload, format!("assigned:{}", v.id));
            }
        }
    }
    assert_eq!(ids(d.assignment()), ids(&before));
    for clone_limit in [3, 24] {
        clones.set(0);
        limit.set(clone_limit);
        panic_score.set(clone_limit == 24);
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                let _ = d.apply(&m);
            }))
            .is_err()
        );
        panic_score.set(false);
        limit.set(usize::MAX);
        assert_eq!(ids(d.assignment()), ids(&before));
        d.assert_consistent().unwrap();
    }
    let p = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap(),
        p,
    );
    let before = p.initial_assignment();
    let net = MixedConstraintNetwork::<_, _, SimpleScore>::new(1, 3, 28);
    let mut d = MixedScoreDirector::new(&p, &net, before.clone()).unwrap();
    clones.set(0);
    limit.set(24);
    {
        let trial = d
            .apply(&MixedMove {
                basic: Move::Change {
                    variable: 0,
                    value: Some(1),
                },
                lists: movement(),
            })
            .unwrap();
        assert_eq!(ids(trial.assignment().lists()), expected);
        assert_eq!(clones.get(), 24);
    }
    limit.set(usize::MAX);
    assert_eq!(d.assignment(), &before);
    d.assert_consistent().unwrap();
}
#[test]
fn global_dependency_failure_after_in_place_span_edits_restores_rows() {
    let p = ListProblem::new_indexed((0..8).collect(), vec![ListEntity::new((0..8).collect())])
        .unwrap()
        .with_dependencies(vec![
            vec![],
            vec![0],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
        ])
        .unwrap();
    let before = p.initial_assignment();
    let mut a = before.clone();
    let m = ListMove::Composite(vec![
        swap(range(0, 4, 6), range(0, 6, 8), true),
        ListMove::SubListChange {
            entity: 0,
            start: 0,
            end: 2,
            destination: ListPosition {
                entity: 0,
                index: 0,
            },
            reverse: true,
        },
    ]);
    assert!(matches!(
        p.apply(&mut a, &m),
        Err(ListError::DependencyCycle { .. })
    ));
    assert_eq!(a, before);
}
