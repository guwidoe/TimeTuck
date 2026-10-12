// Adapted TEST-only AbstractConstraintStreamTest.TestConstraintJustification.
// Apache-2.0; Arc-backed native objects replace Java Object[] references.
#![allow(dead_code)]
use std::{any::Any, sync::Arc};
use timetuck::SimpleScore;
pub struct TestJustification {
    pub score: SimpleScore,
    pub facts: Vec<Arc<dyn Any + Send + Sync>>,
}
