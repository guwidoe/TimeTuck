use std::{
    cell::{Cell, RefCell},
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    rc::Rc,
};
use timetuck::*;
struct Solution;
fn schema() -> PlanningSchema {
    PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<usize>(vec![
        PlanningVariableDefinition::basic::<i32>("first", false),
        PlanningVariableDefinition::basic::<i32>("second", false),
    ])])
    .unwrap()
}
#[test]
fn span_resolves_actual_list_geometry_executes_and_rejects_missing_or_overflow() {
    let p = ListProblem::new(
        (0..8).collect(),
        vec![ListEntity::new((0..8).collect()), ListEntity::new(vec![])],
    )
    .unwrap();
    let mut a = p.initial_assignment();
    let span = SubListSpan::new("A", 2, 5).unwrap();
    let movement = span
        .change_move(
            |name| (*name == "A").then_some(0),
            ListPosition {
                entity: 1,
                index: 0,
            },
            true,
        )
        .unwrap();
    {
        let trial = p.apply(&mut a, &movement).unwrap();
        assert_eq!(trial.lists(), &[vec![0, 1, 7], vec![6, 5, 4, 3, 2]]);
    }
    assert_eq!(a, p.initial_assignment());
    p.apply(&mut a, &movement).unwrap().commit();
    assert_eq!(a.lists(), &[vec![0, 1, 7], vec![6, 5, 4, 3, 2]]);
    assert_eq!(span.to_string(), "A[2..7]");
    assert_eq!(
        span.resolve(|_| None).unwrap_err(),
        SelectionIdentityError::UnknownEntity
    );
    assert_eq!(
        SubListSpan::new("A", usize::MAX, 1).unwrap_err(),
        SelectionIdentityError::SpanOverflow
    );
    let clone = span.clone();
    let hash = |s: &SubListSpan<&str>| {
        let mut h = DefaultHasher::new();
        s.hash(&mut h);
        h.finish()
    };
    assert_eq!(hash(&span), hash(&clone));
}
#[test]
fn live_demand_groups_bound_fields_executes_and_refreshes_sources_filters_and_values() {
    let s = schema();
    let fields = s
        .entity::<usize>()
        .unwrap()
        .genuine_variables()
        .collect::<Vec<_>>();
    let entities = Rc::new(RefCell::new(vec![0usize, 1, 2, 3]));
    let reader = entities.clone();
    let source = PillarEntitySource::new(move || reader.borrow().clone());
    let ceiling = Rc::new(Cell::new(3));
    let current = ceiling.clone();
    let filter = PillarEntityFilter::new(move |entity: &usize| *entity < current.get());
    let demand = PillarSelectionDemand::new(
        source.clone(),
        filter.clone(),
        fields.clone(),
        PillarSelectionPolicy::whole(),
    )
    .unwrap();
    let p = FiniteProblem::new(
        [1, 8, 1, 8, 2, 9, 2, 9]
            .into_iter()
            .map(|v| Variable::new(vec![1, 2, 8, 9]).with_initial(v))
            .collect(),
    )
    .unwrap();
    let mut a = p.initial_assignment();
    let bind =
        |e: &usize, v: PlanningVariableRef<'_>| Some(e * 2 + usize::from(v.name() == "second"));
    let groups = demand.prepare(&p, &a, bind).unwrap();
    assert_eq!(
        groups.iter().map(|g| g.rows().to_vec()).collect::<Vec<_>>(),
        vec![vec![vec![0, 1], vec![2, 3]], vec![vec![4, 5]]]
    );
    let change = groups[0].change_move(&a, &[Some(2), Some(9)]).unwrap();
    p.apply(&mut a, &change).unwrap().commit();
    assert_eq!(demand.prepare(&p, &a, bind).unwrap().len(), 1);
    ceiling.set(4);
    let regrouped = demand.prepare(&p, &a, bind).unwrap();
    assert_eq!(regrouped[0].rows().len(), 4);
    entities.borrow_mut().retain(|e| *e != 1);
    assert_eq!(demand.prepare(&p, &a, bind).unwrap()[0].rows().len(), 3);
    let copied = PillarSelectionDemand::new(
        source.clone(),
        filter.clone(),
        fields.clone(),
        PillarSelectionPolicy::whole(),
    )
    .unwrap();
    assert_eq!(demand, copied);
    let changed_policy = PillarSelectionDemand::new(
        source.clone(),
        filter.clone(),
        fields.clone(),
        PillarSelectionPolicy::subsets(1, 1).unwrap(),
    )
    .unwrap();
    assert_ne!(demand, changed_policy);
    assert_eq!(changed_policy.prepare(&p, &a, bind).unwrap().len(), 3);
    let other_source = PillarSelectionDemand::new(
        PillarEntitySource::new(|| vec![0usize]),
        filter.clone(),
        fields.clone(),
        PillarSelectionPolicy::whole(),
    )
    .unwrap();
    assert_ne!(demand, other_source);
    let other_filter = PillarSelectionDemand::new(
        source,
        PillarEntityFilter::new(|_: &usize| true),
        fields,
        PillarSelectionPolicy::whole(),
    )
    .unwrap();
    assert_ne!(demand, other_filter);
}
#[test]
fn invalid_metadata_and_binding_fail_without_changing_assignment() {
    let s = schema();
    let other = schema();
    let first = s.entity::<usize>().unwrap().variable("first").unwrap();
    let second = s.entity::<usize>().unwrap().variable("second").unwrap();
    let foreign = other.entity::<usize>().unwrap().variable("first").unwrap();
    let source = PillarEntitySource::new(|| vec![0usize, 1]);
    let filter = PillarEntityFilter::new(|_: &usize| true);
    assert!(
        PillarSelectionDemand::new(
            source.clone(),
            filter.clone(),
            vec![],
            PillarSelectionPolicy::whole()
        )
        .is_err()
    );
    assert!(
        PillarSelectionDemand::new(
            source.clone(),
            filter.clone(),
            vec![first, first],
            PillarSelectionPolicy::whole()
        )
        .is_err()
    );
    assert!(
        PillarSelectionDemand::new(
            source.clone(),
            filter.clone(),
            vec![first, foreign],
            PillarSelectionPolicy::whole()
        )
        .is_err()
    );
    let demand = PillarSelectionDemand::new(
        source.clone(),
        filter.clone(),
        vec![first, second],
        PillarSelectionPolicy::whole(),
    )
    .unwrap();
    let p = FiniteProblem::new(vec![Variable::new(vec![1]).with_initial(1); 4]).unwrap();
    let a = p.initial_assignment();
    let before = a.clone();
    assert!(demand.prepare(&p, &a, |_, _| None).is_err());
    assert!(demand.prepare(&p, &a, |_, _| Some(0)).is_err());
    assert_eq!(a, before);
    let none = PillarSelectionDemand::new(
        source,
        PillarEntityFilter::new(|_: &usize| false),
        vec![first],
        PillarSelectionPolicy::whole(),
    )
    .unwrap();
    assert!(
        none.prepare(&p, &a, |_, _| panic!("excluded rows must not bind"))
            .unwrap()
            .is_empty()
    );
}
