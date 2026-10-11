use ListPathEndpoint::{End, Start, Value};
use std::collections::BTreeSet;
use timetuck::*;

fn at(entity: usize, index: usize) -> ListPathEndpoint {
    Value(ListPosition { entity, index })
}
fn permutations(values: &mut [usize], start: usize, out: &mut Vec<Vec<usize>>) {
    if start == values.len() {
        out.push(values.to_vec());
        return;
    }
    for i in start..values.len() {
        values.swap(start, i);
        permutations(values, start + 1, out);
        values.swap(start, i);
    }
}
// Independent oracle: target rows explicitly define edges between original
// value IDs and owner boundary IDs. The production builder never sees these rows.
fn edges(rows: &[Vec<usize>], n: usize) -> BTreeSet<(usize, usize)> {
    let mut out = BTreeSet::new();
    for (owner, row) in rows.iter().enumerate() {
        let nodes = std::iter::once(n + 2 * owner)
            .chain(row.iter().copied())
            .chain(std::iter::once(n + 2 * owner + 1))
            .collect::<Vec<_>>();
        for edge in nodes.windows(2) {
            out.insert((edge[0].min(edge[1]), edge[0].max(edge[1])));
        }
    }
    out
}
#[test]
fn exhaustive_small_row_permutations_match_edge_reconstruction_and_undo() {
    let mut checked = 0;
    for n in 0..=4 {
        let original = vec![(0..n).collect::<Vec<_>>(), vec![], vec![]];
        let p = ListProblem::new(
            (0..n).collect(),
            original.iter().cloned().map(ListEntity::new).collect(),
        )
        .unwrap();
        let opening = p.initial_assignment();
        let old_edges = edges(&original, n);
        let mut orders = vec![];
        permutations(&mut (0..n).collect::<Vec<_>>(), 0, &mut orders);
        for order in orders {
            for first in 0..=n {
                for second in first..=n {
                    let expected = vec![
                        order[..first].to_vec(),
                        order[first..second].to_vec(),
                        order[second..].to_vec(),
                    ];
                    let new_edges = edges(&expected, n);
                    let endpoint = |id| {
                        if id < n {
                            at(0, id)
                        } else if (id - n) % 2 == 0 {
                            Start((id - n) / 2)
                        } else {
                            End((id - n) / 2)
                        }
                    };
                    let cuts = old_edges
                        .difference(&new_edges)
                        .map(|&(a, b)| (endpoint(a), endpoint(b)))
                        .collect::<Vec<_>>();
                    let joins = new_edges
                        .difference(&old_edges)
                        .map(|&(a, b)| (endpoint(a), endpoint(b)))
                        .collect::<Vec<_>>();
                    // Non-model order must not change boundary ownership.
                    let movement = p
                        .reconnect_list_paths(&opening, &[2, 0, 1], &cuts, &joins)
                        .unwrap();
                    let mut a = opening.clone();
                    {
                        let trial = p.apply(&mut a, &movement).unwrap();
                        assert_eq!(trial.lists(), expected);
                        assert!(p.is_complete(&trial));
                    }
                    assert_eq!(a, opening);
                    checked += 1;
                }
            }
        }
    }
    assert_eq!(checked, 436);
}

