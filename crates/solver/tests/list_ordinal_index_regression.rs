//! Original checked ordinal registry contracts; no upstream method credit.
use timetuck::*;
#[path = "../examples/mixed_benchmark_support/mod.rs"]
#[allow(dead_code)]
mod support;
#[derive(Clone, Debug)]
struct Value {
    key: usize,
    ordinal: usize,
    label: String,
} // Deliberately no Hash or Ord.
impl PartialEq for Value {
    fn eq(&self, b: &Self) -> bool {
        self.key == b.key
    }
}
impl Eq for Value {}
fn v(key: usize, ordinal: usize) -> Value {
    Value {
        key,
        ordinal,
        label: format!("payload-{key}"),
    }
}
fn key(v: &Value) -> usize {
    v.ordinal
}
fn pos(entity: usize, index: usize) -> ListPosition {
    ListPosition { entity, index }
}
#[test]
fn nonhash_payloads_arbitrary_registry_order_clones_and_outside_queries() {
    let values = vec![v(91, 0), v(7, 1), v(42, 2)];
    let p = ListProblem::new_ordinal_indexed(
        values.clone(),
        vec![ListEntity::new(vec![values[1].clone()])],
        key,
    )
    .unwrap()
    .allowing_unassigned();
    assert_eq!(
        p.values().iter().map(|v| v.key).collect::<Vec<_>>(),
        vec![91, 7, 42]
    );
    for (id, value) in values.iter().enumerate() {
        assert_eq!(p.value_index(value), Some(id));
        assert_eq!(p.clone().value_index(value), Some(id));
    }
    assert_eq!(p.value_index(&v(999, 0)), None);
    assert_eq!(p.value_index(&v(999, usize::MAX)), None);
    assert_eq!(p.values()[0].label, "payload-91");
    let before = p.initial_assignment();
    let mut a = before.clone();
    assert!(matches!(
        p.apply(
            &mut a,
            &ListMove::Assign {
                value: v(999, usize::MAX),
                destination: pos(0, 0)
            }
        ),
        Err(ListError::ValueOutsideDomain)
    ));
    assert_eq!(a, before);
}
#[test]
fn invalid_registry_ordinals_duplicates_ranges_and_pins_are_typed() {
    assert_eq!(
        ListProblem::new_ordinal_indexed(vec![v(10, 2)], vec![], key).unwrap_err(),
        ListError::InvalidValueOrdinal {
            expected: 0,
            actual: 2
        }
    );
    assert_eq!(
        ListProblem::new_ordinal_indexed(vec![v(10, usize::MAX)], vec![], key).unwrap_err(),
        ListError::InvalidValueOrdinal {
            expected: 0,
            actual: usize::MAX
        }
    );
    assert_eq!(
        ListProblem::new_ordinal_indexed(vec![v(10, 0), v(10, 0)], vec![], key).unwrap_err(),
        ListError::DuplicateDomainValue
    );
    assert_eq!(
        ListProblem::new(vec![v(10, 0), v(10, 0)], vec![]).unwrap_err(),
        ListError::DuplicateDomainValue
    );
    for ordinal in [false, true] {
        let make = |entities| {
            if ordinal {
                ListProblem::new_ordinal_indexed(vec![0usize, 1], entities, |v| *v)
            } else {
                ListProblem::new(vec![0usize, 1], entities)
            }
        };
        assert_eq!(
            make(vec![
                ListEntity::new(vec![]).with_value_range(vec![usize::MAX])
            ])
            .unwrap_err(),
            ListError::EntityRangeValueOutsideDomain { entity: 0 }
        );
        assert_eq!(
            make(vec![ListEntity::new(vec![]).with_value_range(vec![1, 1])]).unwrap_err(),
            ListError::DuplicateEntityRangeValue { entity: 0 }
        );
        assert_eq!(
            make(vec![ListEntity::new(vec![0]).with_pinned_prefix(2)]).unwrap_err(),
            ListError::InvalidPin { entity: 0 }
        );
        assert_eq!(
            make(vec![ListEntity::new(vec![0, 0])]).unwrap_err(),
            ListError::DuplicateAssignedValue
        );
        assert_eq!(
            make(vec![ListEntity::new(vec![usize::MAX])]).unwrap_err(),
            ListError::ValueOutsideDomain
        );
    }
}
#[test]
fn ordering_ranges_pins_and_atomic_composites_match_eq_only() {
    fn rank(v: &usize) -> usize {
        match v {
            91 => 0,
            7 => 1,
            42 => 2,
            _ => usize::MAX,
        }
    }
    let values = vec![91, 7, 42];
    let entities = vec![
        ListEntity::new(vec![91, 7])
            .with_pinned_prefix(1)
            .with_value_range(values.clone()),
        ListEntity::new(vec![42]).with_value_range(vec![7, 42]),
    ];
    let p = ListProblem::new(values.clone(), entities.clone()).unwrap();
    let q = ListProblem::new_ordinal_indexed(values, entities, rank).unwrap();
    for n in [
        Neighborhood::Change,
        Neighborhood::Swap,
        Neighborhood::UniqueSwap,
    ] {
        assert_eq!(
            list_moves(&p, &p.initial_assignment(), n.clone())
                .unwrap()
                .collect::<Vec<_>>(),
            list_moves(&q, &q.initial_assignment(), n)
                .unwrap()
                .collect::<Vec<_>>()
        );
    }
    let movement = ListMove::Composite(vec![
        ListMove::Change {
            source: pos(0, 1),
            destination: pos(1, 1),
        },
        ListMove::Change {
            source: pos(0, 0),
            destination: pos(1, 0),
        },
    ]);
    for problem in [&p, &q] {
        let mut a = problem.initial_assignment();
        let before = a.clone();
        assert!(problem.apply(&mut a, &movement).is_err());
        assert_eq!(a, before);
    }
}
const SMALL: &str =
    "ordinal-small\t1:1,2:0,18:0,19:1\t0:0,20:0\t2,2\t0,0\t2,0;3,1\t00\t0,0\t16\t10000";
