//! Original solver orchestration tests split for independent compilation.
include!("solver_support/fixture.rs");

#[path = "support/solver_exact_cases.rs"]
mod exact_cases;

#[path = "support/solver_bounded_cases.rs"]
mod bounded_cases;
