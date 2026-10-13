//! Original actual manager/director publication and pre-callback metadata errors.
use std::{cell::Cell, sync::Arc};
use timetuck::*;
struct Entity;
type Domain = ManagedSolution<Assignment<i32>, (), Arc<SimpleScore>>;
fn layout<P: 'static, S: 'static>() -> Arc<ComposedPlanningSolution> {
    let entities = PlanningComposition::new(vec![PlanningComponent::registered::<Entity>(vec![
        PlanningVariableDefinition::basic::<i32>("value", false),
    ])])
    .unwrap();
    let solution =
        SolutionComposition::new(vec![PlanningSolutionComponent::registered::<P>(vec![
            SolutionMemberDefinition::entity_collection::<Entity>("entities"),
            SolutionMemberDefinition::score::<S>("score"),
        ])])
        .unwrap();
    Arc::new(
        solution
            .compile::<P>(
                &entities,
                &[PlanningType::of::<Entity>()],
                SolutionCompositionPolicy::default(),
                PlanningCompositionPolicy::default(),
            )
            .unwrap(),
    )
}
#[test]
fn actual_manager_refresh_and_director_publication_share_the_live_score_member() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]).with_initial(1)]).unwrap();
    let mut n = ConstraintNetwork::new(1);
    n.add_constraint("value", vec![0], |v| {
        SimpleScore::new(v[0].copied().map_or(0, i64::from))
    })
    .unwrap();
    let accessor = SolutionScoreAccessor::new(
        layout::<Domain, SimpleScore>(),
        "score",
        |s: &Domain| s.score.as_ref(),
        |s, score| s.score = score,
    )
    .unwrap();
    let mut domain = Domain::new(p.initial_assignment(), ());
    let mut manager = SolutionManager::new(
        |_| Ok::<_, ScoringError>(()),
        |a: &Assignment<i32>, _: &()| p.validate(a).map_err(ScoringError::from),
        |a, _| n.calculate(&p, a).map(Arc::new),
    );
    let updated = manager.update(&mut domain).unwrap().unwrap().clone();
    assert!(Arc::ptr_eq(accessor.read(&domain).unwrap(), &updated));
    assert_eq!(*updated, SimpleScore::new(1));
    let mut director = ScoreDirector::new(&p, &n, domain.assignment.clone()).unwrap();
    director
        .apply(&Move::Change {
            variable: 0,
            value: Some(2),
        })
        .unwrap()
        .commit();
    domain.assignment = director.assignment().clone();
    let published = accessor.publish(&mut domain, director.score());
    assert!(Arc::ptr_eq(accessor.read(&domain).unwrap(), &published));
    assert_eq!(*published, SimpleScore::new(2));
    assert_eq!(*updated, SimpleScore::new(1));
    domain.assignment = Assignment::new(vec![Some(99)]);
    assert!(
        accessor
            .refresh(&mut domain, |s| n.calculate(&p, &s.assignment))
            .is_err()
    );
    assert!(Arc::ptr_eq(accessor.read(&domain).unwrap(), &published));
    accessor.write(&mut domain, None);
    assert!(accessor.read(&domain).is_none());
}
#[test]
fn metadata_errors_invoke_neither_getter_nor_setter() {
    struct Other;
    let calls = Cell::new(0);
    let build = |layout, member| {
        SolutionScoreAccessor::<Domain, SimpleScore>::new(
            layout,
            member,
            |_| {
                calls.set(calls.get() + 1);
                None
            },
            |_, _| {
                calls.set(calls.get() + 1);
            },
        )
    };
    for (l, name, kind) in [
        (layout::<Other, SimpleScore>(), "score", 0),
        (layout::<Domain, HardSoftScore>(), "score", 1),
        (layout::<Domain, SimpleScore>(), "absent", 2),
        (layout::<Domain, SimpleScore>(), "entities", 3),
    ] {
        let error = match build(l, name) {
            Err(e) => e,
            Ok(_) => panic!("bad metadata accepted"),
        };
        assert!(match kind {
            0 => matches!(error, SolutionScoreAccessError::SolutionType { .. }),
            1 => matches!(error, SolutionScoreAccessError::ScoreType { .. }),
            2 => matches!(error, SolutionScoreAccessError::MissingMember(_)),
            _ => matches!(error, SolutionScoreAccessError::NotScore(_)),
        });
        assert_eq!(calls.get(), 0);
    }
}