fn case(ordinal: bool) -> support::Case {
    let mut c = support::Case::parse_with_registry(
        SMALL,
        if ordinal {
            support::ListRegistryPolicy::Ordinal
        } else {
            support::ListRegistryPolicy::EqOnly
        },
    )
    .unwrap();
    c.stream_graph_policy = support::StreamGraphPolicy::FusedOwnerLinks;
    c
}
fn rows(
    g: &StreamGraph<'_, HardSoftScore>,
    s: &MixedStreamSources<usize, usize>,
    a: &MixedAssignment<usize, usize>,
) {
    let basic = g.source_rows(&s.basic).unwrap();
    let entities = g.source_rows(&s.lists.entities).unwrap();
    let values = g.source_rows(&s.lists.values).unwrap();
    assert_eq!(basic.len(), 2);
    assert_eq!(entities.len(), 2);
    assert_eq!(values.len(), 4);
    for (i, v) in a.basic().values().iter().enumerate() {
        assert_eq!(basic[&i], (i, *v));
    }
    for (i, r) in a.lists().lists().iter().enumerate() {
        assert_eq!(entities[&i], (i, r.clone()));
    }
    for v in 0..4 {
        let mut expected = ListValueRow {
            range_index: v,
            value: v,
            position: None,
            previous: None,
            next: None,
        };
        for (e, r) in a.lists().lists().iter().enumerate() {
            if let Some(i) = r.iter().position(|x| *x == v) {
                expected.position = Some(pos(e, i));
                expected.previous = i.checked_sub(1).map(|j| r[j]);
                expected.next = r.get(i + 1).copied();
            }
        }
        assert_eq!(values[&v], expected);
    }
}
#[test]
fn actual_mixed_dense_and_stream_preserve_all_rows_shadows_scores_and_rollback() {
    let mut observations = Vec::new();
    for ordinal in [false, true] {
        let c = case(ordinal);
        let mut net = MixedConstraintNetwork::new(2, 2, 4);
        for owner in 0..2 {
            let c = &c;
            net.add_constraint(
                format!("route {owner}"),
                vec![MixedScope::Basic(owner), MixedScope::ListEntity(owner)],
                move |facts| {
                    let (MixedFact::Basic(d), MixedFact::ListEntity(r)) = (&facts[0], &facts[1])
                    else {
                        unreachable!()
                    };
                    c.route_score(*d.unwrap(), r, owner)
                },
            )
            .unwrap();
        }
        let mut dense =
            MixedScoreDirector::new(&c.problem, &net, c.problem.initial_assignment()).unwrap();
        let mut b = StreamGraphBuilder::new();
        let s = MixedStreamSources::new(&mut b);
        let inspect = s.clone();
        let handle = support::declare_owner_routes(
            &mut b,
            s.basic.port(),
            s.lists.entities.port(),
            2,
            support::StreamGraphPolicy::FusedOwnerLinks,
            |((owner, d), (_, r))| c.route_score(d.unwrap(), r, *owner),
        )
        .unwrap();
        let mut stream =
            MixedStreamScoreDirector::new(&c.problem, b.build(), s, c.problem.initial_assignment())
                .unwrap();
        let support::OwnerRouteConstraint::FusedOwnerLinks(handle) = handle else {
            unreachable!()
        };
        let saved = stream.graph().constraint_matches(&handle).unwrap().clone();
        let initial = stream.assignment().clone();
        let movement = MixedMove {
            basic: Move::Change {
                variable: 0,
                value: Some(1),
            },
            lists: ListMove::Swap {
                left: pos(0, 0),
                right: pos(0, 1),
            },
        };
        for commit in [false, true] {
            let a = dense.apply(&movement).unwrap();
            let b = stream.apply(&movement).unwrap();
            assert_eq!(a.assignment(), b.assignment());
            assert_eq!(a.score(), b.score());
            assert_eq!(b.score(), c.score(b.assignment()));
            rows(b.graph(), &inspect, b.assignment());
            if commit {
                a.commit();
                b.commit();
            } else {
                drop(a);
                drop(b);
                assert_eq!(stream.assignment(), &initial);
            }
        }
        rows(stream.graph(), &inspect, stream.assignment());
        assert_eq!(saved[&((0, 0), 0)].0, ((0, Some(0)), (0, vec![2, 0])));
        assert_eq!(dense.assignment(), stream.assignment());
        observations.push((
            stream.assignment().clone(),
            stream.score(),
            stream.graph().constraint_matches(&handle).unwrap().clone(),
        ));
    }
    assert_eq!(observations[0], observations[1]);
}
#[test]
fn real_warm_search_and_phase_handoffs_keep_quality_assignments_and_logical_work() {
    for stream in [false, true] {
        let p = case(false);
        let q = case(true);
        let a = support::run(&p, stream).unwrap();
        let b = support::run(&q, stream).unwrap();
        let a_best = a.best.unwrap();
        let b_best = b.best.unwrap();
        assert_eq!(a_best.assignment, b_best.assignment);
        assert_eq!(a_best.score, b_best.score);
        assert_eq!(a.evaluations, b.evaluations);
        assert_eq!(a.completed_steps, b.completed_steps);
        q.validate(&b_best.assignment, b_best.score).unwrap();
        let a = support::run_component_refinement(&p, stream, false).unwrap();
        let b = support::run_component_refinement(&q, stream, false).unwrap();
        assert_eq!(a.assignment, b.assignment);
        assert_eq!(a.score, b.score);
        assert_eq!(a.work, b.work);
        assert_eq!(a.phases, b.phases);
        q.validate(&b.assignment, b.score.unwrap()).unwrap();
    }
}