#[test]
fn tail_exchange_reversal_empty_destination_and_pinned_prefix() {
    let p = ListProblem::new(
        (0..8).collect(),
        vec![
            ListEntity::new(vec![0, 1, 2, 3]).with_pinned_prefix(1),
            ListEntity::new(vec![4, 5]),
            ListEntity::new(vec![]),
            ListEntity::new(vec![6]).with_pinned(true),
        ],
    )
    .unwrap();
    let original = p.initial_assignment();
    // Exchange tails, preserving prefix 0 and pinned/unselected row 6.
    let movement = p
        .reconnect_list_paths(
            &original,
            &[1, 0],
            &[(at(0, 1), at(0, 2)), (at(1, 0), at(1, 1))],
            &[(at(0, 1), at(1, 1)), (at(1, 0), at(0, 2))],
        )
        .unwrap();
    let mut a = original.clone();
    a_apply(
        &p,
        &mut a,
        &movement,
        &[vec![0, 1, 5], vec![4, 2, 3], vec![], vec![6]],
    );
    assert_eq!(a, original);
    // Reverse a movable segment using boundaries to fix its orientation.
    let reversal = p
        .reconnect_list_paths(
            &original,
            &[0],
            &[(Start(0), at(0, 1)), (at(0, 3), End(0))],
            &[(Start(0), at(0, 3)), (at(0, 1), End(0))],
        )
        .unwrap();
    a_apply(
        &p,
        &mut a,
        &reversal,
        &[vec![0, 3, 2, 1], vec![4, 5], vec![], vec![6]],
    );
    // Empty destinations have their own direct boundary edge; remove both
    // connections around the entire suffix and reconnect it under owner 2.
    let transfer = p
        .reconnect_list_paths(
            &original,
            &[0, 2],
            &[(Start(0), at(0, 1)), (at(0, 3), End(0)), (Start(2), End(2))],
            &[(Start(0), End(0)), (Start(2), at(0, 1)), (at(0, 3), End(2))],
        )
        .unwrap();
    a_apply(
        &p,
        &mut a,
        &transfer,
        &[vec![0], vec![4, 5], vec![1, 2, 3], vec![6]],
    );
    assert_eq!(a, original);
    assert_eq!(a.location(&7), None);
}
fn a_apply(
    p: &ListProblem<usize>,
    a: &mut ListAssignment<usize>,
    movement: &ListMove<usize>,
    expected: &[Vec<usize>],
) {
    let trial = p.apply(a, movement).unwrap();
    assert_eq!(trial.lists(), expected);
}

#[test]
fn malformed_graphs_reject_without_mutation() {
    let p = ListProblem::new(
        (0..4).collect(),
        vec![ListEntity::new(vec![0, 1, 2]), ListEntity::new(vec![3])],
    )
    .unwrap();
    let a = p.initial_assignment();
    let cases = [
        (vec![(Start(0), at(0, 0))], vec![], ListKOptError::EdgeCount),
        (
            vec![(Start(0), Start(0))],
            vec![(Start(0), at(0, 0))],
            ListKOptError::InvalidEdge,
        ),
        (
            vec![(Start(0), at(0, 1))],
            vec![(Start(0), at(0, 0))],
            ListKOptError::InvalidEdge,
        ),
        (
            vec![(Start(0), at(0, 0)); 2],
            vec![(Start(0), at(0, 0)); 2],
            ListKOptError::DuplicateEdge,
        ),
        (
            vec![(Start(0), at(0, 0))],
            vec![(at(0, 0), at(0, 1))],
            ListKOptError::DuplicateEdge,
        ),
        (
            vec![(Start(0), at(0, 0))],
            vec![(Start(0), End(0))],
            ListKOptError::Degree,
        ),
        (
            vec![(Start(0), at(0, 0)), (Start(1), at(1, 0))],
            vec![(Start(0), Start(1)), (at(0, 0), at(1, 0))],
            ListKOptError::Partition,
        ),
        (
            vec![(Start(0), at(0, 0)), (at(0, 2), End(0))],
            vec![(Start(0), End(0)), (at(0, 0), at(0, 2))],
            ListKOptError::Disconnected,
        ),
    ];
    for (cuts, joins, expected) in cases {
        assert_eq!(
            p.reconnect_list_paths(&a, &[0, 1], &cuts, &joins),
            Err(expected)
        );
        assert_eq!(a, p.initial_assignment());
    }
    for entities in [vec![], vec![0, 0], vec![2]] {
        assert!(p.reconnect_list_paths(&a, &entities, &[], &[]).is_err());
    }
    assert_eq!(
        p.reconnect_list_paths(&a, &[0], &[(Start(1), at(0, 0))], &[(Start(0), at(0, 0))]),
        Err(ListKOptError::Entities)
    );
    assert_eq!(
        p.reconnect_list_paths(&a, &[0], &[(Start(0), at(0, 9))], &[(Start(0), at(0, 0))]),
        Err(ListKOptError::List(ListError::ElementIndex(ListPosition {
            entity: 0,
            index: 9
        })))
    );
    let duplicate = ListAssignment::new(vec![vec![0, 0, 2], vec![3]]);
    assert_eq!(
        p.reconnect_list_paths(&duplicate, &[0, 1], &[], &[]),
        Err(ListKOptError::List(ListError::DuplicateAssignedValue))
    );
}

