//! Original solver orchestration tests split for independent compilation.
include!("solver_support/fixture.rs");

#[path = "support/solver_observer_cases.rs"]
mod observer_cases;

#[path = "support/solver_manager_integration.rs"]
mod manager_integration;

#[path = "support/solver_initialization_cases.rs"]
mod initialization_cases;
