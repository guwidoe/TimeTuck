//! Fluent time settings on the configurations consumed by native solvers.
use crate::{
    MixedSolverConfig, MixedSolverPlan, MixedSolverSequence, SolverConfig, SolverLimits,
    SolverPlan, SolverSequence,
};
use std::time::Duration;

/// Configure only the selected solve-wide limit. `None` disables that limit;
/// zero remains an immediate cooperative limit. Phase-local settings and all
/// other limits are preserved. The native engines enforce these same fields.
pub trait SolverTimeSettings: Sized {
    fn with_time_limit(self, limit: Option<Duration>) -> Self;
    fn with_unimproved_time_limit(self, limit: Option<Duration>) -> Self;
}
impl<S> SolverTimeSettings for SolverLimits<'_, S> {
    fn with_time_limit(mut self, limit: Option<Duration>) -> Self {
        self.max_time = limit;
        self
    }
    fn with_unimproved_time_limit(mut self, limit: Option<Duration>) -> Self {
        self.max_unimproved_time = limit;
        self
    }
}
macro_rules! configurations {
    ($($config:ident),+ $(,)?) => {$(
        impl<S> SolverTimeSettings for $config<'_, S> {
            fn with_time_limit(mut self, limit: Option<Duration>) -> Self {
                self.limits.max_time = limit;
                self
            }
            fn with_unimproved_time_limit(mut self, limit: Option<Duration>) -> Self {
                self.limits.max_unimproved_time = limit;
                self
            }
        }
    )+};
}
configurations!(
    SolverConfig,
    SolverPlan,
    SolverSequence,
    MixedSolverConfig,
    MixedSolverPlan,
    MixedSolverSequence,
);
