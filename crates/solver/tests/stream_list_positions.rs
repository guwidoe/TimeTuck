//! Original regressions for incremental list metadata; no upstream method credit.
use timetuck::*;

#[derive(Clone, Debug)]
struct Value {
    id: usize,
    payload: i64,
}
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for Value {}
fn value(id: usize) -> Value {
    Value {
        id,
        payload: 100 + id as i64,
    }
}
fn position(entity: usize, index: usize) -> ListPosition {
    ListPosition { entity, index }
}
fn problem() -> ListProblem<Value> {
    ListProblem::new(
        (0..9)
            .map(|id| Value {
                id,
                payload: 1000 + id as i64,
            })
            .collect(),
        vec![
            ListEntity::new((0..4).map(value).collect()).with_pinned_prefix(1),
            ListEntity::new((4..7).map(value).collect()),
            ListEntity::new(vec![]),
            ListEntity::new(vec![value(7)]).with_pinned(true),
        ],
    )
    .unwrap()
    .allowing_unassigned()
}
fn movements() -> Vec<ListMove<Value>> {
    vec![
        ListMove::Unassign {
            entity: 0,
            start: 1,
            end: 3,
        },
        ListMove::Assign {
            value: value(1),
            destination: position(2, 0),
        },
        ListMove::SubListSwap {
            left: ListRange {
                entity: 0,
                start: 1,
                end: 2,
            },
            right: ListRange {
                entity: 1,
                start: 0,
                end: 2,
            },
            reverse: true,
        },
        ListMove::Composite(vec![
            ListMove::Unassign {
                entity: 0,
                start: 1,
                end: 2,
            },
            ListMove::Assign {
                value: Value {
                    id: 5,
                    payload: 900,
                },
                destination: position(2, 1),
            },
            ListMove::Change {
                source: position(1, 1),
                destination: position(0, 1),
            },
        ]),
        ListMove::Assign {
            value: value(2),
            destination: position(1, 1),
        },
        ListMove::SubListChange {
            entity: 0,
            start: 1,
            end: 3,
            destination: position(0, 1),
            reverse: true,
        },
        ListMove::Composite(vec![]),
    ]
}
fn contribution(row: &ListValueRow<Value>) -> SimpleScore {
    SimpleScore::new(
        row.value.payload
            + row
                .position
                .map_or(-10, |p| (p.entity * 10 + p.index) as i64)
            + row.previous.as_ref().map_or(0, |v| v.payload * 3)
            - row.next.as_ref().map_or(0, |v| v.payload * 7),
    )
}
fn check(
    graph: &StreamGraph<'_, SimpleScore>,
    source: &ListStreamSources<Value>,
    problem: &ListProblem<Value>,
    assignment: &ListAssignment<Value>,
    observed: bool,
    score: SimpleScore,
) {
    let rows = graph.source_rows(&source.values).unwrap();
    assert_eq!(rows.len(), 9);
    let mut expected_score = SimpleScore::ZERO;
    for (i, registry) in problem.values().iter().enumerate() {
        let mut expected = ListValueRow {
            range_index: i,
            value: registry.clone(),
            position: None,
            previous: None,
            next: None,
        };
        for (entity, values) in assignment.lists().iter().enumerate() {
            for (index, v) in values.iter().enumerate() {
                if v.id == registry.id {
                    expected.position = Some(position(entity, index));
                    expected.previous = index.checked_sub(1).map(|j| values[j].clone());
                    expected.next = values.get(index + 1).cloned();
                }
            }
        }
        let actual = &rows[&i];
        assert_eq!(actual, &expected);
        assert_eq!(actual.value.payload, expected.value.payload);
        assert_eq!(
            actual.previous.as_ref().map(|v| v.payload),
            expected.previous.as_ref().map(|v| v.payload)
        );
        assert_eq!(
            actual.next.as_ref().map(|v| v.payload),
            expected.next.as_ref().map(|v| v.payload)
        );
        expected_score = expected_score + contribution(&expected);
    }
    assert_eq!(
        score,
        if observed {
            expected_score
        } else {
            SimpleScore::ZERO
        }
    );
    let entities = graph.source_rows(&source.entities).unwrap();
    for (i, list) in assignment.lists().iter().enumerate() {
        let payloads = |vs: &[Value]| vs.iter().map(|v| (v.id, v.payload)).collect::<Vec<_>>();
        assert_eq!(payloads(&entities[&i].1), payloads(list));
    }
}
#[test]
fn list_positions_cover_removed_reassigned_and_unobserved_rows() {
    for observed in [false, true] {
        let p = problem();
        let mut builder = StreamGraphBuilder::new();
        let s = ListStreamSources::new(&mut builder);
        if observed {
            builder
                .score("metadata", s.values.port(), contribution)
                .unwrap();
        }
        let mut d =
            ListStreamScoreDirector::new(&p, builder.build(), s.clone(), p.initial_assignment())
                .unwrap();
        for m in movements() {
            let before = d.assignment().clone();
            let old = d.score();
            let tx = d.apply(&m).unwrap();
            check(tx.graph(), &s, &p, tx.assignment(), observed, tx.score());
            drop(tx);
            assert_eq!(d.assignment(), &before);
            check(d.graph(), &s, &p, &before, observed, old);
            d.apply(&m).unwrap().commit();
            check(d.graph(), &s, &p, d.assignment(), observed, d.score());
            d.assert_consistent().unwrap();
        }
    }
}
#[test]
fn mixed_positions_cover_removed_reassigned_and_unobserved_rows() {
    for observed in [false, true] {
        let p = MixedProblem::new(
            FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap(),
            problem(),
        );
        let mut builder = StreamGraphBuilder::new();
        let s = MixedStreamSources::new(&mut builder);
        if observed {
            builder
                .score("metadata", s.lists.values.port(), contribution)
                .unwrap();
        }
        let mut d =
            MixedStreamScoreDirector::new(&p, builder.build(), s.clone(), p.initial_assignment())
                .unwrap();
        for (i, lists) in movements().into_iter().enumerate() {
            let m = MixedMove {
                basic: Move::Change {
                    variable: 0,
                    value: Some(i % 2),
                },
                lists,
            };
            let before = d.assignment().clone();
            let old = d.score();
            let tx = d.apply(&m).unwrap();
            check(
                tx.graph(),
                &s.lists,
                p.lists(),
                tx.assignment().lists(),
                observed,
                tx.score(),
            );
            drop(tx);
            assert_eq!(d.assignment(), &before);
            check(
                d.graph(),
                &s.lists,
                p.lists(),
                before.lists(),
                observed,
                old,
            );
            d.apply(&m).unwrap().commit();
            check(
                d.graph(),
                &s.lists,
                p.lists(),
                d.assignment().lists(),
                observed,
                d.score(),
            );
            d.assert_consistent().unwrap();
        }
    }
}
