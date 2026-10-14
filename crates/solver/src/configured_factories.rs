//! Owned declarative resources resolved into real named factory configuration.
use crate::*;
use std::{collections::BTreeMap, fmt, str::FromStr};
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfiguredFactoryError(pub String);
impl fmt::Display for ConfiguredFactoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for ConfiguredFactoryError {}
fn error(value: impl ToString) -> ConfiguredFactoryError {
    ConfiguredFactoryError(value.to_string())
}
#[derive(Clone, Debug)]
pub struct ConfiguredSolverRequest<S: 'static = SimpleScore> {
    pub configuration: NamedSolverConfiguration<S>,
    pub selected_resource: Option<String>,
    pub manager_workers: usize,
    pub xml_termination: Option<XmlTerminationPlan>,
    pub runtime_termination: DiminishedRuntimeBinding,
}
impl<S: Clone + Send + Sync + 'static> ConfiguredSolverRequest<S> {
    pub fn build_manager<P: Send + 'static, R: Send + Sync + 'static, E: Send + Sync + 'static>(
        &self,
        factory: &SolverFactory<P, R, E, S>,
    ) -> std::io::Result<ConfiguredSolverManager<P, R, E, S>> {
        factory.build_manager(self.manager_workers)
    }
}
/// Explicit owned resource and typed entity registrations; no classpath or I/O.
pub struct NativeSolverConfigurationSource {
    resources: BTreeMap<String, SolverXmlConfiguration>,
    entities: BTreeMap<String, PlanningType>,
    default_resource: Option<String>,
    global_resource_property: Option<String>,
    rules: PropertyFallbackRules,
}
impl Default for NativeSolverConfigurationSource {
    fn default() -> Self {
        Self {
            resources: BTreeMap::new(),
            entities: BTreeMap::new(),
            default_resource: None,
            global_resource_property: None,
            rules: PropertyFallbackRules::community_test_profile(),
        }
    }
}
impl NativeSolverConfigurationSource {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn register_resource(
        &mut self,
        key: impl Into<String>,
        xml: SolverXmlConfiguration,
    ) -> Result<(), ConfiguredFactoryError> {
        let key = key.into();
        if key.is_empty() || self.resources.contains_key(&key) {
            return Err(error("empty or duplicate XML resource key"));
        }
        self.resources.insert(key, xml);
        Ok(())
    }
    pub fn register_entity<E: 'static>(
        &mut self,
        key: impl Into<String>,
    ) -> Result<(), ConfiguredFactoryError> {
        let key = key.into();
        if key.is_empty() || self.entities.contains_key(&key) {
            return Err(error("empty or duplicate entity key"));
        }
        self.entities.insert(key, PlanningType::of::<E>());
        Ok(())
    }
    pub fn default_resource(&mut self, key: Option<String>) {
        self.default_resource = key;
    }
    pub fn global_resource_property(&mut self, key: Option<String>) {
        self.global_resource_property = key;
    }
    pub fn property_rules(&mut self, rules: PropertyFallbackRules) {
        self.rules = rules;
    }
    /// Specific XML beats the caller's global resource property, then default.
    /// XML is bound first; supplied runtime properties override shared controls.
    /// Existing explicit native/XML phase policies retain their precedence.
    pub fn resolve<S: Clone + FromStr + 'static>(
        &self,
        base: NamedSolverConfiguration<S>,
        source: &OwnedPropertySource,
        prefix: &str,
        sink: &mut impl FnMut(PropertyFallbackWarning),
    ) -> Result<ConfiguredSolverRequest<S>, ConfiguredFactoryError> {
        let specific = format!("{prefix}.solver-config-xml");
        let selected = self
            .rules
            .resolve(source, &specific, sink)
            .map(str::to_owned)
            .or_else(|| {
                self.global_resource_property
                    .as_ref()
                    .and_then(|key| self.rules.resolve(source, key, sink))
                    .map(str::to_owned)
            })
            .or_else(|| self.default_resource.clone());
        let mut configuration = base;
        let xml_termination = if let Some(key) = &selected {
            let xml = self
                .resources
                .get(key)
                .ok_or_else(|| error(format!("unknown XML resource {key}")))?;
            configuration.apply_xml_selection(xml).map_err(error)?;
            let declared = xml
                .root()
                .children
                .iter()
                .filter(|node| node.name == "entityClass")
                .map(|node| {
                    self.entities.get(node.text.trim()).copied().ok_or_else(|| {
                        error(format!("unregistered entity key {}", node.text.trim()))
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            if !declared.is_empty() {
                configuration.entity_types = Some(declared);
            }
            let plan = xml.native_termination_plan().map_err(error)?;
            bind_owned(&plan, &mut configuration.sequence)?;
            Some(plan)
        } else {
            None
        };
        let termination_prefix = format!("{prefix}.termination");
        let runtime = RuntimePropertyPlan::resolve(source, &self.rules, &termination_prefix, sink)
            .map_err(error)?;
        let runtime_termination = runtime.bind(&mut configuration.sequence).map_err(error)?;
        let key = format!("{termination_prefix}.best-score-limit");
        if let Some(raw) = self.rules.resolve(source, &key, sink) {
            configuration.sequence.limits.best_score_limit = Some(
                raw.parse()
                    .map_err(|_| error(format!("invalid score property {key}")))?,
            );
        }
        let workers_key = "timefold.solver-manager.parallel-solver-count";
        let manager_workers = self
            .rules
            .resolve(source, workers_key, sink)
            .map(|raw| {
                raw.parse::<usize>()
                    .map_err(|_| error("parallel solver count requires a positive integer"))
            })
            .transpose()?
            .unwrap_or(1);
        if !(1..=128).contains(&manager_workers) {
            return Err(error(
                "native parallel solver count must be between 1 and 128",
            ));
        }
        Ok(ConfiguredSolverRequest {
            configuration,
            selected_resource: selected,
            manager_workers,
            xml_termination,
            runtime_termination,
        })
    }
}
/// Transfer only validated owned values from the existing executable binder.
/// References in the original static template are retained, never extended.
fn bind_owned<S: Clone + FromStr>(
    plan: &XmlTerminationPlan,
    target: &mut SolverSequence<'static, S>,
) -> Result<(), ConfiguredFactoryError> {
    if plan.solver.as_ref().is_some_and(|raw| raw.steps.is_some()) {
        return Err(error(
            "owned factory XML profile does not support solve-wide stepCountLimit; use XmlTerminationPlan::with_sequence",
        ));
    }
    let mut staged = target.clone();
    plan.with_sequence(target.clone(), |effective| {
        staged.limits.max_time = effective.limits.max_time;
        staged.limits.max_unimproved_time = effective.limits.max_unimproved_time;
        staged.limits.max_unimproved_steps = effective.limits.max_unimproved_steps;
        staged.limits.best_score_limit = effective.limits.best_score_limit.clone();
        for (owned, borrowed) in staged.phases.iter_mut().zip(&effective.phases) {
            match (owned, borrowed) {
                (SolverPhase::Construction(a), SolverPhase::Construction(b)) => {
                    a.max_time = b.max_time;
                    a.max_steps = b.max_steps;
                }
                (SolverPhase::LocalSearch(a), SolverPhase::LocalSearch(b)) => {
                    a.max_time = b.max_time;
                    a.max_unimproved_time = b.max_unimproved_time;
                    a.max_steps = b.max_steps;
                    a.max_unimproved_steps = b.max_unimproved_steps;
                    a.best_score_limit = b.best_score_limit.clone();
                    a.diminished_returns = b.diminished_returns;
                }
                (SolverPhase::Exact(a), SolverPhase::Exact(b)) => a.max_time = b.max_time,
                (SolverPhase::Custom(a), SolverPhase::Custom(b)) => a.max_time = b.max_time,
                (SolverPhase::RuinRecreate(a), SolverPhase::RuinRecreate(b)) => {
                    a.max_time = b.max_time
                }
                _ => unreachable!("existing XML binder validates phase kinds"),
            }
        }
    })
    .map_err(error)?;
    *target = staged;
    Ok(())
}