#[test]
fn pins_receiver_domains_and_final_dependencies_remain_checked() {
    let p = ListProblem::new(
        vec![0, 1, 2, 3],
        vec![
            ListEntity::new(vec![0, 1]).with_value_range(vec![0, 1]),
            ListEntity::new(vec![2, 3]).with_value_range(vec![2, 3]),
        ],
    )
    .unwrap();
    let a = p.initial_assignment();
    let cuts = [(at(0, 0), at(0, 1)), (at(1, 0), at(1, 1))];
    let joins = [(at(0, 0), at(1, 1)), (at(1, 0), at(0, 1))];
    assert_eq!(
        p.reconnect_list_paths(&a, &[0, 1], &cuts, &joins),
        Err(ListKOptError::List(ListError::ValueOutsideEntityRange {
            entity: 0
        }))
    );
    let p = ListProblem::new(
        vec![0, 1, 2],
        vec![ListEntity::new(vec![0, 1, 2]).with_pinned_prefix(1)],
    )
    .unwrap();
    assert_eq!(
        p.reconnect_list_paths(
            &p.initial_assignment(),
            &[0],
            &[(at(0, 0), at(0, 1))],
            &[(at(0, 0), at(0, 1))]
        ),
        Err(ListKOptError::List(ListError::PinChanged { entity: 0 }))
    );
    let p = ListProblem::new(vec![0], vec![ListEntity::new(vec![0]).with_pinned(true)]).unwrap();
    assert_eq!(
        p.reconnect_list_paths(&p.initial_assignment(), &[0], &[], &[]),
        Err(ListKOptError::List(ListError::PinChanged { entity: 0 }))
    );
    let p = ListProblem::new(vec![0, 1], vec![ListEntity::new(vec![0, 1])])
        .unwrap()
        .with_dependencies(vec![vec![], vec![0]])
        .unwrap();
    assert!(matches!(
        p.reconnect_list_paths(
            &p.initial_assignment(),
            &[0],
            &[(Start(0), at(0, 0)), (at(0, 1), End(0))],
            &[(Start(0), at(0, 1)), (at(0, 0), End(0))]
        ),
        Err(ListKOptError::List(ListError::DependencyCycle { .. }))
    ));
}

#[test]
fn both_scoring_backends_apply_undo_commit_and_rollback_callback_panics() {
    use std::{
        cell::Cell,
        panic::{AssertUnwindSafe, catch_unwind},
    };
    let p = ListProblem::new(
        (0..4).collect(),
        vec![ListEntity::new(vec![0, 1]), ListEntity::new(vec![2, 3])],
    )
    .unwrap();
    let original = p.initial_assignment();
    let movement = p
        .reconnect_list_paths(
            &original,
            &[0, 1],
            &[(at(0, 0), at(0, 1)), (at(1, 0), at(1, 1))],
            &[(at(0, 0), at(1, 1)), (at(1, 0), at(0, 1))],
        )
        .unwrap();
    let fail = Cell::new(false);
    let score = |owner: usize, values: &[usize]| {
        assert!(
            !fail.get() || owner != 0 || !values.contains(&3),
            "candidate score failure"
        );
        SimpleScore::new(((owner + 1) * values.iter().sum::<usize>()) as i64)
    };
    let mut network = ListConstraintNetwork::new(2, 4);
    for owner in 0..2 {
        let score = &score;
        network
            .add_constraint(
                format!("owner{owner}"),
                vec![ListScope::Entity(owner)],
                move |facts| {
                    let ListFact::Entity(row) = facts[0] else {
                        unreachable!()
                    };
                    score(owner, row)
                },
            )
            .unwrap();
    }
    let mut scoped = ListScoreDirector::new(&p, &network, original.clone()).unwrap();
    assert_eq!(scoped.score(), SimpleScore::new(11));
    {
        let trial = scoped.apply(&movement).unwrap();
        assert_eq!(trial.assignment().lists(), &[vec![0, 3], vec![2, 1]]);
        assert_eq!(trial.score(), SimpleScore::new(9));
    }
    assert_eq!(scoped.assignment(), &original);
    assert_eq!(scoped.score(), SimpleScore::new(11));
    fail.set(true);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _trial = scoped.apply(&movement);
        }))
        .is_err()
    );
    fail.set(false);
    assert_eq!(scoped.assignment(), &original);
    scoped.assert_consistent().unwrap();
    scoped.apply(&movement).unwrap().commit();
    assert_eq!(scoped.score(), SimpleScore::new(9));
    scoped.assert_consistent().unwrap();

    let mut builder = StreamGraphBuilder::new();
    let sources = ListStreamSources::new(&mut builder);
    builder
        .score("owners", sources.entities.port(), |(owner, row)| {
            score(*owner, row)
        })
        .unwrap();
    let mut stream =
        ListStreamScoreDirector::new(&p, builder.build(), sources, original.clone()).unwrap();
    assert_eq!(stream.score(), SimpleScore::new(11));
    {
        let trial = stream.apply(&movement).unwrap();
        assert_eq!(trial.assignment().lists(), &[vec![0, 3], vec![2, 1]]);
        assert_eq!(trial.score(), SimpleScore::new(9));
    }
    assert_eq!(stream.assignment(), &original);
    assert_eq!(stream.score(), SimpleScore::new(11));
    fail.set(true);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _trial = stream.apply(&movement);
        }))
        .is_err()
    );
    fail.set(false);
    assert_eq!(stream.assignment(), &original);
    stream.assert_consistent().unwrap();
    stream.apply(&movement).unwrap().commit();
    assert_eq!(stream.score(), SimpleScore::new(9));
    stream.assert_consistent().unwrap();
}

