//! Original borrowed gather construction and exact native draw regressions.
use std::{cell::Cell, num::NonZeroUsize, rc::Rc};
use timetuck::*;

// Deliberately simple owned-row reference, independent of cached locations and
// touched-row validation. Preserve the native pre-optimization operation order.
fn reference<V: Clone + Eq>(
    p: &ListProblem<V>,
    before: &ListAssignment<V>,
    members: &[V],
    destination: Option<ListPosition>,
) -> Result<ListMove<V>, ListGatherError> {
    p.validate(before)?;
    for (index, value) in members.iter().enumerate() {
        if members[..index].contains(value) {
            return Err(ListGatherError::DuplicateValue { index });
        }
        if !p.values().contains(value) {
            return Err(ListError::ValueOutsideDomain.into());
        }
    }
    let insertion = if let Some(pos) = destination {
        let row = before.list(pos.entity).ok_or(ListError::EntityIndex {
            entity: pos.entity,
            count: before.lists().len(),
        })?;
        if pos.index > row.len() {
            return Err(ListError::InsertionIndex(pos).into());
        }
        Some(ListPosition {
            entity: pos.entity,
            index: row[..pos.index]
                .iter()
                .filter(|v| !members.contains(v))
                .count(),
        })
    } else {
        None
    };
    let mut rows = before.lists().to_vec();
    let mut steps = vec![];
    let mut gathered = vec![];
    for member in members {
        let source = rows.iter().enumerate().find_map(|(entity, row)| {
            row.iter()
                .position(|value| value == member)
                .map(|index| ListPosition { entity, index })
        });
        if let Some(pos) = source {
            let value = rows[pos.entity].remove(pos.index);
            steps.push(ListMove::Unassign {
                entity: pos.entity,
                start: pos.index,
                end: pos.index + 1,
            });
            if insertion.is_some() {
                gathered.push(value);
            }
        } else if insertion.is_some() {
            gathered.push(member.clone());
        }
    }
    if let Some(pos) = insertion {
        for (offset, value) in gathered.into_iter().enumerate() {
            let destination = ListPosition {
                entity: pos.entity,
                index: pos.index + offset,
            };
            steps.push(ListMove::Assign {
                value: value.clone(),
                destination,
            });
            rows[destination.entity].insert(destination.index, value);
        }
    }
    p.validate(&ListAssignment::new(rows))?;
    Ok(ListMove::Composite(steps))
}

fn model(indexed: bool, shape: usize) -> ListProblem<usize> {
    let mut rows = vec![
        ListEntity::new(vec![0, 1, 2]),
        ListEntity::new(vec![3]),
        ListEntity::new(vec![]),
    ];
    if shape == 1 {
        rows[0] = rows[0]
            .clone()
            .with_pinned_prefix(1)
            .with_value_range(vec![0, 1, 2, 4])
            .with_range_diagnostics("first", "tasks", |value| format!("task {value}"));
        rows[1] = rows[1].clone().with_pinned(true);
        rows[2] = rows[2]
            .clone()
            .with_value_range(vec![1, 3, 4, 5])
            .with_range_diagnostics("third", "tasks", |value| format!("task {value}"));
    }
    let values = vec![5, 2, 0, 4, 1, 3];
    let p = if indexed {
        ListProblem::new_indexed(values, rows)
    } else {
        ListProblem::new(values, rows)
    }
    .unwrap();
    if shape == 2 {
        // Registry 1 (value2) depends on registry4 (value1), which depends on0.
        p.with_dependencies(vec![vec![], vec![4], vec![], vec![], vec![2], vec![]])
            .unwrap()
    } else {
        p
    }
}

#[test]
fn borrowed_gathers_match_owned_reference_composites_and_error_precedence() {
    for indexed in [false, true] {
        for shape in 0..3 {
            let p = model(indexed, shape);
            let a = p.initial_assignment();
            let mut groups = vec![vec![]];
            for first in 0..7 {
                groups.push(vec![first]);
                for second in 0..7 {
                    groups.push(vec![first, second]);
                    for third in 0..7 {
                        groups.push(vec![first, second, third]);
                    }
                }
            }
            let destinations: Vec<_> = std::iter::once(None)
                .chain((0..4).flat_map(|entity| {
                    (0..=a.list(entity).map_or(1, |row| row.len() + 1))
                        .map(move |index| Some(ListPosition { entity, index }))
                }))
                .collect();
            for members in &groups {
                for &destination in &destinations {
                    assert_eq!(
                        p.gather_move(&a, members, destination),
                        reference(&p, &a, members, destination),
                        "{indexed} {shape} {members:?} {destination:?}"
                    );
                }
            }
            let invalid = ListAssignment::new(vec![vec![6], vec![], vec![]]);
            assert_eq!(
                p.gather_move(
                    &invalid,
                    &[6, 6],
                    Some(ListPosition {
                        entity: 9,
                        index: 9
                    })
                ),
                reference(
                    &p,
                    &invalid,
                    &[6, 6],
                    Some(ListPosition {
                        entity: 9,
                        index: 9
                    })
                )
            );
            assert_eq!(a, p.initial_assignment());
        }
    }
}

