// Original factory/cache/resource regressions; no upstream-method credit.
use EnvironmentMode::{
    FullAssert, NoAssert, NonIntrusiveFullAssert, NonReproducible, PhaseAssert, StepAssert,
    TrackedFullAssert,
};
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;
fn model() -> Rc<FiniteProblem<i64>> {
    Rc::new(
        FiniteProblem::new(
            (0..7)
                .map(|i| Variable::new((0..5).collect()).with_initial(i % 5))
                .collect(),
        )
        .unwrap(),
    )
}
fn shape() -> Rc<FactoryScoreShape> {
    Rc::new(FactoryScoreShape::for_score::<SimpleScore>())
}
fn profile() -> EnvironmentProfile {
    EnvironmentProfile::new(Some(PhaseAssert), vec![None, Some(FullAssert)])
}
#[test]
fn shared_dense_network_creates_independent_live_directors_and_current_state_children() {
    let p = model();
    let mut network = ConstraintNetwork::new(7);
    for i in 0..7 {
        network
            .add_constraint(format!("weighted-{i}"), vec![i], move |rows| {
                SimpleScore::new((i as i64 + 1) * rows[0].copied().unwrap_or(0))
            })
            .unwrap();
    }
    let calls = Cell::new(0);
    let recipe = ScoringFactoryRecipe::incremental_recipe(
        |_| {
            calls.set(calls.get() + 1);
            Ok::<_, ScoringError>(&network)
        },
        |n: &&ConstraintNetwork<'_, i64, SimpleScore>, _, input| ScoreDirector::new(&p, n, input),
        |d: &ScoreDirector<'_, '_, i64, SimpleScore>| Ok(d.assignment().clone()),
    );
    let f = ScoringFactoryDeclaration::default()
        .with_scoring_recipe(recipe)
        .prepare_score_factory(p.clone(), shape(), profile())
        .unwrap();
    let mut first = f
        .score_factory_builder(p.initial_assignment())
        .instantiate_score_director()
        .unwrap();
    let second = f
        .score_factory_builder_in(p.initial_assignment(), FullAssert)
        .instantiate_score_director()
        .unwrap();
    assert!(
        first
            .scoring_factory()
            .same_prepared_factory(second.scoring_factory())
    );
    assert_eq!(calls.get(), 1);
    assert_eq!(first.factory_resource().score(), SimpleScore::new(47));
    first
        .factory_resource_mut()
        .apply(&Move::Change {
            variable: 0,
            value: Some(4),
        })
        .unwrap()
        .commit();
    let child = first.create_scoring_child().unwrap();
    assert_eq!(child.factory_resource().score(), SimpleScore::new(51));
    assert_eq!(second.factory_resource().score(), SimpleScore::new(47));
    first
        .factory_resource_mut()
        .apply(&Move::Change {
            variable: 0,
            value: Some(3),
        })
        .unwrap()
        .commit();
    assert_eq!(first.factory_resource().score(), SimpleScore::new(50));
    assert_eq!(child.factory_resource().score(), SimpleScore::new(51));
    assert_eq!(calls.get(), 1);
    first.factory_resource_mut().assert_consistent().unwrap();
}
struct CompiledWeights {
    weights: Rc<[i64]>,
    mode: EnvironmentMode,
}
#[test]
fn shared_stream_recipe_allocates_independent_indexes_and_preserves_child_mode() {
    let p = model();
    let calls = Cell::new(0);
    let recipe = ScoringFactoryRecipe::stream_recipe(
        |mode| {
            calls.set(calls.get() + 1);
            Ok::<_, StreamScoringError>(CompiledWeights {
                weights: (1..=7).collect::<Vec<_>>().into(),
                mode,
            })
        },
        |prepared: &CompiledWeights, settings, input| {
            assert_eq!(prepared.mode, settings.requested_mode);
            let mut b = StreamGraphBuilder::new();
            let source = b.source();
            let weights = prepared.weights.clone();
            b.score(
                "weighted",
                source.port(),
                move |(i, v): &(usize, Option<i64>)| SimpleScore::new(weights[*i] * v.unwrap_or(0)),
            )?;
            StreamScoreDirector::new(&p, b.build(), source, input)
        },
        |d: &StreamScoreDirector<'_, '_, i64, SimpleScore>| Ok(d.assignment().clone()),
    );
    let f = ScoringFactoryDeclaration::default()
        .with_scoring_recipe(recipe)
        .prepare_score_factory(p.clone(), shape(), profile())
        .unwrap();
    let mut first = f
        .score_factory_builder_in(p.initial_assignment(), FullAssert)
        .instantiate_score_director()
        .unwrap();
    let mut second = f
        .score_factory_builder_in(p.initial_assignment(), FullAssert)
        .instantiate_score_director()
        .unwrap();
    assert!(
        first
            .scoring_factory()
            .same_prepared_factory(second.scoring_factory())
    );
    assert!(!f.same_prepared_factory(first.scoring_factory()));
    assert_eq!(calls.get(), 2);
    assert_eq!(first.factory_resource().score(), SimpleScore::new(47));
    first
        .factory_resource_mut()
        .apply(&Move::Change {
            variable: 0,
            value: Some(4),
        })
        .unwrap()
        .commit();
    let mut child = first.create_scoring_child().unwrap();
    assert_eq!(child.factory_director_settings().requested_mode, FullAssert);
    assert_eq!(child.factory_resource().score(), SimpleScore::new(51));
    assert_eq!(second.factory_resource().score(), SimpleScore::new(47));
    {
        let trial = second
            .factory_resource_mut()
            .apply(&Move::Change {
                variable: 1,
                value: Some(4),
            })
            .unwrap();
        assert_eq!(trial.score(), SimpleScore::new(53));
    }
    assert_eq!(second.factory_resource().score(), SimpleScore::new(47));
    assert_eq!(first.factory_resource().score(), SimpleScore::new(51));
    assert_eq!(calls.get(), 2);
    assert_eq!(f.prepared_factory_count(), 2);
    child.factory_resource_mut().assert_consistent().unwrap();
}
struct ReportingCalculator {
    value: i64,
}
impl IncrementalCalculator<i64> for ReportingCalculator {
    type Score = SimpleScore;
    type Justification = ();
    type Change = ();
    fn supports_matches(&self) -> bool {
        true
    }
    fn reset(
        &mut self,
        input: &i64,
        registry: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.value = *input;
        if registry.is_enabled() {
            registry.register("value", SimpleScore::new(*input), ())?;
        }
        Ok(())
    }
    fn calculate_score(&mut self) -> SimpleScore {
        SimpleScore::new(self.value)
    }
}
#[test]
fn all_native_modes_reuse_incremental_preparation_and_forward_real_registry_policy() {
    let calls = Cell::new(0);
    let recipe = ScoringFactoryRecipe::incremental_recipe(
        |_| {
            calls.set(calls.get() + 1);
            Ok::<_, MatchRegistryError>(())
        },
        |_: &(), settings, input| {
            CalculatorSession::new(
                input,
                ReportingCalculator { value: 0 },
                settings.matching == ContextMatchPolicy::Enabled,
            )
        },
        |d: &CalculatorSession<i64, ReportingCalculator>| Ok(*d.solution()),
    );
    let f = ScoringFactoryDeclaration::default()
        .with_scoring_recipe(recipe)
        .prepare_score_factory(Rc::new(()), shape(), profile())
        .unwrap();
    for mode in [
        NonReproducible,
        NoAssert,
        PhaseAssert,
        StepAssert,
        NonIntrusiveFullAssert,
        FullAssert,
        TrackedFullAssert,
    ] {
        for policy in [ContextMatchPolicy::Disabled, ContextMatchPolicy::Enabled] {
            let mut d = f
                .score_factory_builder_in(9, mode)
                .with_factory_matching(policy)
                .instantiate_score_director()
                .unwrap();
            assert!(d.scoring_factory().same_prepared_factory(&f));
            assert_eq!(d.factory_director_settings().requested_mode, mode);
            assert_eq!(
                d.factory_resource().registry().unwrap().is_enabled(),
                policy == ContextMatchPolicy::Enabled
            );
            assert_eq!(
                d.factory_resource_mut().score().unwrap(),
                SimpleScore::new(9)
            );
            if policy == ContextMatchPolicy::Enabled {
                assert_eq!(
                    *d.factory_resource().registry().unwrap().total().unwrap(),
                    SimpleScore::new(9)
                );
            }
            let child = d.create_scoring_child().unwrap();
            assert_eq!(
                child.factory_director_settings(),
                d.factory_director_settings()
            );
            assert!(!std::ptr::eq(
                d.factory_resource(),
                child.factory_resource()
            ));
        }
    }
    assert_eq!(calls.get(), 1);
    assert_eq!(f.prepared_factory_count(), 1);
}
type SimpleRecipe<'a> = ScoringFactoryRecipe<'a, EnvironmentMode, i64, i64, String>;
fn simple_recipe(calls: &Cell<usize>) -> SimpleRecipe<'_> {
    ScoringFactoryRecipe::stream_recipe(
        |mode| {
            calls.set(calls.get() + 1);
            Ok(mode)
        },
        |_: &EnvironmentMode, _, input| Ok(input),
        |d: &i64| Ok(*d),
    )
}
#[test]
fn invalid_configuration_is_rejected_before_any_preparation_callback() {
    let calls = Cell::new(0);
    for variation in 0..7 {
        let mut d = ScoringFactoryDeclaration::default();
        d.recipes.push(simple_recipe(&calls));
        let mut profile = profile();
        let mut score_shape = shape();
        match variation {
            0 => d.recipes.clear(),
            1 => d.recipes.push(simple_recipe(&calls)),
            2 => d.trend = Some("invalid".into()),
            3 => {
                let mut a =
                    ScoringFactoryDeclaration::default().with_scoring_recipe(simple_recipe(&calls));
                a.assertion = Some(Box::new(
                    ScoringFactoryDeclaration::default().with_scoring_recipe(simple_recipe(&calls)),
                ));
                d.assertion = Some(Box::new(a));
                profile = EnvironmentProfile::new(Some(StepAssert), vec![]);
            }
            4 => {
                d.assertion = Some(Box::new(
                    ScoringFactoryDeclaration::default().with_scoring_recipe(simple_recipe(&calls)),
                ))
            }
            5 => profile = EnvironmentProfile::new(Some(NonReproducible), vec![Some(FullAssert)]),
            6 => score_shape = Rc::new(FactoryScoreShape::for_score::<BendableScore<0, 0>>()),
            _ => unreachable!(),
        }
        assert!(
            d.prepare_score_factory(Rc::new(()), score_shape, profile)
                .is_err()
        );
        assert_eq!(calls.get(), 0);
    }
}
#[test]
fn failed_or_panicked_preparation_can_retry_and_failed_instantiation_keeps_cache() {
    let stage = Cell::new(0);
    let calls = Cell::new(0);
    let builds = Cell::new(0);
    let recipe = ScoringFactoryRecipe::stream_recipe(
        |mode| {
            calls.set(calls.get() + 1);
            if mode == FullAssert {
                match stage.get() {
                    0 => return Err("prepare".to_string()),
                    1 => panic!("prepare"),
                    _ => {}
                }
            }
            Ok(mode)
        },
        |_: &EnvironmentMode, _, input: i64| {
            builds.set(builds.get() + 1);
            if input < 0 {
                Err("build".into())
            } else {
                Ok(input)
            }
        },
        |d: &i64| {
            if *d == 99 {
                Err("child".into())
            } else {
                Ok(*d)
            }
        },
    );
    let f = ScoringFactoryDeclaration::default()
        .with_scoring_recipe(recipe)
        .prepare_score_factory(Rc::new(()), shape(), profile())
        .unwrap();
    assert!(matches!(
        f.select_scoring_factory(FullAssert),
        Err(ScoreFactoryBuildError::Preparation { .. })
    ));
    assert_eq!(f.prepared_factory_count(), 1);
    stage.set(1);
    assert!(catch_unwind(AssertUnwindSafe(|| f.select_scoring_factory(FullAssert))).is_err());
    assert_eq!(f.prepared_factory_count(), 1);
    stage.set(2);
    let other = f.select_scoring_factory(FullAssert).unwrap();
    assert_eq!(*other.factory_preparation(), FullAssert);
    assert!(matches!(
        other.score_factory_builder(-1).instantiate_score_director(),
        Err(ScoreFactoryBuildError::Instantiation { .. })
    ));
    assert_eq!(f.prepared_factory_count(), 2);
    let calls_before = calls.get();
    let d = other
        .score_factory_builder(99)
        .instantiate_score_director()
        .unwrap();
    assert!(matches!(
        d.create_scoring_child(),
        Err(ScoreFactoryBuildError::ChildInput(_))
    ));
    assert_eq!(calls.get(), calls_before);
    assert_eq!(builds.get(), 2);
    let _good = f
        .score_factory_builder(3)
        .instantiate_score_director()
        .unwrap();
    assert_eq!(calls.get(), calls_before);
}
#[test]
fn reentrant_same_mode_preparation_returns_typed_error_without_refcell_panic() {
    type Factory = PreparedScoreFactory<'static, (), EnvironmentMode, i64, i64, String>;
    let slot: Rc<RefCell<Option<Factory>>> = Rc::new(RefCell::new(None));
    let weak = Rc::downgrade(&slot);
    let observed = Rc::new(Cell::new(false));
    let copy = observed.clone();
    let recipe = ScoringFactoryRecipe::stream_recipe(
        move |mode| {
            if mode == FullAssert {
                let slot = weak.upgrade().unwrap();
                let f = slot.borrow().as_ref().unwrap().clone();
                copy.set(matches!(
                    f.select_scoring_factory(mode),
                    Err(ScoreFactoryBuildError::RecursivePreparation { mode: FullAssert })
                ));
            }
            Ok(mode)
        },
        |_: &EnvironmentMode, _, input| Ok(input),
        |d: &i64| Ok(*d),
    );
    let f = ScoringFactoryDeclaration::default()
        .with_scoring_recipe(recipe)
        .prepare_score_factory(Rc::new(()), shape(), profile())
        .unwrap();
    *slot.borrow_mut() = Some(f.clone());
    let prepared = f.select_scoring_factory(FullAssert).unwrap();
    assert!(observed.get());
    assert_eq!(prepared.prepared_factory_count(), 2);
    slot.borrow_mut().take();
}
struct DropProbe {
    events: Rc<RefCell<Vec<&'static str>>>,
    name: &'static str,
}
impl Drop for DropProbe {
    fn drop(&mut self) {
        self.events.borrow_mut().push(self.name);
    }
}
#[test]
fn directors_release_before_their_last_nonclone_preparation_owner() {
    let events = Rc::new(RefCell::new(vec![]));
    let prepare_events = events.clone();
    let runtime_events = events.clone();
    let recipe = ScoringFactoryRecipe::full_evaluation_recipe(
        move |_| {
            Ok::<_, String>(DropProbe {
                events: prepare_events.clone(),
                name: "preparation",
            })
        },
        move |_: &DropProbe, _, _: ()| {
            Ok(DropProbe {
                events: runtime_events.clone(),
                name: "director",
            })
        },
        |_: &DropProbe| Ok(()),
    );
    let f = ScoringFactoryDeclaration::default()
        .with_scoring_recipe(recipe)
        .prepare_score_factory(Rc::new(()), shape(), EnvironmentProfile::default())
        .unwrap();
    let d = f
        .score_factory_builder(())
        .instantiate_score_director()
        .unwrap();
    drop(f);
    assert!(events.borrow().is_empty());
    drop(d);
    assert_eq!(*events.borrow(), vec!["director", "preparation"]);
}
#[test]
fn retained_mode_views_keep_model_shape_trend_and_independent_assertion_recipe() {
    let main_calls = Cell::new(0);
    let assertion_calls = Cell::new(0);
    let mut config =
        ScoringFactoryDeclaration::default().with_scoring_recipe(simple_recipe(&main_calls));
    config.trend = Some("ONLY_DOWN".into());
    let a = ScoringFactoryDeclaration::default().with_scoring_recipe(
        ScoringFactoryRecipe::incremental_recipe(
            |mode| {
                assertion_calls.set(assertion_calls.get() + 1);
                Ok::<_, String>(mode)
            },
            |_: &EnvironmentMode, settings, _: i64| Ok(settings.requested_mode as i64),
            |d: &i64| Ok(*d),
        ),
    );
    config.assertion = Some(Box::new(a));
    let model = Rc::new(());
    let score_shape = shape();
    let f = config
        .prepare_score_factory(
            model.clone(),
            score_shape.clone(),
            EnvironmentProfile::new(Some(StepAssert), vec![Some(FullAssert)]),
        )
        .unwrap();
    let other = f.select_scoring_factory(FullAssert).unwrap();
    let a = other.assertion_score_factory().unwrap();
    assert_eq!(*a.factory_preparation(), NonReproducible);
    assert_eq!(
        a.scoring_factory_strategy(),
        ScoringFactoryStrategy::Incremental
    );
    assert!(Rc::ptr_eq(other.factory_model(), &model));
    assert!(Rc::ptr_eq(other.factory_score_shape(), &score_shape));
    assert_eq!(
        *other.factory_score_trend(),
        InitializingScoreTrend::parse("ONLY_DOWN", 1).unwrap()
    );
    assert!(a.same_prepared_factory(f.assertion_score_factory().unwrap()));
    assert_eq!(main_calls.get(), 2);
    assert_eq!(assertion_calls.get(), 1);
}
