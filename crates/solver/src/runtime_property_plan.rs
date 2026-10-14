//! Native property ingestion. Supplied fields override shared limits; missing
//! fields and existing explicit local diminished policies remain intact.
use crate::{
    DiminishedRuntimeBinding, JsonDuration, MixedSolverSequence, OwnedPropertySource,
    PropertyFallbackRules, PropertyFallbackWarning, RuntimeDiminishedReturns,
    RuntimeSolverProperties, RuntimeTerminationProperties, SolverSequence,
};
use std::{fmt, time::Duration};
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimePropertyPlanError {
    pub key: String,
    pub message: String,
}
impl fmt::Display for RuntimePropertyPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.key, self.message)
    }
}
impl std::error::Error for RuntimePropertyPlanError {}
fn error(key: &str, message: impl ToString) -> RuntimePropertyPlanError {
    RuntimePropertyPlanError {
        key: key.into(),
        message: message.to_string(),
    }
}
/// ISO parsing delegates to JsonDuration. Native short form is an unsigned
/// integer followed by ms/s/m/h/d, with checked arithmetic; no whitespace.
pub fn parse_runtime_property_duration(
    key: &str,
    value: &str,
) -> Result<Duration, RuntimePropertyPlanError> {
    if value.starts_with('P') || value.starts_with("-P") || value.starts_with("+P") {
        return value
            .parse::<JsonDuration>()
            .and_then(JsonDuration::to_std)
            .map_err(|e| error(key, e));
    }
    let (digits, unit) = value
        .find(|c: char| !c.is_ascii_digit())
        .map(|i| value.split_at(i))
        .ok_or_else(|| error(key, "expected ISO duration or unsigned integer with unit"))?;
    if digits.is_empty() {
        return Err(error(key, "duration must have an unsigned integer"));
    }
    let number = digits.parse::<u64>().map_err(|e| error(key, e))?;
    if unit == "ms" {
        return Ok(Duration::from_millis(number));
    }
    let scale = match unit {
        "s" => 1,
        "m" => 60,
        "h" => 3600,
        "d" => 86400,
        _ => return Err(error(key, "unsupported native duration unit")),
    };
    Ok(Duration::from_secs(
        number
            .checked_mul(scale)
            .ok_or_else(|| error(key, "duration overflow"))?,
    ))
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RuntimePropertyPlan {
    pub spent_limit: Option<Duration>,
    pub unimproved_spent_limit: Option<Duration>,
    pub solver: RuntimeSolverProperties,
}
impl RuntimePropertyPlan {
    /// Namespace is explicit (for example timefold.model.termination or
    /// quarkus.timefold.solver.termination). No automatic framework config loading.
    pub fn resolve(
        source: &OwnedPropertySource,
        rules: &PropertyFallbackRules,
        prefix: &str,
        sink: &mut impl FnMut(PropertyFallbackWarning),
    ) -> Result<Self, RuntimePropertyPlanError> {
        let key = |suffix: &str| format!("{prefix}.{suffix}");
        let mut duration = |suffix: &str| {
            let k = key(suffix);
            rules
                .resolve(source, &k, sink)
                .map(|v| parse_runtime_property_duration(&k, v))
                .transpose()
        };
        let spent_limit = duration("spent-limit")?;
        let unimproved_spent_limit = duration("unimproved-spent-limit")?;
        let window = duration("diminished-returns.sliding-window-duration")?;
        let k = key("diminished-returns.enabled");
        let enabled = rules
            .resolve(source, &k, sink)
            .map(|v| match v {
                "true" => Ok(true),
                "false" => Ok(false),
                _ => Err(error(&k, "expected true or false")),
            })
            .transpose()?;
        let k = key("diminished-returns.minimum-improvement-ratio");
        let ratio = rules
            .resolve(source, &k, sink)
            .map(|v| v.parse::<f64>().map_err(|e| error(&k, e)))
            .transpose()?;
        // Validate even explicitly disabled supplied ratios/windows; no hidden malformed declarations.
        if ratio.is_some_and(|v| !v.is_finite() || v <= 0.0) {
            return Err(error(&k, "ratio must be positive finite"));
        }
        if window == Some(Duration::ZERO) {
            return Err(error(
                &key("diminished-returns.sliding-window-duration"),
                "window must be positive",
            ));
        }
        let raw = RuntimeDiminishedReturns {
            enabled,
            sliding_window_duration: window,
            minimum_improvement_ratio: ratio,
        };
        let present = enabled.is_some() || window.is_some() || ratio.is_some();
        Ok(Self {
            spent_limit,
            unimproved_spent_limit,
            solver: RuntimeSolverProperties {
                termination: present.then_some(RuntimeTerminationProperties {
                    diminished_returns: Some(raw),
                }),
            },
        })
    }
    pub fn bind<S: Clone>(
        &self,
        plan: &mut SolverSequence<'_, S>,
    ) -> Result<DiminishedRuntimeBinding, RuntimePropertyPlanError> {
        let mut staged = plan.clone();
        let receipt = self
            .solver
            .bind(&mut staged)
            .map_err(|e| error("diminished-returns", e))?;
        if let Some(v) = self.spent_limit {
            staged.limits.max_time = Some(v);
        }
        if let Some(v) = self.unimproved_spent_limit {
            staged.limits.max_unimproved_time = Some(v);
        }
        *plan = staged;
        Ok(receipt)
    }
    pub fn bind_mixed<S: Clone>(
        &self,
        plan: &mut MixedSolverSequence<'_, S>,
    ) -> Result<DiminishedRuntimeBinding, RuntimePropertyPlanError> {
        let mut staged = plan.clone();
        let receipt = self
            .solver
            .bind_mixed(&mut staged)
            .map_err(|e| error("diminished-returns", e))?;
        if let Some(v) = self.spent_limit {
            staged.limits.max_time = Some(v);
        }
        if let Some(v) = self.unimproved_spent_limit {
            staged.limits.max_unimproved_time = Some(v);
        }
        *plan = staged;
        Ok(receipt)
    }
}
