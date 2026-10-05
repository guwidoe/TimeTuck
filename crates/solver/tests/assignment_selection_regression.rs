use timetuck::*;
#[test]
fn native_basic_modes_distinguish_required_optional_pinned_and_current_assignments() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![1, 2]),
        Variable::new(vec![1, 2]).allowing_unassigned(),
        Variable::new(vec![1, 2]).with_initial(1).with_pinned(true),
        Variable::new(vec![1, 2]).with_initial(2),
    ])
    .unwrap();
    let a = p.initial_assignment();
    let c = BasicSelectionContext::new(&p, &a).unwrap();
    for (mode, expected) in [
        (BasicEntitySelection::All, vec![0, 1, 2, 3]),
        (BasicEntitySelection::Movable, vec![0, 1, 3]),
        (BasicEntitySelection::Uninitialized, vec![0]),
    ] {
        let mut s = BasicEntitySource(mode);
        assert_eq!(s.size(&c), Some(expected.len()));
        assert_eq!(s.open(&c).collect::<Vec<_>>(), expected);
    }
    let mut source =
        GatedSelectionSource::new(BasicValueSource, |v: &BasicValueContext<'_, i32>| {
            v.needs_initialization()
        });
    assert_eq!(
        source.open(&c.variable(0).unwrap()).collect::<Vec<_>>(),
        [Some(1), Some(2)]
    );
    for i in 1..4 {
        assert_eq!(source.size(&c.variable(i).unwrap()), Some(0));
        assert!(source.open(&c.variable(i).unwrap()).next().is_none());
    }
    let mut values = BasicValueSource;
    assert_eq!(
        values.open(&c.variable(1).unwrap()).collect::<Vec<_>>(),
        [Some(1), Some(2), None]
    );
    assert_eq!(values.size(&c.variable(1).unwrap()), Some(3));
    assert_eq!(c.variable(3).unwrap().current(), Some(&2));
    let b = Assignment::new(vec![Some(2), None, Some(1), None]);
    let context = BasicSelectionContext::new(&p, &b).unwrap();
    assert_eq!(
        BasicEntitySource(BasicEntitySelection::Uninitialized)
            .open(&context)
            .collect::<Vec<_>>(),
        [3]
    );
}
#[test]
fn list_modes_follow_current_ownership_and_both_pin_forms() {
    let p = ListProblem::new(
        vec![1, 2, 3, 4, 5, 6],
        vec![
            ListEntity::new(vec![1, 2]).with_pinned_prefix(1),
            ListEntity::new(vec![3]).with_pinned(true),
            ListEntity::new(vec![4]),
        ],
    )
    .unwrap();
    let a = p.initial_assignment();
    let c = ListSelectionContext::new(&p, &a).unwrap();
    for (mode, expected) in [
        (ListValueSelection::All, vec![1, 2, 3, 4, 5, 6]),
        (ListValueSelection::Assigned, vec![1, 2, 3, 4]),
        (ListValueSelection::Unassigned, vec![5, 6]),
        (ListValueSelection::Movable, vec![2, 4]),
    ] {
        let mut s = ListValueSource(mode);
        assert_eq!(s.size(&c), Some(expected.len()));
        assert_eq!(s.open(&c).collect::<Vec<_>>(), expected);
    }
    let b = ListAssignment::new(vec![vec![1], vec![3], vec![4, 5]]);
    let c = ListSelectionContext::new(&p, &b).unwrap();
    assert_eq!(
        ListValueSource(ListValueSelection::Unassigned)
            .open(&c)
            .collect::<Vec<_>>(),
        [2, 6]
    );
}
#[test]
fn invalid_contexts_and_unknown_variables_are_errors() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![1, 2]).with_initial(1).with_pinned(true),
    ])
    .unwrap();
    for a in [
        Assignment::new(vec![]),
        Assignment::new(vec![Some(3)]),
        Assignment::new(vec![Some(2)]),
    ] {
        assert!(BasicSelectionContext::new(&p, &a).is_err());
    }
    let a = p.initial_assignment();
    let c = BasicSelectionContext::new(&p, &a).unwrap();
    assert!(matches!(
        c.variable(1),
        Err(MoveError {
            variable: 1,
            variable_count: 1
        })
    ));
    let p = ListProblem::new(vec![1, 2], vec![ListEntity::new(vec![])]).unwrap();
    for a in [
        ListAssignment::new(vec![]),
        ListAssignment::new(vec![vec![1, 1]]),
        ListAssignment::new(vec![vec![3]]),
    ] {
        assert!(ListSelectionContext::new(&p, &a).is_err());
    }
}
struct Bomb {
    events: usize,
}
impl SelectionSource<bool, usize> for Bomb {
    fn event(&mut self, _: SelectionLifecycleEvent, _: &bool) {
        self.events += 1;
    }
    fn size(&mut self, _: &bool) -> Option<usize> {
        panic!("disabled size")
    }
    fn open<'a>(&'a mut self, _: &'a bool) -> Box<dyn Iterator<Item = usize> + 'a>
    where
        usize: 'a,
    {
        panic!("disabled open")
    }
}
#[test]
fn disabled_sources_forward_events_without_touching_child_data() {
    let mut s = GatedSelectionSource::new(Bomb { events: 0 }, |enabled: &bool| *enabled);
    s.event(SelectionLifecycleEvent::StepStarted, &false);
    assert_eq!(s.size(&false), Some(0));
    assert_eq!(s.open(&false).next(), None);
    assert_eq!(s.source().events, 1);
}
#[test]
fn native_entity_and_value_sources_initialize_with_both_scoring_backends() {
    let p = FiniteProblem::new(vec![Variable::new(vec![1, 2]), Variable::new(vec![3, 4])]).unwrap();
    let mut n = ConstraintNetwork::new(2);
    n.add_constraint("assigned", vec![0, 1], |v| {
        SimpleScore::new(v.iter().filter(|v| v.is_some()).count() as i64)
    })
    .unwrap();
    let mut scoped = ScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
    let mut b = StreamGraphBuilder::new();
    let source = b.source::<usize, (usize, Option<i32>)>();
    b.score("assigned", source.port(), |r| {
        SimpleScore::new(i64::from(r.1.is_some()))
    })
    .unwrap();
    let mut stream =
        StreamScoreDirector::new(&p, b.build(), source, p.initial_assignment()).unwrap();
    let mut entities = BasicEntitySource(BasicEntitySelection::Uninitialized);
    let mut values =
        GatedSelectionSource::new(BasicValueSource, |v: &BasicValueContext<'_, i32>| {
            v.needs_initialization()
        });
    for expected in 1..=2 {
        let c = BasicSelectionContext::new(&p, scoped.assignment()).unwrap();
        let variable = entities.open(&c).next().unwrap();
        let value = values.open(&c.variable(variable).unwrap()).next().unwrap();
        let movement = Move::Change { variable, value };
        scoped.apply(&movement).unwrap().commit();
        stream.apply(&movement).unwrap().commit();
        scoped.assert_consistent().unwrap();
        stream.assert_consistent().unwrap();
        assert_eq!(scoped.assignment(), stream.assignment());
        assert_eq!(
            scoped.score(),
            SimpleScore::new(expected).with_structural(expected - 2)
        );
        assert_eq!(stream.score(), scoped.score());
    }
    let c = BasicSelectionContext::new(&p, scoped.assignment()).unwrap();
    assert!(entities.open(&c).next().is_none());
    assert!(p.is_complete(scoped.assignment()));
}
#[test]
fn native_unassigned_values_initialize_lists_with_both_scoring_backends() {
    let p = ListProblem::new(vec![1, 2, 3], vec![ListEntity::new(vec![1])]).unwrap();
    let mut n = ListConstraintNetwork::new(1, 3);
    n.add_constraint("count", vec![ListScope::Entity(0)], |facts| {
        let ListFact::Entity(values) = &facts[0] else {
            unreachable!()
        };
        SimpleScore::new(values.len() as i64)
    })
    .unwrap();
    let mut scoped = ListScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
    let mut b = StreamGraphBuilder::new();
    let sources = ListStreamSources::new(&mut b);
    b.score("count", sources.values.port(), |v| {
        SimpleScore::new(i64::from(v.position.is_some()))
    })
    .unwrap();
    let mut stream =
        ListStreamScoreDirector::new(&p, b.build(), sources, p.initial_assignment()).unwrap();
    let mut source = ListValueSource(ListValueSelection::Unassigned);
    for expected in 2..=3 {
        let c = ListSelectionContext::new(&p, scoped.assignment()).unwrap();
        let value = source.open(&c).next().unwrap();
        let movement = ListMove::Assign {
            value,
            destination: ListPosition {
                entity: 0,
                index: scoped.assignment().lists()[0].len(),
            },
        };
        scoped.apply(&movement).unwrap().commit();
        stream.apply(&movement).unwrap().commit();
        scoped.assert_consistent().unwrap();
        stream.assert_consistent().unwrap();
        assert_eq!(scoped.assignment(), stream.assignment());
        assert_eq!(
            scoped.score(),
            SimpleScore::new(expected).with_structural(expected - 3)
        );
        assert_eq!(stream.score(), scoped.score());
    }
    let c = ListSelectionContext::new(&p, scoped.assignment()).unwrap();
    assert!(source.open(&c).next().is_none());
    assert!(p.is_complete(scoped.assignment()));
}