#[test]
fn building_reconnections_borrows_assigned_payloads_without_cloning() {
    use std::{cell::Cell, rc::Rc};
    #[derive(Debug)]
    struct Item {
        id: usize,
        payload: &'static str,
        clones: Rc<Cell<usize>>,
    }
    impl Clone for Item {
        fn clone(&self) -> Self {
            self.clones.set(self.clones.get() + 1);
            Self {
                id: self.id,
                payload: self.payload,
                clones: self.clones.clone(),
            }
        }
    }
    impl PartialEq for Item {
        fn eq(&self, other: &Self) -> bool {
            self.id == other.id
        }
    }
    impl Eq for Item {}
    let clones = Rc::new(Cell::new(0));
    let item = |id, payload| Item {
        id,
        payload,
        clones: clones.clone(),
    };
    let p = ListProblem::new(
        (0..4).map(|i| item(i, "registry")).collect(),
        vec![
            ListEntity::new(vec![item(0, "assigned0"), item(1, "assigned1")]),
            ListEntity::new(vec![item(2, "assigned2"), item(3, "assigned3")]),
        ],
    )
    .unwrap();
    let mut a = p.initial_assignment();
    clones.set(0);
    let movement = p
        .reconnect_list_paths(
            &a,
            &[0, 1],
            &[(at(0, 0), at(0, 1)), (at(1, 0), at(1, 1))],
            &[(at(0, 0), at(1, 1)), (at(1, 0), at(0, 1))],
        )
        .unwrap();
    assert_eq!(clones.get(), 0);
    {
        let trial = p.apply(&mut a, &movement).unwrap();
        assert_eq!(
            trial.lists()[0]
                .iter()
                .map(|v| v.payload)
                .collect::<Vec<_>>(),
            ["assigned0", "assigned3"]
        );
        assert_eq!(
            trial.lists()[1]
                .iter()
                .map(|v| v.payload)
                .collect::<Vec<_>>(),
            ["assigned2", "assigned1"]
        );
    }
    assert_eq!(a.lists()[0][1].payload, "assigned1");
}

#[test]
fn larger_reconnections_cross_the_shared_relocation_threshold() {
    for n in [127, 128, 129, 260] {
        let half = n / 2;
        let p = ListProblem::new(
            (0..n).collect(),
            vec![
                ListEntity::new((0..half).collect()),
                ListEntity::new((half..n).collect()),
            ],
        )
        .unwrap();
        let original = p.initial_assignment();
        // Exchange complete suffixes, intentionally permuting cut/join order and
        // reversing endpoint directions. Owners follow starts, not traversal rank.
        let cuts = [(at(1, 0), Start(1)), (Start(0), at(0, 0))];
        let joins = [(at(0, 0), Start(1)), (at(1, 0), Start(0))];
        let movement = p
            .reconnect_list_paths(&original, &[1, 0], &cuts, &joins)
            .unwrap();
        let mut a = original.clone();
        a_apply(
            &p,
            &mut a,
            &movement,
            &[(half..n).collect(), (0..half).collect()],
        );
        assert_eq!(a, original);
    }
}
