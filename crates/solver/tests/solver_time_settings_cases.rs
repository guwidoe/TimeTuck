// Adapted from pinned Community SolverConfigTest, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
#[path = "fixtures/scoring_factory_bank_fixture.rs"]
#[allow(dead_code)]
mod fixture;
use fixture::*;
use std::{cell::Cell, rc::Rc, time::Duration};
use timetuck::*;

#[test]
fn with_termination_spent_limit() {
    let config = SolverConfig::<SimpleScore>::default();
    assert_eq!(config.limits.max_time, None);
    let duration = Duration::from_secs(2 * 60);
    let config = config.with_time_limit(Some(duration));
    assert_eq!(config.limits.max_time, Some(duration));
}

#[test]
fn with_termination_unimproved_spent_limit() {
    let config = SolverConfig::<SimpleScore>::default();
    assert_eq!(config.limits.max_unimproved_time, None);
    let duration = Duration::from_secs(2 * 60);
    let config = config.with_unimproved_time_limit(Some(duration));
    assert_eq!(config.limits.max_unimproved_time, Some(duration));
}

fn registered_recipe(strategy: ScoringFactoryStrategy) {
    let problem = model();
    let declaration = Declaration::default();
    assert!(declaration.recipes.is_empty());
    let preparations = Rc::new(Cell::new(0));
    let declaration = declaration.with_scoring_recipe(recipe(
        &problem,
        strategy,
        Properties::default(),
        preparations.clone(),
    ));
    assert_eq!(declaration.recipes.len(), 1);
    let factory = declaration
        .prepare_score_factory(problem.clone(), shape(), EnvironmentProfile::default())
        .unwrap();
    assert_eq!(factory.scoring_factory_strategy(), strategy);
    let mut director = factory
        .score_factory_builder(problem.initial_assignment())
        .instantiate_score_director()
        .unwrap();
    assert_eq!(preparations.get(), 1);
    assert_eq!(
        director.factory_resource_mut().actual_score(),
        SimpleScore::ZERO
    );
    match strategy {
        ScoringFactoryStrategy::FullEvaluation => {
            assert!(matches!(director.factory_resource(), Resource::Full { .. }))
        }
        ScoringFactoryStrategy::Stream => assert!(matches!(
            director.factory_resource(),
            Resource::Stream { .. }
        )),
        _ => unreachable!(),
    }
}

#[test]
fn with_easy_score_calculator_class() {
    registered_recipe(ScoringFactoryStrategy::FullEvaluation);
}
#[test]
fn with_constraint_provider_class() {
    registered_recipe(ScoringFactoryStrategy::Stream);
}
