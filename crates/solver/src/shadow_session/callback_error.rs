use super::*;
use std::sync::Arc;

/// The evaluation stage that returned an application error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShadowCallbackStage {
    Input,
    Derived,
}

#[derive(Debug)]
struct Failure {
    registration_index: usize,
    variable: String,
    entity_type: &'static str,
    stage: ShadowCallbackStage,
    cause: Box<dyn Error + Send + Sync>,
}

/// One failed callback invocation, retaining its original application error.
/// Clones compare equal by failure identity; separate invocations remain distinct
/// even when their messages match. Registration indices are local to the session
/// that returned the error, not portable handles or identifiers across sessions.
#[derive(Clone, Debug)]
pub struct ShadowCallbackError(Arc<Failure>);

impl ShadowCallbackError {
    pub(super) fn new(
        registration_index: usize,
        variable: PlanningVariableRef<'_>,
        stage: ShadowCallbackStage,
        cause: Box<dyn Error + Send + Sync>,
    ) -> Self {
        Self(Arc::new(Failure {
            registration_index,
            variable: variable.name().into(),
            entity_type: variable.entity().entity_type().name(),
            stage,
            cause,
        }))
    }
    pub fn registration_index(&self) -> usize {
        self.0.registration_index
    }
    pub fn variable(&self) -> &str {
        &self.0.variable
    }
    pub fn entity_type(&self) -> &'static str {
        self.0.entity_type
    }
    pub fn stage(&self) -> ShadowCallbackStage {
        self.0.stage
    }
    /// The original error, including its concrete type for `downcast_ref`.
    pub fn cause(&self) -> &(dyn Error + Send + Sync + 'static) {
        self.0.cause.as_ref()
    }
}
impl PartialEq for ShadowCallbackError {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for ShadowCallbackError {}
impl fmt::Display for ShadowCallbackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:?} callback for {}.{} (registration {}) failed: {}",
            self.stage(),
            self.entity_type(),
            self.variable(),
            self.registration_index(),
            self.cause()
        )
    }
}
impl Error for ShadowCallbackError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.cause())
    }
}
