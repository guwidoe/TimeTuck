//! Optional owned range diagnostics that survive failed-move rollback.
use crate::RangeViolationLocation;
use std::{fmt, sync::Arc};

/// Rendered context retained after a rejected candidate has been rolled back.
/// Every allowed value is listed in declared order, including sparse domains.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValueRangeReport {
    pub location: RangeViolationLocation,
    pub entity: String,
    pub variable: String,
    pub value: String,
    pub allowed: Vec<String>,
}
impl fmt::Display for ValueRangeReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "entity {}, variable {}: value {} outside allowed range [{}]",
            self.entity,
            self.variable,
            self.value,
            self.allowed.join(", ")
        )
    }
}
struct Metadata<V> {
    entity: String,
    variable: String,
    render: Box<dyn Fn(&V) -> String + Send + Sync>,
}
pub(crate) struct RangeDiagnostic<V>(Arc<Metadata<V>>);
impl<V> Clone for RangeDiagnostic<V> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<V> fmt::Debug for RangeDiagnostic<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RangeDiagnostic")
            .field("entity", &self.0.entity)
            .field("variable", &self.0.variable)
            .finish_non_exhaustive()
    }
}
impl<V> RangeDiagnostic<V> {
    pub(crate) fn new(
        entity: impl Into<String>,
        variable: impl Into<String>,
        render: impl Fn(&V) -> String + Send + Sync + 'static,
    ) -> Self {
        Self(Arc::new(Metadata {
            entity: entity.into(),
            variable: variable.into(),
            render: Box::new(render),
        }))
    }
    pub(crate) fn report(
        &self,
        location: RangeViolationLocation,
        value: &V,
        allowed: &[V],
    ) -> Box<ValueRangeReport> {
        Box::new(ValueRangeReport {
            location,
            entity: self.0.entity.clone(),
            variable: self.0.variable.clone(),
            value: (self.0.render)(value),
            allowed: allowed.iter().map(&self.0.render).collect(),
        })
    }
}