fn reference_samples(
    p: &ListProblem<usize>,
    a: &ListAssignment<usize>,
    c: GatherSamplingConfig,
) -> Vec<ListMove<usize>> {
    let eligible: Vec<_> = p
        .values()
        .iter()
        .copied()
        .filter(|value| {
            a.location(value).is_none_or(|pos| {
                !p.entities()[pos.entity].is_pinned()
                    && pos.index >= p.entities()[pos.entity].pinned_prefix()
            })
        })
        .collect();
    let max = c.maximum().min(eligible.len());
    if max < c.minimum() {
        return vec![];
    }
    let mut random = SampleRandom::new(c.seed);
    let mut indices: Vec<_> = (0..eligible.len()).collect();
    let draw = |rng: &mut SampleRandom, bound: usize| {
        if bound == 1 {
            0
        } else {
            rng.index(NonZeroUsize::new(bound).unwrap())
        }
    };
    let index = ListDestinationIndex::new(p, a, c.unassignment).unwrap();
    let mut result = vec![];
    for _ in 0..c.attempts {
        let size = c.minimum() + draw(&mut random, max - c.minimum() + 1);
        for i in 0..size {
            let chosen = i + draw(&mut random, indices.len() - i);
            indices.swap(i, chosen);
        }
        let members: Vec<_> = indices[..size].iter().map(|&i| eligible[i]).collect();
        if let Some(destination) = index.for_values(&members).unwrap().sample(&mut random)
            && let Ok(movement) = reference(p, a, &members, destination)
        {
            let mut next = a.clone();
            p.apply(&mut next, &movement).unwrap().commit();
            if next != *a {
                result.push(movement);
            }
        }
    }
    result
}

#[test]
fn sampled_gathers_preserve_exact_draws_rejections_and_abandoned_iteration() {
    for indexed in [false, true] {
        for shape in 0..3 {
            for optional in [false, true] {
                let p = model(indexed, shape);
                let p = if optional { p.allowing_unassigned() } else { p };
                let a = p.initial_assignment();
                for seed in [0, 93, u64::MAX] {
                    let mut c = GatherSamplingConfig::new(1, 4, 100, seed).unwrap();
                    c.unassignment = optional;
                    let expected = reference_samples(&p, &a, c);
                    assert_eq!(
                        sampled_list_gather_moves(&p, &a, c)
                            .unwrap()
                            .collect::<Vec<_>>(),
                        expected
                    );
                    assert_eq!(
                        sampled_list_gather_moves(&p, &a, c).unwrap().next(),
                        expected.first().cloned()
                    );
                    assert_eq!(
                        sampled_list_gather_moves(&p, &a, c)
                            .unwrap()
                            .collect::<Vec<_>>(),
                        expected
                    );
                }
            }
        }
    }
}

#[derive(Debug)]
struct Value {
    id: usize,
    payload: &'static str,
    clones: Rc<Cell<usize>>,
}
impl Clone for Value {
    fn clone(&self) -> Self {
        self.clones.set(self.clones.get() + 1);
        Self {
            id: self.id,
            payload: self.payload,
            clones: self.clones.clone(),
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
fn borrowed_construction_clones_only_output_values_and_preserves_payload_sources() {
    let clones = Rc::new(Cell::new(0));
    let value = |id, payload| Value {
        id,
        payload,
        clones: clones.clone(),
    };
    let p = ListProblem::new(
        (0..200).map(|id| value(id, "registry")).collect(),
        (0..99)
            .map(|e| ListEntity::new(vec![value(e * 2, "assigned"), value(e * 2 + 1, "assigned")]))
            .collect(),
    )
    .unwrap();
    let a = p.initial_assignment();
    let members = vec![
        value(1, "supplied"),
        value(199, "supplied"),
        value(0, "supplied"),
    ];
    clones.set(0);
    let movement = p
        .gather_move(
            &a,
            &members,
            Some(ListPosition {
                entity: 0,
                index: 1,
            }),
        )
        .unwrap();
    assert_eq!(clones.get(), 3);
    let ListMove::Composite(steps) = movement else {
        panic!("composite");
    };
    let assigned: Vec<_> = steps
        .iter()
        .filter_map(|step| {
            if let ListMove::Assign { value, .. } = step {
                Some((value.id, value.payload))
            } else {
                None
            }
        })
        .collect();
    assert_eq!(
        assigned,
        vec![(1, "assigned"), (199, "supplied"), (0, "assigned")]
    );
    clones.set(0);
    p.gather_move(&a, &members, None).unwrap();
    assert_eq!(clones.get(), 0);
}

#[derive(Clone)]
struct Identity(usize, Rc<Cell<usize>>);
impl PartialEq for Identity {
    fn eq(&self, other: &Self) -> bool {
        self.1.set(self.1.get() + 1);
        self.0 == other.0
    }
}
impl Eq for Identity {}
impl std::hash::Hash for Identity {
    fn hash<H: std::hash::Hasher>(&self, hasher: &mut H) {
        self.0.hash(hasher);
    }
}
#[test]
fn indexed_dependency_validation_keeps_registry_lookups_linear() {
    let calls = Rc::new(Cell::new(0));
    let values: Vec<_> = (0..200).map(|id| Identity(id, calls.clone())).collect();
    let p = ListProblem::new_indexed(
        values.clone(),
        values
            .iter()
            .map(|value| ListEntity::new(vec![value.clone()]))
            .collect(),
    )
    .unwrap()
    .with_dependencies(vec![vec![]; 200])
    .unwrap();
    let a = p.initial_assignment();
    calls.set(0);
    p.gather_move(
        &a,
        &values[198..],
        Some(ListPosition {
            entity: 0,
            index: 1,
        }),
    )
    .unwrap();
    assert!(calls.get() < 200 * 20, "{} equality calls", calls.get());
}
