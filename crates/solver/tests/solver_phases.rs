//! Original solver orchestration tests split for independent compilation.
include!("solver_support/fixture.rs");

#[path = "support/solver_sequence_cases.rs"]
mod sequence_cases;

#[path = "support/solver_construction_cases.rs"]
mod construction_cases;

#[path = "support/solver_incomplete_handoff_cases.rs"]
mod incomplete_handoff_cases;

#[path = "support/solver_step_counting_cases.rs"]
mod step_counting_cases;

#[path = "support/solver_great_deluge_cases.rs"]
mod great_deluge_cases;
