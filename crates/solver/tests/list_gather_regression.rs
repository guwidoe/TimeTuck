// Original gather reference, validation, scoring and panic-safety checks.
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;
fn pos(entity: usize, index: usize) -> ListPosition {
    ListPosition { entity, index }
}
fn reference(
    a: &ListAssignment<i32>,
    members: &[i32],
    destination: Option<ListPosition>,
) -> ListAssignment<i32> {
    let lists = a
        .lists()
        .iter()
        .enumerate()
        .map(|(entity, list)| {
            let mut out = Vec::new();
            for index in 0..=list.len() {
                if destination == Some(pos(entity, index)) {
                    out.extend_from_slice(members);
                }
                if let Some(value) = list.get(index).filter(|v| !members.contains(v)) {
                    out.push(*value);
                }
            }
            out
        })
        .collect();
    ListAssignment::new(lists)
}
fn selections(prefix: Vec<i32>, out: &mut Vec<Vec<i32>>) {
    out.push(prefix.clone());
    for v in 0..4 {
        if !prefix.contains(&v) {
            let mut next = prefix.clone();
            next.push(v);
            selections(next, out);
        }
    }
}
#[test]
fn exhaustive_small_gathers_match_boundary_reference_and_rollback() {
    let mut samples = Vec::new();
    selections(vec![], &mut samples);
    assert_eq!(samples.len(), 65);
    let mut checked = 0;
    for mut layout in 0..81 {
        let mut lists = vec![vec![], vec![]];
        for value in 0..4 {
            let e = layout % 3;
            layout /= 3;
            if e < 2 {
                lists[e].push(value);
            }
        }
        // Non-numeric input order distinguishes identity order from list order.
        lists[0].reverse();
        let p = ListProblem::new(
            (0..4).collect(),
            lists.into_iter().map(ListEntity::new).collect(),
        )
        .unwrap();
        let original = p.initial_assignment();
        let destinations: Vec<_> = std::iter::once(None)
            .chain(
                original
                    .lists()
                    .iter()
                    .enumerate()
                    .flat_map(|(e, l)| (0..=l.len()).map(move |i| Some(pos(e, i)))),
            )
            .collect();
        for members in &samples {
            for &destination in &destinations {
                let m = p.gather_move(&original, members, destination).unwrap();
                let expected = reference(&original, members, destination);
                let mut a = original.clone();
                {
                    let trial = p.apply(&mut a, &m).unwrap();
                    assert_eq!(*trial, expected);
                }
                assert_eq!(a, original);
                p.apply(&mut a, &m).unwrap().commit();
                assert_eq!(a, expected);
                let mut keys = p.planning_values(&original, &m).unwrap();
                keys.sort();
                let mut expected_keys: Vec<_> = members
                    .iter()
                    .copied()
                    .filter(|v| destination.is_some() || original.location(v).is_some())
                    .collect();
                expected_keys.sort();
                assert_eq!(keys, expected_keys);
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 29835);
}
#[test]
fn invalid_inputs_ranges_and_pins_are_rejected_without_mutation() {
    let p = ListProblem::new(
        vec![0, 1, 2],
        vec![
            ListEntity::new(vec![0, 1]).with_pinned_prefix(1),
            ListEntity::new(vec![]).with_value_range(vec![2]),
        ],
    )
    .unwrap();
    let a = p.initial_assignment();
    assert_eq!(
        p.gather_move(&a, &[1, 1], None),
        Err(ListGatherError::DuplicateValue { index: 1 })
    );
    assert_eq!(
        p.gather_move(&a, &[9], None),
        Err(ListError::ValueOutsideDomain.into())
    );
    assert_eq!(
        p.gather_move(&a, &[], Some(pos(2, 0))),
        Err(ListError::EntityIndex {
            entity: 2,
            count: 2
        }
        .into())
    );
    assert_eq!(
        p.gather_move(&a, &[], Some(pos(0, 3))),
        Err(ListError::InsertionIndex(pos(0, 3)).into())
    );
    assert_eq!(
        p.gather_move(&a, &[0], None),
        Err(ListError::PinChanged { entity: 0 }.into())
    );
    assert_eq!(
        p.gather_move(&a, &[1], Some(pos(1, 0))),
        Err(ListError::ValueOutsideEntityRange { entity: 1 }.into())
    );
    // A composite may temporarily remove a pinned value if its final position is unchanged.
    assert!(p.gather_move(&a, &[0], Some(pos(0, 0))).is_ok());
    assert!(p.gather_move(&a, &[2], None).is_ok());
    assert!(p.gather_move(&a, &[], None).is_ok());
    assert_eq!(a, p.initial_assignment());
    let wrong = ListAssignment::new(vec![vec![0, 1], vec![1]]);
    assert!(p.gather_move(&wrong, &[], None).is_err());
    let frozen = ListProblem::new(
        vec![0, 1],
        vec![ListEntity::new(vec![0, 1]).with_pinned(true)],
    )
    .unwrap();
    assert!(
        frozen
            .gather_move(&frozen.initial_assignment(), &[0, 1], Some(pos(0, 0)))
            .is_ok()
    );
    assert!(
        frozen
            .gather_move(&frozen.initial_assignment(), &[1, 0], Some(pos(0, 0)))
            .is_err()
    );
    let empty = ListProblem::<i32>::new(vec![], vec![]).unwrap();
    assert_eq!(
        empty.gather_move(&empty.initial_assignment(), &[], None),
        Ok(ListMove::Composite(vec![]))
    );
}
fn route_score(entity: usize, list: &[i32]) -> i64 {
    list.iter()
        .enumerate()
        .map(|(i, v)| (entity as i64 + 1) * (i as i64 + 1) * (i64::from(*v) + 1))
        .sum()
}
#[test]
fn both_list_scoring_backends_commit_and_reject_gathers_with_partial_states() {
    let p = ListProblem::new(
        (0..4).collect(),
        vec![ListEntity::new(vec![0, 1]), ListEntity::new(vec![2])],
    )
    .unwrap();
    let mut net = ListConstraintNetwork::new(2, 4);
    for e in 0..2 {
        net.add_constraint(
            format!("route{e}"),
            vec![ListScope::Entity(e)],
            move |facts| {
                let ListFact::Entity(list) = &facts[0] else {
                    panic!("entity scope")
                };
                SimpleScore::new(route_score(e, list))
            },
        )
        .unwrap();
    }
    let mut b = StreamGraphBuilder::new();
    let sources = ListStreamSources::<i32>::new(&mut b);
    b.score("routes", sources.entities.port(), |(e, list)| {
        SimpleScore::new(route_score(*e, list))
    })
    .unwrap();
    let mut scoped = ListScoreDirector::new(&p, &net, p.initial_assignment()).unwrap();
    let mut stream =
        ListStreamScoreDirector::new(&p, b.build(), sources, p.initial_assignment()).unwrap();
    let mut samples = vec![];
    selections(vec![], &mut samples);
    for step in 0..260 {
        let original = scoped.assignment().clone();
        let old = scoped.score();
        let destination = if step % 4 == 0 {
            None
        } else {
            let e = step % 2;
            Some(pos(e, step % (original.lists()[e].len() + 1)))
        };
        let members = &samples[step % samples.len()];
        let m = p.gather_move(&original, members, destination).unwrap();
        let expected = reference(&original, members, destination);
        let assigned: usize = expected.lists().iter().map(Vec::len).sum();
        let score = SimpleScore::new(
            expected
                .lists()
                .iter()
                .enumerate()
                .map(|(e, l)| route_score(e, l))
                .sum(),
        )
        .with_structural(-((4 - assigned) as i64));
        let a = scoped.apply(&m).unwrap();
        let b = stream.apply(&m).unwrap();
        assert_eq!(a.assignment(), &expected);
        assert_eq!(b.assignment(), &expected);
        assert_eq!(a.score(), score);
        assert_eq!(b.score(), score);
        if step % 3 == 0 {
            drop(a);
            drop(b);
            assert_eq!(scoped.assignment(), &original);
            assert_eq!(stream.assignment(), &original);
            assert_eq!(scoped.score(), old);
            assert_eq!(stream.score(), old);
        } else {
            a.commit();
            b.commit();
        }
    }
}
#[derive(Debug)]
struct Value {
    id: i32,
    payload: &'static str,
    panic: Rc<Cell<bool>>,
}
impl Clone for Value {
    fn clone(&self) -> Self {
        assert!(!self.panic.get(), "injected clone panic");
        Self {
            id: self.id,
            payload: self.payload,
            panic: self.panic.clone(),
        }
    }
}
impl PartialEq for Value {
    fn eq(&self, b: &Self) -> bool {
        self.id == b.id
    }
}
impl Eq for Value {}
#[test]
fn assigned_payloads_are_preserved_and_clone_panics_leave_input_reusable() {
    let panic = Rc::new(Cell::new(false));
    let v = |id, payload| Value {
        id,
        payload,
        panic: panic.clone(),
    };
    let p = ListProblem::new(
        vec![v(0, "registry"), v(1, "registry")],
        vec![ListEntity::new(vec![v(0, "assigned")])],
    )
    .unwrap();
    let mut a = p.initial_assignment();
    let members = vec![v(1, "new"), v(0, "sample")];
    panic.set(true);
    assert!(
        catch_unwind(AssertUnwindSafe(|| p.gather_move(
            &a,
            &members,
            Some(pos(0, 0))
        )))
        .is_err()
    );
    panic.set(false);
    assert_eq!(a.list(0).unwrap()[0].payload, "assigned");
    let m = p.gather_move(&a, &members, Some(pos(0, 0))).unwrap();
    p.apply(&mut a, &m).unwrap().commit();
    assert_eq!(
        a.list(0)
            .unwrap()
            .iter()
            .map(|v| (v.id, v.payload))
            .collect::<Vec<_>>(),
        vec![(1, "new"), (0, "assigned")]
    );
}