#[test]
fn cold_reconstruction_preserves_index_pins_quality_and_all_phase_work() {
    for pinned in [false, true] {
        let input = if pinned {
            SMALL.replace("\t00\t0,0\t", "\t10\t0,1\t")
        } else {
            SMALL.to_owned()
        } + "\t0:-78";
        let mut cases = Vec::new();
        for policy in [
            support::ListRegistryPolicy::EqOnly,
            support::ListRegistryPolicy::Ordinal,
        ] {
            let mut c = support::Case::parse_with_registry(&input, policy).unwrap();
            c.stream_graph_policy = support::StreamGraphPolicy::FusedOwnerLinks;
            let p = support::cold_problem(&c).unwrap();
            assert_eq!(p.lists().value_index(&usize::MAX), None);
            if pinned {
                assert_eq!(p.initial_assignment().basic().values(), &[Some(0), None]);
                assert_eq!(
                    p.initial_assignment().lists().lists(),
                    &[vec![2, 0], vec![3]]
                );
            }
            cases.push(c);
        }
        for stream in [false, true] {
            for refinement in [false, true] {
                let solve = |c: &support::Case| {
                    if refinement {
                        support::run_component_refinement(c, stream, true)
                    } else {
                        support::run_cold(c, stream)
                    }
                };
                let a = solve(&cases[0]).unwrap();
                let b = solve(&cases[1]).unwrap();
                assert!(a.complete && b.complete);
                assert_eq!(a.assignment, b.assignment);
                assert_eq!(a.score, b.score);
                assert_eq!(a.work, b.work);
                assert_eq!(a.phases, b.phases);
                cases[1].validate(&b.assignment, b.score.unwrap()).unwrap();
            }
        }
    }
}
