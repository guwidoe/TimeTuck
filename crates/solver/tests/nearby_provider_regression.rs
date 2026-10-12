//! Original runtime proximity-provider checks; no upstream method credit.
use std::cell::{Cell, RefCell};
use timetuck::*;
struct Solution;
struct Row;
struct Meter;
struct Unknown;
fn schema(list: bool) -> PlanningSchema {
    PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Row>(vec![if list {
        PlanningVariableDefinition::list::<i32>("valueList", false)
    } else {
        PlanningVariableDefinition::basic::<i32>("value", false)
    }])])
    .unwrap()
}
fn config(list: bool) -> NearbyMoveConfig {
    NearbyMoveConfig::new(if list {
        NearbyMoveKind::ListChange
    } else {
        NearbyMoveKind::Change
    })
    .enable_nearby(PlanningType::of::<Meter>(), 0)
    .unwrap()
}
// Deliberately neither Clone nor Eq: each payload occurrence is moved once.
#[derive(Debug)]
struct Payload {
    distance: f64,
    occurrence: usize,
}
struct Child<'a> {
    reads: &'a Cell<usize>,
    opens: &'a Cell<usize>,
    events: &'a RefCell<Vec<MoveProviderEvent>>,
    infinite: bool,
}
impl MoveProvider<(), (), Payload> for Child<'_> {
    fn neighborhood_count(&self) -> Result<usize, NeighborhoodCountError> {
        Err(NeighborhoodCountError::WeightedUnion)
    }
    fn event(&mut self, e: MoveProviderEvent) {
        self.events.borrow_mut().push(e);
    }
    fn construction_placement_completed(&mut self, installed: bool) -> bool {
        !installed
    }
    fn moves<'a>(
        &'a mut self,
        _: &'a (),
        _: &'a (),
        step: u64,
    ) -> Box<dyn Iterator<Item = Payload> + 'a>
    where
        Payload: 'a,
    {
        self.opens.set(self.opens.get() + 1);
        let n = if self.infinite { usize::MAX } else { 3 };
        let reads = self.reads;
        Box::new((0..n).map(move |i| {
            reads.set(reads.get() + 1);
            Payload {
                distance: if i == 0 { 2.0 } else { 1.0 },
                occurrence: step as usize * 10 + i,
            }
        }))
    }
}
#[test]
fn owned_occurrences_rank_without_cloning_refresh_each_open_and_forward_hooks() {
    let reads = Cell::new(0);
    let opens = Cell::new(0);
    let metric = Cell::new(0);
    let events = RefCell::new(vec![]);
    let origins = RefCell::new(vec![]);
    let mut meters = NearbyDistanceRegistry::default();
    meters
        .register::<Meter>(|_: &u64, m: &Payload| {
            metric.set(metric.get() + 1);
            m.distance
        })
        .unwrap();
    let child = Child {
        reads: &reads,
        opens: &opens,
        events: &events,
        infinite: false,
    };
    let mut q = NearbyMoveProvider::new(
        child,
        |_: &(), _: &(), step| {
            origins.borrow_mut().push(step);
            step
        },
        &config(false),
        &schema(false),
        &meters,
        3,
    )
    .unwrap();
    assert_eq!(
        MoveProvider::<(), (), Payload>::neighborhood_count(&q),
        Err(NeighborhoodCountError::WeightedUnion)
    );
    use MoveProviderEvent::*;
    for e in [SolveStarted, PhaseStarted, StepStarted] {
        MoveProvider::<(), (), Payload>::event(&mut q, e);
    }
    for step in [0, 4] {
        let got: Vec<_> = q
            .try_moves(&(), &(), step)
            .unwrap()
            .map(|m| m.occurrence)
            .collect();
        assert_eq!(
            got,
            vec![
                step as usize * 10 + 1,
                step as usize * 10 + 2,
                step as usize * 10
            ]
        );
    }
    for e in [StepEnded, PhaseEnded, SolveEnded] {
        MoveProvider::<(), (), Payload>::event(&mut q, e);
    }
    assert_eq!(
        *events.borrow(),
        vec![
            SolveStarted,
            PhaseStarted,
            StepStarted,
            StepEnded,
            PhaseEnded,
            SolveEnded
        ]
    );
    assert_eq!(*origins.borrow(), vec![0, 4]);
    assert_eq!(opens.get(), 2);
    assert_eq!(reads.get(), 6);
    assert_eq!(metric.get(), 6);
    assert!(MoveProvider::<(), (), Payload>::construction_placement_completed(&mut q, false));
    assert!(!MoveProvider::<(), (), Payload>::construction_placement_completed(&mut q, true));
    assert_eq!(q.into_inner().opens.get(), 2);
}
#[test]
fn infinite_overflow_is_bounded_zero_disables_and_invalid_distance_can_be_retried() {
    let reads = Cell::new(0);
    let opens = Cell::new(0);
    let metric = Cell::new(0);
    let origin = Cell::new(0);
    let events = RefCell::new(vec![]);
    let invalid = Cell::new(false);
    let mut meters = NearbyDistanceRegistry::default();
    meters
        .register::<Meter>(|_: &(), m: &Payload| {
            metric.set(metric.get() + 1);
            if invalid.get() {
                f64::INFINITY
            } else {
                m.distance
            }
        })
        .unwrap();
    let child = Child {
        reads: &reads,
        opens: &opens,
        events: &events,
        infinite: true,
    };
    let mut q = NearbyMoveProvider::new(
        child,
        |_: &(), _: &(), _| {
            origin.set(origin.get() + 1);
        },
        &config(false),
        &schema(false),
        &meters,
        3,
    )
    .unwrap();
    assert!(matches!(
        q.try_moves(&(), &(), 0),
        Err(NearbyConfigError::CandidateLimit { count: 4, limit: 3 })
    ));
    assert_eq!(reads.get(), 4);
    assert_eq!(metric.get(), 0);
    q.provider_mut().infinite = false;
    invalid.set(true);
    assert!(matches!(
        q.try_moves(&(), &(), 1),
        Err(NearbyConfigError::InvalidDistance { candidate: 0 })
    ));
    invalid.set(false);
    assert_eq!(q.try_moves(&(), &(), 2).unwrap().count(), 3);
    let child = q.into_inner();
    let mut disabled = NearbyMoveProvider::new(
        child,
        |_: &(), _: &(), _| panic!("disabled origin"),
        &config(false),
        &schema(false),
        &meters,
        0,
    )
    .unwrap();
    assert_eq!(disabled.try_moves(&(), &(), 3).unwrap().count(), 0);
    assert_eq!(reads.get(), 10);
    assert_eq!(opens.get(), 3);
    assert_eq!(metric.get(), 4);
    assert_eq!(origin.get(), 3);
}
#[test]
fn binding_rejects_unknown_meter_union_missing_reference_and_mixed_basic_before_open() {
    let mut meters = NearbyDistanceRegistry::<(), i32>::default();
    meters.register::<Meter>(|_, _| 0.0).unwrap();
    let bind = |c: &NearbyMoveConfig, s: &PlanningSchema| {
        NearbyMoveProvider::new(
            |_: &(), _: &(), _: u64| vec![1],
            |_: &(), _: &(), _: u64| (),
            c,
            s,
            &meters,
            3,
        )
    };
    let mut c = config(false);
    c.selector_mut(NearbySelectorRole::Value)
        .nearby
        .as_mut()
        .unwrap()
        .meter = PlanningType::of::<Unknown>();
    assert!(matches!(
        bind(&c, &schema(false)),
        Err(NearbyConfigError::UnknownMeter(_))
    ));
    assert!(matches!(
        bind(&NearbyMoveConfig::union(vec![]), &schema(false)),
        Err(NearbyConfigError::Unsupported(NearbyMoveKind::Union))
    ));
    assert!(matches!(
        bind(
            &NearbyMoveConfig::new(NearbyMoveKind::Change),
            &schema(false)
        ),
        Err(NearbyConfigError::InvalidOrigin)
    ));
    let mixed = PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Row>(vec![
        PlanningVariableDefinition::basic::<i32>("x", false),
        PlanningVariableDefinition::list::<i32>("list", false),
    ])])
    .unwrap();
    assert!(matches!(
        bind(&config(false), &mixed),
        Err(NearbyConfigError::MixedModel(NearbyMoveKind::Change))
    ));
}
#[test]
fn nearby_basic_provider_optimizes_through_both_backends_and_returns_typed_engine_errors() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]).with_initial(0)]).unwrap();
    let s = schema(false);
    let c = config(false);
    let mut meters = NearbyDistanceRegistry::default();
    meters
        .register::<Meter>(|o: &i32, m: &Move<i32>| {
            let Move::Change { value: Some(v), .. } = m else {
                unreachable!()
            };
            f64::from((v - o).abs())
        })
        .unwrap();
    let mut n = ConstraintNetwork::new(1);
    n.add_constraint("value", vec![0], |v| {
        SimpleScore::new(i64::from(*v[0].unwrap()))
    })
    .unwrap();
    for stream in [false, true] {
        let origins = RefCell::new(vec![]);
        let mut q = NearbyMoveProvider::new(
            |p: &FiniteProblem<i32>, a: &Assignment<i32>, _| change_moves(p, a).unwrap().collect(),
            |_: &FiniteProblem<i32>, a: &Assignment<i32>, step| {
                origins.borrow_mut().push((step, a.values()[0].unwrap()));
                a.values()[0].unwrap()
            },
            &c,
            &s,
            &meters,
            2,
        )
        .unwrap();
        let cfg = LocalSearchConfig {
            max_steps: 4,
            assert_scores: true,
            ..Default::default()
        };
        let r = if stream {
            let mut b = StreamGraphBuilder::new();
            let src = b.source::<usize, (usize, Option<i32>)>();
            b.score("value", src.port(), |(_, v)| {
                SimpleScore::new(i64::from(v.unwrap()))
            })
            .unwrap();
            stream_local_search_from_with_provider(
                &p,
                b.build(),
                src,
                p.initial_assignment(),
                cfg,
                &mut q,
            )
            .unwrap()
        } else {
            local_search_from_with_provider(&p, &n, p.initial_assignment(), cfg, &mut q).unwrap()
        };
        let best = r.best.unwrap();
        assert_eq!(best.score, SimpleScore::new(2));
        assert_eq!(best.assignment.values(), &[Some(2)]);
        assert_eq!(origins.borrow()[0], (0, 0));
        assert!(origins.borrow().iter().any(|(_, v)| *v == 2));
    }
    let mut bad = NearbyDistanceRegistry::default();
    bad.register::<Meter>(|_: &i32, _: &Move<i32>| f64::NAN)
        .unwrap();
    let mut q = NearbyMoveProvider::new(
        |p: &FiniteProblem<i32>, a: &Assignment<i32>, _| change_moves(p, a).unwrap().collect(),
        |_: &FiniteProblem<i32>, a: &Assignment<i32>, _| a.values()[0].unwrap(),
        &c,
        &s,
        &bad,
        2,
    )
    .unwrap();
    let err = local_search_from_with_fallible_provider(
        &p,
        &n,
        p.initial_assignment(),
        LocalSearchConfig::default(),
        &mut q,
    )
    .unwrap_err();
    assert!(err.to_string().contains("InvalidDistance"));
    assert_eq!(p.initial_assignment().values(), &[Some(0)]);
}
#[test]
fn resolved_list_plan_feeds_nearby_real_list_search_on_both_backends() {
    let s = schema(true);
    let field = s
        .genuine_entity::<Row>()
        .unwrap()
        .genuine_variable("valueList")
        .unwrap();
    let p = ListProblem::new(
        vec![7],
        vec![
            ListEntity::new(vec![7]),
            ListEntity::new(vec![]),
            ListEntity::new(vec![]),
        ],
    )
    .unwrap();
    let mut n = ListConstraintNetwork::new(3, 1);
    n.add_constraint("owner", vec![ListScope::Value(0)], |f| {
        let ListFact::Value { element, .. } = &f[0] else {
            unreachable!()
        };
        SimpleScore::new(element.as_ref().unwrap().position.entity as i64)
    })
    .unwrap();
    let mut meters = NearbyDistanceRegistry::default();
    meters
        .register::<Meter>(|owner: &usize, m: &ListMove<i32>| {
            let ListMove::Change { destination, .. } = m else {
                unreachable!()
            };
            owner.abs_diff(destination.entity) as f64
        })
        .unwrap();
    for stream in [false, true] {
        let child = SchemaMoveConfig::new(SchemaMoveKind::Change)
            .resolve(&s)
            .unwrap()
            .bind(&[SchemaVariableSlots {
                variable: field,
                slots: vec![0, 1, 2],
            }])
            .unwrap();
        let origins = RefCell::new(vec![]);
        let mut q = NearbyMoveProvider::new(
            child,
            |_: &ListProblem<i32>, a: &ListAssignment<i32>, step| {
                let owner = a.location(&7).unwrap().entity;
                origins.borrow_mut().push((step, owner));
                owner
            },
            &config(true),
            &s,
            &meters,
            2,
        )
        .unwrap();
        let cfg = LocalSearchConfig {
            max_steps: 4,
            assert_scores: true,
            ..Default::default()
        };
        let r = if stream {
            let mut b = StreamGraphBuilder::new();
            let src = ListStreamSources::new(&mut b);
            b.score("owner", src.values.port(), |v| {
                SimpleScore::new(v.position.unwrap().entity as i64)
            })
            .unwrap();
            list_stream_local_search_from_with_provider(
                &p,
                b.build(),
                src,
                p.initial_assignment(),
                cfg,
                &mut q,
            )
            .unwrap()
        } else {
            list_local_search_from_with_provider(&p, &n, p.initial_assignment(), cfg, &mut q)
                .unwrap()
        };
        let best = r.best.unwrap();
        assert_eq!(best.score, SimpleScore::new(2));
        assert_eq!(best.assignment.lists(), &[vec![], vec![], vec![7]]);
        assert_eq!(origins.borrow()[0], (0, 0));
        assert!(origins.borrow().iter().any(|(_, v)| *v == 2));
    }
}
#[test]
fn resolved_metadata_preserves_each_list_field_identity_and_runtime_limits() {
    struct Other;
    let s = PlanningSchema::new::<Solution>(vec![
        PlanningEntityDefinition::of::<Row>(vec![
            PlanningVariableDefinition::basic::<i32>("basic", false),
            PlanningVariableDefinition::list::<i32>("values", false),
        ]),
        PlanningEntityDefinition::of::<Other>(vec![PlanningVariableDefinition::list::<i32>(
            "values", false,
        )]),
    ])
    .unwrap();
    let mut c = SchemaMoveConfig::new(SchemaMoveKind::Change);
    c.cache_scope = SelectionCacheScope::Phase;
    c.entity_comparator = Some(PlanningType::of::<Meter>());
    c.selected_count_limit = Some(1);
    let p = c.resolve(&s).unwrap();
    let selectors: Vec<_> = p.list_selectors().collect();
    assert_eq!(selectors.len(), 2);
    assert_ne!(selectors[0].value, selectors[1].value);
    for v in selectors {
        let d = v.destination.unwrap();
        assert_eq!(d.entity, v.value.entity().entity_type());
        assert_eq!(d.value, v.value);
        assert_eq!(d.entity_comparator, c.entity_comparator);
        assert_eq!(v.cache_scope, SelectionCacheScope::Phase);
        assert_eq!(v.selected_count_limit, Some(1));
    }
    c.entity = Some(PlanningType::of::<Row>());
    c.variables = vec!["values".into()];
    let field = s
        .genuine_entity::<Row>()
        .unwrap()
        .genuine_variable("values")
        .unwrap();
    let q = c
        .resolve(&s)
        .unwrap()
        .bind(&[SchemaVariableSlots {
            variable: field,
            slots: vec![0, 1],
        }])
        .unwrap();
    let list = ListProblem::new(
        vec![1, 2],
        vec![ListEntity::new(vec![1, 2]), ListEntity::new(vec![])],
    )
    .unwrap();
    assert_eq!(
        q.try_list_moves(&list, &list.initial_assignment(), 0)
            .unwrap()
            .count(),
        1
    );
}
