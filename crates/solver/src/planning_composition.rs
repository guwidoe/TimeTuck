//! Compile explicit component ancestry into ordinary planning schemas.
use crate::{
    PlanningEntityDefinition, PlanningSchema, PlanningSchemaError, PlanningType,
    PlanningVariableDefinition,
};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fmt,
    num::NonZeroUsize,
};

/// A parent supplies an owned embedded model or a projected field contract.
/// These are explicit metadata links, not Rust inheritance or reflection.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum PlanningParentKind {
    Owned,
    Projection,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlanningParent {
    pub component: PlanningType,
    pub kind: PlanningParentKind,
}
#[derive(Clone, Debug)]
pub struct PlanningComponent {
    component: PlanningType,
    registered: bool,
    variables: Vec<PlanningVariableDefinition>,
    parents: Vec<PlanningParent>,
}
impl PlanningComponent {
    pub fn registered<T: 'static>(variables: Vec<PlanningVariableDefinition>) -> Self {
        Self {
            component: PlanningType::of::<T>(),
            registered: true,
            variables,
            parents: vec![],
        }
    }
    /// Plain model components may relay ancestry, but cannot declare planning
    /// fields. Register the actual declaring component to make fields eligible.
    pub fn plain<T: 'static>(variables: Vec<PlanningVariableDefinition>) -> Self {
        Self {
            component: PlanningType::of::<T>(),
            registered: false,
            variables,
            parents: vec![],
        }
    }
    pub fn with_parent<T: 'static>(mut self, kind: PlanningParentKind) -> Self {
        self.parents.push(PlanningParent {
            component: PlanningType::of::<T>(),
            kind,
        });
        self
    }
    pub fn component_type(&self) -> PlanningType {
        self.component
    }
    pub fn variables(&self) -> &[PlanningVariableDefinition] {
        &self.variables
    }
    pub fn parents(&self) -> &[PlanningParent] {
        &self.parents
    }
}
/// Default composition accepts arbitrary acyclic graphs with unique field names.
/// Restrictive policies are explicit model validation choices, not runtime solver
/// or language restrictions. Depth includes the concrete component itself.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlanningCompositionPolicy {
    pub maximum_depth: Option<NonZeroUsize>,
    pub allow_multiple_parents: bool,
    pub allow_mixed_parent_kinds: bool,
}
impl Default for PlanningCompositionPolicy {
    fn default() -> Self {
        Self {
            maximum_depth: None,
            allow_multiple_parents: true,
            allow_mixed_parent_kinds: true,
        }
    }
}
impl PlanningCompositionPolicy {
    /// One concrete component and at most one direct parent, with no deeper
    /// ancestors. Useful for simple models with a single field contract.
    pub fn single_level() -> Self {
        Self {
            maximum_depth: NonZeroUsize::new(2),
            allow_multiple_parents: false,
            allow_mixed_parent_kinds: false,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlanningCompositionError {
    DuplicateComponent(PlanningType),
    DuplicateRoot(PlanningType),
    UnknownComponent(PlanningType),
    UnknownParent {
        component: PlanningType,
        parent: PlanningType,
    },
    DuplicateParent {
        component: PlanningType,
        parent: PlanningType,
    },
    Cycle {
        component: PlanningType,
        parent: PlanningType,
    },
    UnregisteredVariables(PlanningType),
    MixedParents(PlanningType),
    MultipleParents(PlanningType),
    DepthLimit {
        entity: PlanningType,
        actual: usize,
        maximum: usize,
    },
    RedefinedVariable {
        entity: PlanningType,
        variable: String,
        original: PlanningType,
        redefining: PlanningType,
    },
    Schema(PlanningSchemaError),
}
impl fmt::Display for PlanningCompositionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnregisteredVariables(t) => write!(
                f,
                "{} is not registered as a planning component but defines genuine or shadow variables.",
                t.name()
            ),
            Self::MixedParents(t) => write!(
                f,
                "Mixed inheritance is not permitted by the composition policy for {}.",
                t.name()
            ),
            Self::MultipleParents(t) => write!(
                f,
                "{} requires a single-parent inheritance structure under this composition policy.",
                t.name()
            ),
            Self::DepthLimit {
                entity,
                actual,
                maximum,
            } => write!(
                f,
                "{} has component depth {actual}, exceeding {maximum}; use a single-level inheritance structure or a less restrictive composition policy.",
                entity.name()
            ),
            Self::RedefinedVariable {
                entity,
                variable,
                original,
                redefining,
            } => write!(
                f,
                "{} redefines the planning variables ([{variable}]), which is not permitted. {} conflicts with {}.",
                entity.name(),
                redefining.name(),
                original.name()
            ),
            _ => write!(f, "planning composition: {self:?}"),
        }
    }
}
impl std::error::Error for PlanningCompositionError {}
#[derive(Clone, Debug)]
/// Explicit field-declaring components, compiled once before native solving.
///
/// ```
/// use timetuck::*;
/// struct World; struct Common; struct Task;
/// let graph = PlanningComposition::new(vec![
///     PlanningComponent::registered::<Common>(vec![
///         PlanningVariableDefinition::basic::<i32>("worker", false)]),
///     PlanningComponent::plain::<Task>(vec![])
///         .with_parent::<Common>(PlanningParentKind::Owned),
/// ])?;
/// let compiled = graph.compile::<World>(&[PlanningType::of::<Task>()],
///     PlanningCompositionPolicy::single_level())?;
/// let field = compiled.schema().genuine_entity::<Task>()?.genuine_variable("worker")?;
/// let ranges = field.shared_ranges(vec![vec![1, 2]], 2)?;
/// assert_eq!(ranges.variable(0).unwrap().domain(), &[1, 2]);
/// assert_eq!(compiled.field_origin(PlanningType::of::<Task>(), "worker"),
///     Some(PlanningType::of::<Common>()));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub struct PlanningComposition {
    components: Vec<PlanningComponent>,
    indices: HashMap<PlanningType, usize>,
}
/// A regular canonical schema plus the original declaring type for each field.
/// Existing range, statistics, slot and move bindings operate on schema().
#[derive(Debug)]
pub struct ComposedPlanningSchema {
    schema: PlanningSchema,
    origins: HashMap<PlanningType, BTreeMap<String, PlanningType>>,
}
impl ComposedPlanningSchema {
    pub fn schema(&self) -> &PlanningSchema {
        &self.schema
    }
    pub fn into_schema(self) -> PlanningSchema {
        self.schema
    }
    /// Query without allocating or traversing the ancestry graph.
    pub fn field_origin(&self, entity: PlanningType, field: &str) -> Option<PlanningType> {
        self.origins.get(&entity)?.get(field).copied()
    }
}
impl PlanningComposition {
    pub fn new(components: Vec<PlanningComponent>) -> Result<Self, PlanningCompositionError> {
        let mut indices = HashMap::with_capacity(components.len());
        for (index, component) in components.iter().enumerate() {
            if indices.insert(component.component, index).is_some() {
                return Err(PlanningCompositionError::DuplicateComponent(
                    component.component,
                ));
            }
        }
        Ok(Self {
            components,
            indices,
        })
    }
    /// Compile only requested roots and their reachable ancestors. Root order,
    /// parent order and field encounter order are preserved. Shared ancestors
    /// contribute once per concrete root; distinct owners cannot redefine a name.
    /// Traversal is iterative, detects cycles and validates registration before
    /// constructing any schema. All validation/cloning happens before solving;
    /// no model callbacks, scoring or value/range access occurs here.
    pub fn compile<S: 'static>(
        &self,
        roots: &[PlanningType],
        policy: PlanningCompositionPolicy,
    ) -> Result<ComposedPlanningSchema, PlanningCompositionError> {
        let mut seen_roots = HashSet::new();
        let mut entities = vec![];
        let mut origins = HashMap::new();
        for &root in roots {
            if !seen_roots.insert(root) {
                return Err(PlanningCompositionError::DuplicateRoot(root));
            }
            let &index = self
                .indices
                .get(&root)
                .ok_or(PlanningCompositionError::UnknownComponent(root))?;
            let order = self.ancestry(index, policy)?;
            let mut vars = vec![];
            let mut owners = BTreeMap::new();
            for i in order {
                let component = &self.components[i];
                for variable in &component.variables {
                    if let Some(&original) = owners.get(variable.name()) {
                        return Err(PlanningCompositionError::RedefinedVariable {
                            entity: root,
                            variable: variable.name().into(),
                            original,
                            redefining: component.component,
                        });
                    }
                    owners.insert(variable.name().to_owned(), component.component);
                    vars.push(variable.clone());
                }
            }
            entities.push(PlanningEntityDefinition::from_parts(root, vars));
            origins.insert(root, owners);
        }
        Ok(ComposedPlanningSchema {
            schema: PlanningSchema::new::<S>(entities).map_err(PlanningCompositionError::Schema)?,
            origins,
        })
    }
    fn ancestry(
        &self,
        root: usize,
        policy: PlanningCompositionPolicy,
    ) -> Result<Vec<usize>, PlanningCompositionError> {
        let mut stack = vec![(root, 0usize)];
        let mut states = HashMap::new();
        let mut heights = HashMap::new();
        let mut order = vec![];
        while let Some((i, next)) = stack.last_mut() {
            let component = &self.components[*i];
            if states.get(i).copied().unwrap_or(0) == 0 {
                if !component.registered && !component.variables.is_empty() {
                    return Err(PlanningCompositionError::UnregisteredVariables(
                        component.component,
                    ));
                }
                let mut names = HashSet::new();
                for variable in &component.variables {
                    if variable.name().is_empty() {
                        return Err(PlanningCompositionError::Schema(
                            PlanningSchemaError::EmptyVariableName(component.component),
                        ));
                    }
                    if !names.insert(variable.name()) {
                        return Err(PlanningCompositionError::Schema(
                            PlanningSchemaError::DuplicateVariable {
                                entity: component.component,
                                variable: variable.name().into(),
                            },
                        ));
                    }
                }
                let mut parents = HashSet::new();
                for parent in &component.parents {
                    if !parents.insert(parent.component) {
                        return Err(PlanningCompositionError::DuplicateParent {
                            component: component.component,
                            parent: parent.component,
                        });
                    }
                }
                if !policy.allow_mixed_parent_kinds
                    && component.parents.windows(2).any(|p| p[0].kind != p[1].kind)
                {
                    return Err(PlanningCompositionError::MixedParents(component.component));
                }
                if !policy.allow_multiple_parents && component.parents.len() > 1 {
                    return Err(PlanningCompositionError::MultipleParents(
                        component.component,
                    ));
                }
                states.insert(*i, 1);
            }
            if let Some(parent) = component.parents.get(*next) {
                *next += 1;
                let &p = self.indices.get(&parent.component).ok_or(
                    PlanningCompositionError::UnknownParent {
                        component: component.component,
                        parent: parent.component,
                    },
                )?;
                match states.get(&p).copied().unwrap_or(0) {
                    1 => {
                        return Err(PlanningCompositionError::Cycle {
                            component: component.component,
                            parent: parent.component,
                        });
                    }
                    0 => stack.push((p, 0)),
                    _ => {}
                }
            } else {
                let (i, _) = stack.pop().unwrap();
                let height = self.components[i]
                    .parents
                    .iter()
                    .map(|p| heights[&self.indices[&p.component]])
                    .max()
                    .unwrap_or(0usize)
                    + 1;
                heights.insert(i, height);
                states.insert(i, 2);
                order.push(i);
            }
        }
        let depth = heights[&root];
        if let Some(max) = policy.maximum_depth.filter(|m| depth > m.get()) {
            return Err(PlanningCompositionError::DepthLimit {
                entity: self.components[root].component,
                actual: depth,
                maximum: max.get(),
            });
        }
        Ok(order)
    }
}
