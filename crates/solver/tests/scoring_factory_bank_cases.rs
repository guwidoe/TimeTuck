// Adapted from pinned Timefold Community ScoreDirectorFactoryFactoryTest,
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
#[path = "fixtures/scoring_factory_bank_fixture.rs"]
mod fixture;
use EnvironmentMode::{FullAssert, NoAssert, NonReproducible, PhaseAssert, StepAssert};
use ScoringFactoryStrategy::{FullEvaluation, Incremental, MultiModeStream, Stream};
use fixture::*;
use std::{cell::Cell, rc::Rc};
use timetuck::*;

#[test]
fn easy_score_calculator() {
    let p = model();
    let f = factory(&p, FullEvaluation, PhaseAssert, vec![]);
    assert_eq!(f.scoring_factory_strategy(), FullEvaluation);
    let mut d = f
        .score_factory_builder(p.initial_assignment())
        .instantiate_score_director()
        .unwrap();
    assert!(matches!(d.factory_resource(), Resource::Full { .. }));
    assert_eq!(d.factory_resource_mut().actual_score(), SimpleScore::ZERO);
}
#[test]
fn incremental_score_calculator() {
    let p = model();
    let f = factory(&p, Incremental, PhaseAssert, vec![]);
    assert_eq!(f.scoring_factory_strategy(), Incremental);
    let mut d = f
        .score_factory_builder(p.initial_assignment())
        .instantiate_score_director()
        .unwrap();
    assert!(matches!(d.factory_resource(), Resource::Incremental { .. }));
    assert_eq!(d.factory_resource_mut().actual_score(), SimpleScore::ZERO);
}
#[test]
fn constraint_streams() {
    let p = model();
    let f = factory(&p, Stream, PhaseAssert, vec![]);
    assert_eq!(f.scoring_factory_strategy(), Stream);
    let mut d = f
        .score_factory_builder(p.initial_assignment())
        .instantiate_score_director()
        .unwrap();
    assert!(matches!(d.factory_resource(), Resource::Stream { .. }));
    assert_eq!(d.factory_resource_mut().actual_score(), SimpleScore::ZERO);
}
#[test]
fn factory_is_shared_with_the_score_directors_it_builds() {
    let p = model();
    let f = factory(&p, Incremental, PhaseAssert, vec![]);
    let d = f
        .score_factory_builder(p.initial_assignment())
        .instantiate_score_director()
        .unwrap();
    assert!(f.same_prepared_factory(d.scoring_factory()));
    assert!(Rc::ptr_eq(
        f.factory_model(),
        d.scoring_factory().factory_model()
    ));
    assert!(Rc::ptr_eq(
        f.factory_score_shape(),
        d.scoring_factory().factory_score_shape()
    ));
}
#[test]
fn no_score_calculation_throws_exception() {
    let p = model();
    let e = Declaration::default()
        .prepare_score_factory(p, shape(), EnvironmentProfile::default())
        .err()
        .unwrap();
    assert!(matches!(e, ScoreFactoryBuildError::MissingRecipe));
    assert!(e.to_string().contains("lacks configuration"));
}
#[test]
fn solver_config_without_score_director_factory_throws_exception() {
    let p = model();
    let phases = EnvironmentProfile::new(Some(PhaseAssert), vec![None, None]);
    let e = Declaration::default()
        .prepare_score_factory(p, shape(), phases)
        .err()
        .unwrap();
    assert!(matches!(e, ScoreFactoryBuildError::MissingRecipe));
    assert!(e.to_string().contains("lacks configuration"));
}
fn conflicts(first: ScoringFactoryStrategy) {
    let p = model();
    let config = declaration(&p, Stream).with_scoring_recipe(recipe(
        &p,
        first,
        Properties::default(),
        Rc::new(Cell::new(0)),
    ));
    let e = config
        .prepare_score_factory(p.clone(), shape(), EnvironmentProfile::default())
        .err()
        .unwrap();
    assert!(matches!(e, ScoreFactoryBuildError::ConflictingRecipes));
    assert!(e.to_string().contains("scoring factory"));
    assert!(e.to_string().contains("together"));
}
#[test]
fn multiple_score_calculations_throws_exception() {
    conflicts(FullEvaluation);
}
#[test]
fn incremental_multiple_score_calculations_throws_exception() {
    conflicts(Incremental);
}
#[test]
fn no_phase_override_yields_the_concrete_factory() {
    let p = model();
    let f = factory(&p, Stream, PhaseAssert, vec![None, None]);
    assert_eq!(f.scoring_factory_strategy(), Stream);
    assert_opens(&f, p.initial_assignment());
}
#[test]
fn one_overriding_phase_yields_the_multi_environment_factory() {
    let p = model();
    let f = factory(&p, Stream, PhaseAssert, vec![None, Some(FullAssert)]);
    assert_eq!(f.scoring_factory_strategy(), MultiModeStream);
    assert_opens(&f, p.initial_assignment());
}
#[test]
fn every_phase_overriding_yields_the_multi_environment_factory() {
    let p = model();
    let f = factory(
        &p,
        Stream,
        PhaseAssert,
        vec![Some(FullAssert), Some(FullAssert)],
    );
    assert_eq!(f.scoring_factory_strategy(), MultiModeStream);
    assert_opens(&f, p.initial_assignment());
}
#[test]
fn phases_restating_the_global_mode_yield_the_concrete_factory() {
    let p = model();
    let f = factory(
        &p,
        Stream,
        PhaseAssert,
        vec![Some(PhaseAssert), Some(PhaseAssert)],
    );
    assert_eq!(f.scoring_factory_strategy(), Stream);
    assert_opens(&f, p.initial_assignment());
}
#[test]
fn multi_environment_factory_carries_the_configured_trend() {
    let p = model();
    let mut config = declaration(&p, Stream);
    config.trend = Some("ONLY_DOWN".into());
    let f = config
        .prepare_score_factory(
            p.clone(),
            shape(),
            EnvironmentProfile::new(Some(PhaseAssert), vec![Some(FullAssert)]),
        )
        .unwrap();
    assert_eq!(f.scoring_factory_strategy(), MultiModeStream);
    assert_eq!(
        *f.factory_score_trend(),
        InitializingScoreTrend::parse("ONLY_DOWN", 1).unwrap()
    );
    assert_opens(&f, p.initial_assignment());
}
#[test]
fn multi_environment_factory_carries_the_configured_assertion_factory() {
    let p = model();
    let mut config = declaration(&p, Stream);
    config.assertion = Some(Box::new(declaration(&p, Incremental)));
    let f = config
        .prepare_score_factory(
            p.clone(),
            shape(),
            EnvironmentProfile::new(Some(StepAssert), vec![Some(FullAssert)]),
        )
        .unwrap();
    assert_eq!(f.scoring_factory_strategy(), MultiModeStream);
    let a = f.assertion_score_factory().unwrap();
    assert_eq!(a.scoring_factory_strategy(), Incremental);
    assert_opens(a, p.initial_assignment());
}
#[test]
fn easy_score_calculator_serves_every_environment_mode_itself() {
    let p = model();
    let f = factory(
        &p,
        FullEvaluation,
        PhaseAssert,
        vec![None, Some(FullAssert)],
    );
    assert_eq!(f.scoring_factory_strategy(), FullEvaluation);
    assert_opens(&f, p.initial_assignment());
}
#[test]
fn incremental_score_calculator_serves_every_environment_mode_itself() {
    let p = model();
    let f = factory(&p, Incremental, PhaseAssert, vec![None, Some(FullAssert)]);
    assert_eq!(f.scoring_factory_strategy(), Incremental);
    assert_opens(&f, p.initial_assignment());
}
#[test]
fn a_factory_which_serves_every_mode_itself_still_honours_the_requested_mode() {
    let p = model();
    let f = factory(
        &p,
        FullEvaluation,
        PhaseAssert,
        vec![None, Some(FullAssert)],
    );
    let d = f
        .score_factory_builder_in(p.initial_assignment(), FullAssert)
        .instantiate_score_director()
        .unwrap();
    assert_eq!(d.factory_director_settings().requested_mode, FullAssert);
    assert_eq!(
        d.factory_resource().resource_settings().requested_mode,
        FullAssert
    );
    assert!(f.same_prepared_factory(d.scoring_factory()));
}
#[test]
fn global_environment_mode_is_served_by_the_factory_itself() {
    let p = model();
    let f = factory(&p, Stream, PhaseAssert, vec![None, Some(FullAssert)]);
    let d = f
        .score_factory_builder_in(p.initial_assignment(), PhaseAssert)
        .instantiate_score_director()
        .unwrap();
    assert_eq!(d.factory_director_settings().requested_mode, PhaseAssert);
    assert!(f.same_prepared_factory(d.scoring_factory()));
}
#[test]
fn other_environment_mode_gets_a_factory_of_its_own() {
    let p = model();
    let f = factory(&p, Stream, PhaseAssert, vec![None, Some(FullAssert)]);
    let d = f
        .score_factory_builder_in(p.initial_assignment(), FullAssert)
        .instantiate_score_director()
        .unwrap();
    assert_eq!(d.factory_director_settings().requested_mode, FullAssert);
    assert!(!f.same_prepared_factory(d.scoring_factory()));
    assert_eq!(d.scoring_factory().scoring_factory_strategy(), Stream);
}
#[test]
fn the_factory_for_another_environment_mode_is_built_once_and_shared() {
    let p = model();
    let calls = Rc::new(Cell::new(0));
    let config = Declaration::default().with_scoring_recipe(recipe(
        &p,
        Stream,
        Properties::default(),
        calls.clone(),
    ));
    let f = config
        .prepare_score_factory(
            p.clone(),
            shape(),
            EnvironmentProfile::new(Some(PhaseAssert), vec![None, Some(FullAssert)]),
        )
        .unwrap();
    let first = f
        .score_factory_builder_in(p.initial_assignment(), FullAssert)
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
    assert!(!std::ptr::eq(
        first.factory_resource(),
        second.factory_resource()
    ));
    assert_eq!(calls.get(), 2);
}
#[test]
fn child_thread_score_director_keeps_the_requested_environment_mode() {
    let p = model();
    let f = factory(&p, Stream, PhaseAssert, vec![None, Some(FullAssert)]);
    let d = f
        .score_factory_builder_in(p.initial_assignment(), FullAssert)
        .instantiate_score_director()
        .unwrap();
    // The native child operation does not branch by Java thread role. Each role
    // maps to this same independent resource/working-input copy, not a new thread.
    let child = d.create_scoring_child().unwrap();
    assert_eq!(child.factory_director_settings().requested_mode, FullAssert);
    assert_eq!(
        child.factory_resource().resource_settings().requested_mode,
        FullAssert
    );
    assert_eq!(
        child.factory_resource().working_input(),
        d.factory_resource().working_input()
    );
    assert_eq!(
        child.factory_resource().working_input().values(),
        &[
            Some(0),
            Some(1),
            Some(2),
            Some(3),
            Some(4),
            Some(0),
            Some(1)
        ]
    );
}
#[test]
fn constraint_match_disabled_unless_requested() {
    let p = model();
    let f = factory(&p, Stream, FullAssert, vec![]);
    let d = f
        .score_factory_builder(p.initial_assignment())
        .instantiate_score_director()
        .unwrap();
    assert_eq!(
        d.factory_director_settings().matching,
        ContextMatchPolicy::Disabled
    );
    assert_eq!(
        d.factory_resource().resource_settings().matching,
        ContextMatchPolicy::Disabled
    );
}
#[test]
fn constraint_match_enabled_by_environment_mode() {
    assert_eq!(
        decide_score_factory_matching(PhaseAssert, ContextMetrics::default()),
        ContextMatchPolicy::Disabled
    );
    assert_eq!(
        decide_score_factory_matching(FullAssert, ContextMetrics::default()),
        ContextMatchPolicy::Enabled
    );
}
#[test]
fn constraint_match_applied_to_the_builder_reaches_the_score_director() {
    let p = model();
    let f = factory(&p, Stream, PhaseAssert, vec![]);
    let policy = decide_score_factory_matching(FullAssert, ContextMetrics::default());
    let d = f
        .score_factory_builder_in(p.initial_assignment(), FullAssert)
        .with_factory_matching(policy)
        .instantiate_score_director()
        .unwrap();
    assert_eq!(
        d.factory_director_settings().matching,
        ContextMatchPolicy::Enabled
    );
    assert_eq!(
        d.factory_resource().resource_settings().matching,
        ContextMatchPolicy::Enabled
    );
}
#[test]
fn constraint_match_enabled_by_metric() {
    assert_eq!(
        decide_score_factory_matching(
            NoAssert,
            ContextMetrics {
                best_constraint_totals: true,
                ..Default::default()
            }
        ),
        ContextMatchPolicy::Enabled
    );
}
#[test]
fn constraint_match_not_enabled_by_other_metric() {
    assert_eq!(
        decide_score_factory_matching(
            NoAssert,
            ContextMetrics {
                best_score: true,
                ..Default::default()
            }
        ),
        ContextMatchPolicy::Disabled
    );
}
#[test]
fn incremental_score_calculator_with_custom_properties() {
    let p = model();
    let props = Properties {
        string: "string 1".into(),
        number: 7,
    };
    let config = Declaration::default().with_scoring_recipe(recipe(
        &p,
        Incremental,
        props,
        Rc::new(Cell::new(0)),
    ));
    let f = config
        .prepare_score_factory(p.clone(), shape(), EnvironmentProfile::default())
        .unwrap();
    let d = f
        .score_factory_builder(p.initial_assignment())
        .instantiate_score_director()
        .unwrap();
    assert_eq!(d.factory_resource().source_properties().string, "string 1");
    assert_eq!(d.factory_resource().source_properties().number, 7);
}
#[test]
fn initializing_score_trend_from_config() {
    let p = model();
    let mut config = declaration(&p, Incremental);
    config.trend = Some("ONLY_DOWN".into());
    let f = config
        .prepare_score_factory(p.clone(), shape(), EnvironmentProfile::default())
        .unwrap();
    assert_eq!(
        *f.factory_score_trend(),
        InitializingScoreTrend::parse("ONLY_DOWN", 1).unwrap()
    );
    assert_opens(&f, p.initial_assignment());
}
#[test]
fn initializing_score_trend_defaults_to_any() {
    let p = model();
    let f = factory(&p, Incremental, PhaseAssert, vec![]);
    assert_eq!(
        *f.factory_score_trend(),
        InitializingScoreTrend::parse("ANY", 1).unwrap()
    );
    assert_opens(&f, p.initial_assignment());
}
#[test]
fn build_with_assertion_score_director_factory() {
    let p = model();
    let mut config = declaration(&p, Incremental);
    config.assertion = Some(Box::new(declaration(&p, Incremental)));
    let f = config
        .prepare_score_factory(
            p.clone(),
            shape(),
            EnvironmentProfile::new(Some(StepAssert), vec![]),
        )
        .unwrap();
    let a = f.assertion_score_factory().unwrap();
    assert_eq!(a.scoring_factory_strategy(), Incremental);
    let d = a
        .score_factory_builder(p.initial_assignment())
        .instantiate_score_director()
        .unwrap();
    assert_eq!(
        d.factory_director_settings().requested_mode,
        NonReproducible
    );
    assert!(matches!(d.factory_resource(), Resource::Incremental { .. }));
}
#[test]
fn nested_assertion_score_director_factory_throws_exception() {
    let p = model();
    let mut config = declaration(&p, Incremental);
    let mut nested = declaration(&p, Incremental);
    nested.assertion = Some(Box::new(declaration(&p, Incremental)));
    config.assertion = Some(Box::new(nested));
    let e = config
        .prepare_score_factory(
            p.clone(),
            shape(),
            EnvironmentProfile::new(Some(StepAssert), vec![]),
        )
        .err()
        .unwrap();
    assert!(matches!(e, ScoreFactoryBuildError::NestedAssertion));
    assert!(
        e.to_string()
            .contains("cannot have a non-null assertion scoring factory")
    );
}
#[test]
fn assertion_score_director_factory_in_lenient_environment_mode_throws_exception() {
    let p = model();
    let mut config = declaration(&p, Incremental);
    config.assertion = Some(Box::new(declaration(&p, Incremental)));
    let e = config
        .prepare_score_factory(
            p.clone(),
            shape(),
            EnvironmentProfile::new(Some(PhaseAssert), vec![]),
        )
        .err()
        .unwrap();
    assert!(matches!(
        e,
        ScoreFactoryBuildError::AssertionRequiresStepAssert
    ));
    assert!(e.to_string().contains("requires an environmentMode"));
    assert!(e.to_string().contains("StepAssert"));
}
