//! Native planning declarations validated before creating runtime solver state.
use crate::*;
use std::{collections::HashSet, error::Error, fmt};
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct PlanningFieldKey {
    pub entity: PlanningType,
    pub variable_name: String,
}
impl PlanningFieldKey {
    pub fn of<E: 'static>(variable_name: impl Into<String>) -> Self {
        Self {
            entity: PlanningType::of::<E>(),
            variable_name: variable_name.into(),
        }
    }
}
#[derive(Clone, Debug)]
pub struct VariableSorterDeclaration {
    pub field: PlanningFieldKey,
    pub comparator: Option<PlanningType>,
    pub comparator_factory: Option<PlanningType>,
}
#[derive(Clone, Debug)]
pub struct ConstructionSelectorDeclaration {
    pub field: PlanningFieldKey,
    pub sorted: bool,
    pub cache: SelectionCacheScope,
    /// A destination range supplied by the selected entity, rather than shared.
    pub entity_scoped_range: bool,
    pub nearby: bool,
}
impl ConstructionSelectorDeclaration {
    pub fn for_list_problem<V: Eq + Clone>(
        field: PlanningFieldKey,
        problem: &ListProblem<V>,
        sorted: bool,
        cache: SelectionCacheScope,
        nearby: bool,
    ) -> Self {
        Self {
            field,
            sorted,
            cache,
            nearby,
            entity_scoped_range: problem.entities().iter().any(|e| e.value_range().is_some()),
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SolverPreflightError(pub String);
impl fmt::Display for SolverPreflightError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl Error for SolverPreflightError {}
fn error(text: impl Into<String>) -> SolverPreflightError {
    SolverPreflightError(text.into())
}
#[derive(Clone, Debug)]
pub enum ExhaustiveFieldSelection {
    All,
    BasicChange(Option<PlanningFieldKey>),
    Cartesian(Vec<Self>),
}
impl ExhaustiveFieldSelection {
    fn fields(
        &self,
        schema: &PlanningSchema,
        all: &HashSet<PlanningFieldKey>,
        depth: usize,
    ) -> Result<HashSet<PlanningFieldKey>, SolverPreflightError> {
        if depth > 128 {
            return Err(error("exhaustive selector nesting limit exceeded"));
        }
        match self {
            Self::All => Ok(all.clone()),
            Self::BasicChange(Some(field)) => {
                let entity = schema
                    .entity_type(field.entity)
                    .map_err(|e| error(e.to_string()))?;
                let variable = entity
                    .genuine_variable(&field.variable_name)
                    .map_err(|e| error(e.to_string()))?;
                if !matches!(variable.kind(), PlanningVariableKind::Basic { .. }) {
                    return Err(error("BasicChange requires a basic planning variable"));
                }
                Ok(HashSet::from([field.clone()]))
            }
            Self::BasicChange(None) => Ok(schema
                .genuine_entities()
                .flat_map(|e| e.genuine_variables().map(move |v| (e, v)))
                .find(|(_, v)| matches!(v.kind(), PlanningVariableKind::Basic { .. }))
                .map(|(e, v)| PlanningFieldKey {
                    entity: e.entity_type(),
                    variable_name: v.name().into(),
                })
                .into_iter()
                .collect()),
            Self::Cartesian(children) => {
                let mut out = HashSet::new();
                for child in children {
                    out.extend(child.fields(schema, all, depth + 1)?);
                }
                Ok(out)
            }
        }
    }
}
/// Explicit phase/settings declarations. The native factory binder receives the
/// validated settings and must implement their selection policy or reject it.
/// Validation does not install comparators, caches or nearby distance functions.
#[derive(Clone, Debug, Default)]
pub struct SolverBuildSettings {
    pub sorters: Vec<VariableSorterDeclaration>,
    pub selectors: Vec<ConstructionSelectorDeclaration>,
    /// None requests automatic exhaustive selection. Explicit selector trees
    /// must cover every genuine field, including list fields.
    pub exhaustive_selection: Option<ExhaustiveFieldSelection>,
}
impl SolverBuildSettings {
    pub fn validate(&self, schema: &PlanningSchema) -> Result<(), SolverPreflightError> {
        let fields: HashSet<_> = schema
            .genuine_entities()
            .flat_map(|e| {
                e.genuine_variables().map(move |v| PlanningFieldKey {
                    entity: e.entity_type(),
                    variable_name: v.name().into(),
                })
            })
            .collect();
        for key in self
            .sorters
            .iter()
            .map(|v| &v.field)
            .chain(self.selectors.iter().map(|v| &v.field))
        {
            if !fields.contains(key) {
                return Err(error(format!(
                    "unknown genuine planning field {}.{}",
                    key.entity.name(),
                    key.variable_name
                )));
            }
        }
        for sorter in &self.sorters {
            if let (Some(comparator), Some(factory)) =
                (sorter.comparator, sorter.comparator_factory)
            {
                return Err(error(format!(
                    "The entityClass ({}) property ({}) cannot have a comparatorClass ({}) and comparatorFactoryClass ({}) at the same time.",
                    sorter.field.entity.name(),
                    sorter.field.variable_name,
                    comparator.name(),
                    factory.name()
                )));
            }
        }
        for selector in &self.selectors {
            if selector.sorted
                && selector.entity_scoped_range
                && matches!(
                    selector.cache,
                    SelectionCacheScope::Phase | SelectionCacheScope::Solve
                )
            {
                return Err(error(format!(
                    "resolvedSelectionOrder (SORTED) which does not support the resolvedCacheType ({}). Maybe set the \"cacheType\" to STEP.",
                    if selector.cache == SelectionCacheScope::Phase {
                        "PHASE"
                    } else {
                        "SOLVER"
                    }
                )));
            }
            if selector.sorted && selector.nearby {
                return Err(error(
                    "The nearbySelectorConfig conflicts with sorted selection. Maybe remove difficultyComparatorClass or difficultyWeightFactoryClass from your @PlanningEntity annotation. Maybe remove strengthComparatorClass or strengthWeightFactoryClass from your @PlanningVariable annotation. Maybe disable nearby selection.",
                ));
            }
        }
        if let Some(selected) = &self.exhaustive_selection {
            let selected = selected.fields(schema, &fields, 0)?;
            if selected != fields {
                return Err(error(
                    "Exhaustive selector does not include all the expected planning variables",
                ));
            }
        }
        Ok(())
    }
    pub fn prepare<S>(
        &self,
        schema: &PlanningSchema,
        phases: Option<SolverSequence<'static, S>>,
    ) -> Result<SolverSequence<'static, S>, SolverPreflightError> {
        self.validate(schema)?;
        if let Some(phases) = phases {
            if phases.phases.is_empty() {
                return Err(error("a solver requires at least one phase"));
            }
            return Ok(phases);
        }
        let mut basic = false;
        let mut list = false;
        for entity in schema.genuine_entities() {
            for variable in entity.genuine_variables() {
                match variable.kind() {
                    PlanningVariableKind::Basic { .. } => basic = true,
                    PlanningVariableKind::List { .. } => list = true,
                    PlanningVariableKind::Shadow => {}
                }
            }
        }
        if basic && list {
            return Err(error(
                "The model has both basic and list variables and cannot be deduced automatically; supply explicit phases.",
            ));
        }
        Ok(SolverSequence::default())
    }
    /// The binder builds a real existing native factory after all preflight has
    /// succeeded. Invalid metadata never invokes it or reads a problem instance.
    pub fn build<P, R, E, S: 'static>(
        &self,
        schema: &PlanningSchema,
        phases: Option<SolverSequence<'static, S>>,
        binder: impl FnOnce(
            SolverSequence<'static, S>,
            &Self,
        ) -> Result<SolverFactory<P, R, E, S>, SolverPreflightError>,
    ) -> Result<SolverFactory<P, R, E, S>, SolverPreflightError> {
        let phases = self.prepare(schema, phases)?;
        binder(phases, self)
    }
}
