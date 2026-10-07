// Adapted from pinned AbstractBiConstraintStreamTest assigned-list join/existence cases.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;
fn build(exists: bool, replacement: bool) {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let sources = ListStreamSources::<usize>::new(&mut b);
    let assigned = b
        .filter(sources.values.port(), |v| v.position.is_some())
        .unwrap();
    let pairs = b
        .join_equal(sources.entities.port(), &assigned, |_| (), |_| ())
        .unwrap();
    if exists {
        let rows = b
            .existence(
                &pairs,
                &assigned,
                ExistenceMode::Exists,
                |(_, a)| a.position.unwrap().entity,
                |b| b.position.unwrap().entity,
                |(_, a), b| {
                    assert!(a.position.is_some());
                    assert!(b.position.is_some());
                    true
                },
            )
            .unwrap();
        let rows = b.map(&rows, |((id, _), a)| vec![*id, a.value]).unwrap();
        run(b, sources, rows, true, replacement);
    } else {
        let rows = b
            .join_equal(
                &pairs,
                &assigned,
                |(_, a)| a.position.unwrap().entity,
                |b| b.position.unwrap().entity,
            )
            .unwrap();
        let rows = b
            .filter(&rows, |((_, a), b)| {
                assert!(a.position.is_some());
                assert!(b.position.is_some());
                true
            })
            .unwrap();
        let rows = b
            .map(&rows, |(((id, _), a), b)| vec![*id, a.value, b.value])
            .unwrap();
        run(b, sources, rows, false, replacement);
    }
}
fn run<I: Ord + Clone + 'static>(
    mut b: StreamGraphBuilder<'static, SimpleScore>,
    sources: ListStreamSources<usize>,
    rows: StreamPort<I, Vec<usize>>,
    exists: bool,
    replacement: bool,
) {
    let terminal = b
        .score("testConstraint", &rows, |_| SimpleScore::new(-1))
        .unwrap();
    let p = ListProblem::new(vec![1, 2], vec![ListEntity::new(vec![])])
        .unwrap()
        .allowing_unassigned();
    let mut d =
        ListStreamScoreDirector::new(&p, b.build(), sources, p.initial_assignment()).unwrap();
    let check = |d: &ListStreamScoreDirector<'_, '_, usize, SimpleScore>, values: &[usize]| {
        let mut expected = vec![];
        for &a in values {
            if exists {
                expected.push(vec![0, a]);
            } else {
                for &b in values {
                    expected.push(vec![0, a, b]);
                }
            }
        }
        let mut actual: Vec<_> = d
            .graph()
            .constraint_matches(&terminal)
            .unwrap()
            .values()
            .map(|(r, s)| {
                assert_eq!(*s, SimpleScore::new(-1));
                r.clone()
            })
            .collect();
        expected.sort();
        actual.sort();
        assert_eq!(actual, expected);
        assert_eq!(
            d.graph().score(),
            SimpleScore::new(-(expected.len() as i64))
        );
    };
    let assign = |value, index| ListMove::Assign {
        value,
        destination: ListPosition { entity: 0, index },
    };
    if replacement {
        d.apply(&assign(1, 0)).unwrap().commit();
        check(&d, &[1]);
        for value in [2, 1] {
            d.apply(&ListMove::Composite(vec![
                ListMove::Unassign {
                    entity: 0,
                    start: 0,
                    end: 1,
                },
                assign(value, 0),
            ]))
            .unwrap()
            .commit();
            check(&d, &[value]);
        }
    } else {
        d.apply(&ListMove::Composite(vec![assign(1, 0), assign(2, 1)]))
            .unwrap()
            .commit();
        check(&d, &[1, 2]);
        d.apply(&ListMove::Unassign {
            entity: 0,
            start: 0,
            end: 1,
        })
        .unwrap()
        .commit();
        check(&d, &[2]);
        d.apply(&assign(1, 1)).unwrap().commit();
        check(&d, &[1, 2]);
    }
}
#[test]
fn join_filter_on_assigned_value_unassign_one() {
    build(false, false)
}
#[test]
fn join_filter_on_assigned_value_unassign_one_reassign_other() {
    build(false, true)
}
#[test]
fn if_exists_filter_on_assigned_value_unassign_one() {
    build(true, false)
}
#[test]
fn if_exists_filter_on_assigned_value_unassign_one_reassign_other() {
    build(true, true)
}
