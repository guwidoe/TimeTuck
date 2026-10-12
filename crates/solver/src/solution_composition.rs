//! Compile solution component declarations and bind their concrete basic models.
use crate::{
    ComposedPlanningSchema, FiniteProblem, ManagedValueRanges, ModelError, PlanningComposition,
    PlanningCompositionError, PlanningCompositionPolicy, PlanningSchema, PlanningType,
    PlanningVariableKind,
};
use std::{
    any::Any,
    collections::{HashMap, HashSet},
    fmt,
    num::NonZeroUsize,
    sync::Arc,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SolutionMemberKind {
    Facts {
        allowed_types: Vec<PlanningType>,
        singleton: bool,
    },
    Entities {
        allowed_types: Vec<PlanningType>,
        singleton: bool,
    },
    ValueRange {
        value_type: PlanningType,
        provider: String,
    },
    Score(PlanningType),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SolutionMemberCategory {
    ProblemFact,
    ProblemFactCollection,
    EntityCollection,
    Entity,
    ValueRange,
    Score,
}
impl SolutionMemberKind {
    pub fn category(&self) -> SolutionMemberCategory {
        match self {
            Self::Facts {
                singleton: true, ..
            } => SolutionMemberCategory::ProblemFact,
            Self::Facts {
                singleton: false, ..
            } => SolutionMemberCategory::ProblemFactCollection,
            Self::Entities {
                singleton: false, ..
            } => SolutionMemberCategory::EntityCollection,
            Self::Entities {
                singleton: true, ..
            } => SolutionMemberCategory::Entity,
            Self::ValueRange { .. } => SolutionMemberCategory::ValueRange,
            Self::Score(_) => SolutionMemberCategory::Score,
        }
    }
}
#[derive(Clone, Debug)]
pub struct SolutionMemberDefinition {
    name: String,
    kind: SolutionMemberKind,
    shape: SolutionMemberShape,
    parameters: Vec<PlanningType>,
    fact_collection: bool,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SolutionMemberShape {
    Scalar,
    Collection,
    Array,
}
impl SolutionMemberDefinition {
    fn declaration(
        name: impl Into<String>,
        kind: SolutionMemberKind,
        shape: SolutionMemberShape,
    ) -> Self {
        Self {
            name: name.into(),
            kind,
            shape,
            parameters: vec![],
            fact_collection: false,
        }
    }
    pub fn problem_fact<T: 'static>(name: impl Into<String>) -> Self {
        Self::problem_facts(name, vec![PlanningType::of::<T>()], true)
    }
    pub fn problem_fact_collection<T: 'static>(name: impl Into<String>) -> Self {
        Self::problem_facts(name, vec![PlanningType::of::<T>()], false)
    }
    /// Empty allowed types accept any concrete non-entity fact. Actual records
    /// still undergo role/type checks when an accessor reads the solution.
    pub fn problem_facts(
        name: impl Into<String>,
        allowed_types: Vec<PlanningType>,
        singleton: bool,
    ) -> Self {
        Self::declaration(
            name,
            SolutionMemberKind::Facts {
                allowed_types,
                singleton,
            },
            if singleton {
                SolutionMemberShape::Scalar
            } else {
                SolutionMemberShape::Collection
            },
        )
    }
    /// Declare the additional problem-fact role of a provider collection.
    /// Entity/fact role collisions are rejected during compilation.
    pub fn as_problem_fact_collection(mut self) -> Self {
        self.fact_collection = true;
        self
    }
    pub fn with_member_shape(mut self, shape: SolutionMemberShape) -> Self {
        self.shape = shape;
        self
    }
    pub fn with_external_parameter<P: 'static>(mut self) -> Self {
        self.parameters.push(PlanningType::of::<P>());
        self
    }
    pub fn entity_collection<E: 'static>(name: impl Into<String>) -> Self {
        Self::entities(name, vec![PlanningType::of::<E>()], false)
    }
    pub fn entity<E: 'static>(name: impl Into<String>) -> Self {
        Self::entities(name, vec![PlanningType::of::<E>()], true)
    }
    /// Explicit concrete types replace reflective subtype/object collection discovery.
    pub fn entities(
        name: impl Into<String>,
        allowed_types: Vec<PlanningType>,
        singleton: bool,
    ) -> Self {
        Self::declaration(
            name,
            SolutionMemberKind::Entities {
                allowed_types,
                singleton,
            },
            if singleton {
                SolutionMemberShape::Scalar
            } else {
                SolutionMemberShape::Collection
            },
        )
    }
    pub fn value_range<V: 'static>(name: impl Into<String>, provider: impl Into<String>) -> Self {
        Self::declaration(
            name,
            SolutionMemberKind::ValueRange {
                value_type: PlanningType::of::<V>(),
                provider: provider.into(),
            },
            SolutionMemberShape::Collection,
        )
    }
    pub fn score<S: 'static>(name: impl Into<String>) -> Self {
        Self::declaration(
            name,
            SolutionMemberKind::Score(PlanningType::of::<S>()),
            SolutionMemberShape::Scalar,
        )
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn kind(&self) -> &SolutionMemberKind {
        &self.kind
    }
}
#[derive(Clone, Debug)]
pub struct PlanningSolutionComponent {
    component: PlanningType,
    registered: bool,
    parent: Option<PlanningType>,
    members: Vec<SolutionMemberDefinition>,
}
impl PlanningSolutionComponent {
    pub fn registered<T: 'static>(members: Vec<SolutionMemberDefinition>) -> Self {
        Self {
            component: PlanningType::of::<T>(),
            registered: true,
            parent: None,
            members,
        }
    }
    pub fn plain<T: 'static>(members: Vec<SolutionMemberDefinition>) -> Self {
        Self {
            registered: false,
            ..Self::registered::<T>(members)
        }
    }
    pub fn with_parent<T: 'static>(mut self) -> Self {
        self.parent = Some(PlanningType::of::<T>());
        self
    }
}
/// The default permits arbitrary-depth embedded solution composition. Choosing
/// single_level is a metadata validation policy, not a Rust inheritance restriction.
#[derive(Clone, Copy, Debug, Default)]
pub struct SolutionCompositionPolicy {
    pub maximum_depth: Option<NonZeroUsize>,
}
impl SolutionCompositionPolicy {
    pub fn single_level() -> Self {
        Self {
            maximum_depth: NonZeroUsize::new(2),
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SolutionCompositionError {
    MemberParameters {
        member: String,
        parameters: Vec<PlanningType>,
    },
    CollectionShape {
        member: String,
        shape: SolutionMemberShape,
    },
    FactIsEntity {
        member: String,
        entity: PlanningType,
    },
    MemberRoleConflict(String),
    InvalidFactTypes(String),
    DuplicateComponent(PlanningType),
    UnknownComponent(PlanningType),
    Cycle(PlanningType),
    UnregisteredMembers {
        component: PlanningType,
        names: Vec<String>,
    },
    DepthLimit {
        solution: PlanningType,
        actual: usize,
        maximum: usize,
    },
    EmptyMemberName(PlanningType),
    DuplicateMember {
        name: String,
        original: PlanningType,
        redefining: PlanningType,
        kind: SolutionMemberCategory,
        replacement: SolutionMemberCategory,
    },
    DuplicateProvider(String),
    EmptyProvider(String),
    MissingScore,
    MultipleScores {
        first: String,
        second: String,
    },
    InvalidEntityTypes(String),
    Entities(Box<PlanningCompositionError>),
}
impl fmt::Display for SolutionCompositionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CollectionShape { member, .. } => write!(
                f,
                "solution member {member} does not return a Collection or an array."
            ),
            Self::UnregisteredMembers { component, names } => write!(
                f,
                "{} is not registered as a planning solution but defines planning members. Maybe register {} as a planning solution. Maybe remove the planning members ([{}]).",
                component.name(),
                component.name(),
                names.join(", ")
            ),
            Self::DepthLimit {
                solution,
                maximum: 2,
                ..
            } => write!(
                f,
                "{} inherits its planning solution registration from multiple components. Remove solution components from the composition chain to create a single-level inheritance structure.",
                solution.name()
            ),
            Self::DepthLimit {
                solution,
                actual,
                maximum,
            } => write!(
                f,
                "{} solution composition depth ({actual}) exceeds maximum ({maximum}).",
                solution.name()
            ),
            Self::DuplicateMember {
                name,
                kind: SolutionMemberCategory::EntityCollection,
                replacement: SolutionMemberCategory::EntityCollection,
                ..
            } => write!(
                f,
                "Solution has a planning entity collection member {name} that is duplicated by a planning entity collection member. Maybe the member is declared on both the field and its getter."
            ),
            _ => write!(f, "solution composition: {self:?}"),
        }
    }
}
impl std::error::Error for SolutionCompositionError {}

#[derive(Debug)]
pub struct CompiledSolutionMember {
    declaration: SolutionMemberDefinition,
    declaring_type: PlanningType,
}
impl CompiledSolutionMember {
    pub fn shape(&self) -> SolutionMemberShape {
        self.declaration.shape
    }
    pub fn is_problem_fact_collection(&self) -> bool {
        matches!(
            self.kind(),
            SolutionMemberKind::Facts {
                singleton: false,
                ..
            }
        ) || self.declaration.fact_collection
    }
    pub fn name(&self) -> &str {
        self.declaration.name()
    }
    pub fn kind(&self) -> &SolutionMemberKind {
        self.declaration.kind()
    }
    pub fn declaring_type(&self) -> PlanningType {
        self.declaring_type
    }
}
pub struct SolutionComposition {
    components: Vec<PlanningSolutionComponent>,
    indices: HashMap<PlanningType, usize>,
}
#[derive(Debug)]
pub struct ComposedPlanningSolution {
    entities: ComposedPlanningSchema,
    members: Vec<CompiledSolutionMember>,
    indices: HashMap<String, usize>,
}
impl SolutionComposition {
    pub fn new(
        components: Vec<PlanningSolutionComponent>,
    ) -> Result<Self, SolutionCompositionError> {
        let mut indices = HashMap::new();
        for (i, c) in components.iter().enumerate() {
            if indices.insert(c.component, i).is_some() {
                return Err(SolutionCompositionError::DuplicateComponent(c.component));
            }
        }
        Ok(Self {
            components,
            indices,
        })
    }
    /// Resolve reachable declarations before any model/range/score access.
    /// Parents contribute first. Empty plain components may relay ancestry;
    /// declaring members requires registration. Duplicate names are rejected even
    /// when the replacement has the same type. One score member is required.
    ///
    /// ```
    /// use std::sync::Arc;
    /// use timetuck::*;
    /// struct Schedule; struct Parent; struct Task;
    /// let entities = PlanningComposition::new(vec![PlanningComponent::registered::<Task>(
    ///     vec![PlanningVariableDefinition::basic::<i32>("worker", false)])])?;
    /// let declarations = SolutionComposition::new(vec![
    ///     PlanningSolutionComponent::registered::<Parent>(vec![
    ///         SolutionMemberDefinition::entity_collection::<Task>("tasks"),
    ///         SolutionMemberDefinition::value_range::<i32>("workers", "workerRange"),
    ///         SolutionMemberDefinition::score::<SimpleScore>("score")]),
    ///     PlanningSolutionComponent::plain::<Schedule>(vec![]).with_parent::<Parent>()])?;
    /// let layout = declarations.compile::<Schedule>(&entities, &[PlanningType::of::<Task>()],
    ///     SolutionCompositionPolicy::default(), PlanningCompositionPolicy::default())?;
    /// let bound = layout.bind_basic(
    ///     vec![SolutionCollection { member: "tasks".into(), entities: vec![
    ///         SolutionEntityRecord::new(Arc::new(Task), vec![("worker".into(), None)])] }],
    ///     vec![SolutionRange { member: "workers".into(), values: vec![1, 2] }],
    ///     vec![SolutionFieldRange::new::<Task>("worker", vec!["workerRange".into()])])?;
    /// assert_eq!(bound.problem().variables()[0].domain(), &[1, 2]);
    /// assert_eq!(bound.collection("tasks"), Some([0].as_slice()));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn compile<S: 'static>(
        &self,
        entities: &PlanningComposition,
        entity_roots: &[PlanningType],
        solution_policy: SolutionCompositionPolicy,
        entity_policy: PlanningCompositionPolicy,
    ) -> Result<ComposedPlanningSolution, SolutionCompositionError> {
        let root = PlanningType::of::<S>();
        let mut order = vec![];
        let mut seen = HashSet::new();
        let mut next = Some(root);
        while let Some(component) = next {
            if !seen.insert(component) {
                return Err(SolutionCompositionError::Cycle(component));
            }
            let &index = self
                .indices
                .get(&component)
                .ok_or(SolutionCompositionError::UnknownComponent(component))?;
            let c = &self.components[index];
            if !c.registered && !c.members.is_empty() {
                let mut names: Vec<_> = c.members.iter().map(|m| m.name.clone()).collect();
                names.sort();
                return Err(SolutionCompositionError::UnregisteredMembers { component, names });
            }
            order.push(index);
            next = c.parent;
        }
        if let Some(max) = solution_policy
            .maximum_depth
            .filter(|m| order.len() > m.get())
        {
            return Err(SolutionCompositionError::DepthLimit {
                solution: root,
                actual: order.len(),
                maximum: max.get(),
            });
        }
        // Signature/shape errors do not depend on entity registration and must
        // fail before constructing any model or invoking a member callback.
        for &index in &order {
            for member in &self.components[index].members {
                if !member.parameters.is_empty() {
                    return Err(SolutionCompositionError::MemberParameters {
                        member: member.name.clone(),
                        parameters: member.parameters.clone(),
                    });
                }
                let collection = matches!(
                    member.kind,
                    SolutionMemberKind::Entities {
                        singleton: false,
                        ..
                    } | SolutionMemberKind::Facts {
                        singleton: false,
                        ..
                    } | SolutionMemberKind::ValueRange { .. }
                );
                if collection && member.shape == SolutionMemberShape::Scalar {
                    return Err(SolutionCompositionError::CollectionShape {
                        member: member.name.clone(),
                        shape: member.shape,
                    });
                }
                if member.fact_collection
                    && !matches!(
                        member.kind,
                        SolutionMemberKind::ValueRange { .. }
                            | SolutionMemberKind::Facts {
                                singleton: false,
                                ..
                            }
                    )
                {
                    return Err(SolutionCompositionError::MemberRoleConflict(
                        member.name.clone(),
                    ));
                }
            }
        }
        let entity_schema = entities
            .compile::<S>(entity_roots, entity_policy)
            .map_err(|e| SolutionCompositionError::Entities(Box::new(e)))?;
        let mut members: Vec<CompiledSolutionMember> = vec![];
        let mut indices: HashMap<String, usize> = HashMap::new();
        let mut providers = HashSet::new();
        let mut score: Option<String> = None;
        for index in order.into_iter().rev() {
            let c = &self.components[index];
            for member in &c.members {
                if member.name.is_empty() {
                    return Err(SolutionCompositionError::EmptyMemberName(c.component));
                }
                if let Some(&original) = indices.get(&member.name) {
                    return Err(SolutionCompositionError::DuplicateMember {
                        name: member.name.clone(),
                        original: members[original].declaring_type,
                        redefining: c.component,
                        kind: members[original].declaration.kind.category(),
                        replacement: member.kind.category(),
                    });
                }
                match &member.kind {
                    SolutionMemberKind::Facts { allowed_types, .. } => {
                        if allowed_types.iter().collect::<HashSet<_>>().len() != allowed_types.len()
                        {
                            return Err(SolutionCompositionError::InvalidFactTypes(
                                member.name.clone(),
                            ));
                        }
                        for &t in allowed_types {
                            if entity_schema.schema().entity_type(t).is_ok() {
                                return Err(SolutionCompositionError::FactIsEntity {
                                    member: member.name.clone(),
                                    entity: t,
                                });
                            }
                        }
                    }
                    SolutionMemberKind::Entities { allowed_types, .. } => {
                        let unique: HashSet<_> = allowed_types.iter().collect();
                        if allowed_types.is_empty() || unique.len() != allowed_types.len() {
                            return Err(SolutionCompositionError::InvalidEntityTypes(
                                member.name.clone(),
                            ));
                        }
                        for &t in allowed_types {
                            entity_schema.schema().entity_type(t).map_err(|e| {
                                SolutionCompositionError::Entities(Box::new(
                                    PlanningCompositionError::Schema(e),
                                ))
                            })?;
                        }
                    }
                    SolutionMemberKind::ValueRange {
                        provider,
                        value_type,
                    } => {
                        if member.fact_collection
                            && entity_schema.schema().entity_type(*value_type).is_ok()
                        {
                            return Err(SolutionCompositionError::FactIsEntity {
                                member: member.name.clone(),
                                entity: *value_type,
                            });
                        }
                        if provider.is_empty() {
                            return Err(SolutionCompositionError::EmptyProvider(
                                member.name.clone(),
                            ));
                        }
                        if !providers.insert(provider.clone()) {
                            return Err(SolutionCompositionError::DuplicateProvider(
                                provider.clone(),
                            ));
                        }
                    }
                    SolutionMemberKind::Score(_) => {
                        if let Some(first) = &score {
                            return Err(SolutionCompositionError::MultipleScores {
                                first: first.clone(),
                                second: member.name.clone(),
                            });
                        }
                        score = Some(member.name.clone());
                    }
                }
                indices.insert(member.name.clone(), members.len());
                members.push(CompiledSolutionMember {
                    declaration: member.clone(),
                    declaring_type: c.component,
                });
            }
        }
        if score.is_none() {
            return Err(SolutionCompositionError::MissingScore);
        }
        Ok(ComposedPlanningSolution {
            entities: entity_schema,
            members,
            indices,
        })
    }
}
impl ComposedPlanningSolution {
    pub fn members_in(
        &self,
        category: SolutionMemberCategory,
    ) -> impl Iterator<Item = &CompiledSolutionMember> {
        self.members.iter().filter(move |m| match category {
            SolutionMemberCategory::ProblemFactCollection => m.is_problem_fact_collection(),
            _ => m.kind().category() == category,
        })
    }
    pub fn schema(&self) -> &PlanningSchema {
        self.entities.schema()
    }
    pub fn entity_composition(&self) -> &ComposedPlanningSchema {
        &self.entities
    }
    pub fn members(&self) -> &[CompiledSolutionMember] {
        &self.members
    }
    /// Indexed canonical member lookup; no ancestry traversal while solving.
    pub fn member(&self, name: &str) -> Option<&CompiledSolutionMember> {
        self.indices.get(name).map(|&i| &self.members[i])
    }
}

/// A concrete shared domain object and its genuine field values. No payload
/// equality/hash/clone is required. Arc ownership preserves allocation identity.
pub struct SolutionEntityRecord<V> {
    object: Arc<dyn Any + Send + Sync>,
    entity_type: PlanningType,
    values: Vec<(String, Option<V>)>,
}
impl<V> SolutionEntityRecord<V> {
    pub fn new<E: Any + Send + Sync>(object: Arc<E>, values: Vec<(String, Option<V>)>) -> Self {
        Self {
            object,
            entity_type: PlanningType::of::<E>(),
            values,
        }
    }
}
pub struct SolutionCollection<V> {
    pub member: String,
    pub entities: Vec<SolutionEntityRecord<V>>,
}
pub struct SolutionRange<V> {
    pub member: String,
    pub values: Vec<V>,
}
#[derive(Clone, Debug)]
pub struct SolutionFieldRange {
    pub entity_type: PlanningType,
    pub variable: String,
    pub providers: Vec<String>,
}
impl SolutionFieldRange {
    pub fn new<E: 'static>(variable: impl Into<String>, providers: Vec<String>) -> Self {
        Self {
            entity_type: PlanningType::of::<E>(),
            variable: variable.into(),
            providers,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SolutionBindingError {
    DifferentFactDescriptor,
    UnknownMember(String),
    WrongMemberKind(String),
    DuplicateBinding(String),
    MissingBinding(String),
    WrongValueType {
        member: String,
        expected: PlanningType,
        actual: PlanningType,
    },
    SingletonCardinality(String),
    WrongEntityType {
        member: String,
        entity: PlanningType,
    },
    InvalidFields(PlanningType),
    ConflictingEntityRecord(PlanningType),
    InvalidFieldRange {
        entity: PlanningType,
        variable: String,
    },
    UnknownProvider(String),
    MissingFieldRange {
        entity: PlanningType,
        variable: String,
    },
    UnsupportedVariable {
        entity: PlanningType,
        variable: String,
    },
    Model(ModelError),
}
impl fmt::Display for SolutionBindingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "solution binding: {self:?}")
    }
}
impl std::error::Error for SolutionBindingError {}
/// Stable entity-major slot bindings; distinct equal-valued allocations remain
/// separate. Aliased objects across collections contribute variables only once.
pub struct BoundSolutionEntity {
    object: Arc<dyn Any + Send + Sync>,
    entity_type: PlanningType,
    slots: Vec<(String, usize)>,
}
impl BoundSolutionEntity {
    pub fn entity_type(&self) -> PlanningType {
        self.entity_type
    }
    pub fn object(&self) -> &Arc<dyn Any + Send + Sync> {
        &self.object
    }
    pub fn object_ref<E: Any>(&self) -> Option<&E> {
        self.object.downcast_ref()
    }
    pub fn slots(&self) -> &[(String, usize)] {
        &self.slots
    }
    pub fn slot(&self, name: &str) -> Option<usize> {
        self.slots.iter().find(|(n, _)| n == name).map(|(_, s)| *s)
    }
}
pub struct BoundBasicSolution<V> {
    problem: FiniteProblem<V>,
    entities: Vec<BoundSolutionEntity>,
    collections: HashMap<String, Vec<usize>>,
    facts: Option<crate::BoundSolutionFacts>,
}
impl<V> BoundBasicSolution<V> {
    pub fn facts(&self) -> Option<&crate::BoundSolutionFacts> {
        self.facts.as_ref()
    }
    pub fn problem(&self) -> &FiniteProblem<V> {
        &self.problem
    }
    pub fn entities(&self) -> &[BoundSolutionEntity] {
        &self.entities
    }
    /// Preserve collection occurrences, including aliases, as canonical row indices.
    pub fn collection(&self, name: &str) -> Option<&[usize]> {
        self.collections.get(name).map(Vec::as_slice)
    }
    pub fn into_problem(self) -> FiniteProblem<V> {
        self.problem
    }
}

impl ComposedPlanningSolution {
    /// Attach a validated fact snapshot to the concrete basic binding. It must
    /// belong to this actual descriptor; matching only a solution type would
    /// permit snapshots from a different member/schema configuration.
    pub fn bind_basic_with_facts<V: Clone + Eq + 'static>(
        &self,
        collections: Vec<SolutionCollection<V>>,
        ranges: Vec<SolutionRange<V>>,
        field_ranges: Vec<SolutionFieldRange>,
        facts: crate::BoundSolutionFacts,
    ) -> Result<BoundBasicSolution<V>, SolutionBindingError> {
        if !std::ptr::eq(self, facts.descriptor().as_ref()) {
            return Err(SolutionBindingError::DifferentFactDescriptor);
        }
        let mut bound = self.bind_basic(collections, ranges, field_ranges)?;
        bound.facts = Some(facts);
        Ok(bound)
    }
    /// Bind explicit collection/range reads once, before search. Basic variables
    /// only; list/mixed binding and automatic object getters are separate APIs.
    /// All names/types/provider links and initial values are validated. Aliased
    /// objects must supply identical field values; their first encounter sets order.
    pub fn bind_basic<V: Clone + Eq + 'static>(
        &self,
        collections: Vec<SolutionCollection<V>>,
        ranges: Vec<SolutionRange<V>>,
        field_ranges: Vec<SolutionFieldRange>,
    ) -> Result<BoundBasicSolution<V>, SolutionBindingError> {
        let mut range_values = HashMap::new();
        let mut bound_names = HashSet::new();
        for range in ranges {
            let member = self
                .member(&range.member)
                .ok_or_else(|| SolutionBindingError::UnknownMember(range.member.clone()))?;
            let SolutionMemberKind::ValueRange {
                value_type,
                provider,
            } = member.kind()
            else {
                return Err(SolutionBindingError::WrongMemberKind(range.member));
            };
            if *value_type != PlanningType::of::<V>() {
                return Err(SolutionBindingError::WrongValueType {
                    member: range.member,
                    expected: *value_type,
                    actual: PlanningType::of::<V>(),
                });
            }
            if !bound_names.insert(range.member.clone()) {
                return Err(SolutionBindingError::DuplicateBinding(range.member));
            }
            range_values.insert(provider.clone(), range.values);
        }
        let mut rows: Vec<(SolutionEntityRecord<V>, Vec<Option<V>>)> = vec![];
        let mut objects: HashMap<usize, usize> = HashMap::new();
        let mut member_rows = HashMap::new();
        let mut counts = HashMap::new();
        for collection in collections {
            let member = self
                .member(&collection.member)
                .ok_or_else(|| SolutionBindingError::UnknownMember(collection.member.clone()))?;
            let SolutionMemberKind::Entities {
                allowed_types,
                singleton,
            } = member.kind()
            else {
                return Err(SolutionBindingError::WrongMemberKind(collection.member));
            };
            if !bound_names.insert(collection.member.clone()) {
                return Err(SolutionBindingError::DuplicateBinding(collection.member));
            }
            if *singleton && collection.entities.len() > 1 {
                return Err(SolutionBindingError::SingletonCardinality(
                    collection.member,
                ));
            }
            let mut indices = vec![];
            for record in collection.entities {
                if !allowed_types.contains(&record.entity_type) {
                    return Err(SolutionBindingError::WrongEntityType {
                        member: collection.member,
                        entity: record.entity_type,
                    });
                }
                let entity = self.schema().entity_type(record.entity_type).unwrap();
                let fields: HashMap<_, _> = record
                    .values
                    .iter()
                    .map(|(name, value)| (name.as_str(), value))
                    .collect();
                if fields.len() != record.values.len()
                    || fields.len() != entity.genuine_variables().count()
                {
                    return Err(SolutionBindingError::InvalidFields(record.entity_type));
                }
                let values = entity
                    .genuine_variables()
                    .map(|v| {
                        fields
                            .get(v.name())
                            .map(|v| (*v).clone())
                            .ok_or(SolutionBindingError::InvalidFields(record.entity_type))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let key = Arc::as_ptr(&record.object) as *const () as usize;
                if let Some(&index) = objects.get(&key) {
                    let (previous, previous_values) = &rows[index];
                    if previous.entity_type != record.entity_type || previous_values != &values {
                        return Err(SolutionBindingError::ConflictingEntityRecord(
                            record.entity_type,
                        ));
                    }
                    indices.push(index);
                } else {
                    let index = rows.len();
                    objects.insert(key, index);
                    indices.push(index);
                    *counts.entry(record.entity_type).or_insert(0usize) += 1;
                    rows.push((record, values));
                }
            }
            member_rows.insert(collection.member, indices);
        }
        for member in &self.members {
            if !matches!(
                member.kind(),
                SolutionMemberKind::Score(_) | SolutionMemberKind::Facts { .. }
            ) && !bound_names.contains(member.name())
            {
                return Err(SolutionBindingError::MissingBinding(member.name().into()));
            }
        }
        let mut managed: HashMap<(PlanningType, String), ManagedValueRanges<V>> = HashMap::new();
        for field in field_ranges {
            let error = || SolutionBindingError::InvalidFieldRange {
                entity: field.entity_type,
                variable: field.variable.clone(),
            };
            let entity = self
                .schema()
                .entity_type(field.entity_type)
                .map_err(|_| error())?;
            let variable = entity
                .genuine_variable(&field.variable)
                .map_err(|_| error())?;
            if !matches!(variable.kind(), PlanningVariableKind::Basic { .. }) {
                return Err(SolutionBindingError::UnsupportedVariable {
                    entity: field.entity_type,
                    variable: field.variable,
                });
            }
            if variable.value_type() != PlanningType::of::<V>()
                || field.providers.is_empty()
                || managed.contains_key(&(field.entity_type, field.variable.clone()))
            {
                return Err(error());
            }
            let providers = field
                .providers
                .iter()
                .map(|p| {
                    range_values
                        .get(p)
                        .cloned()
                        .ok_or_else(|| SolutionBindingError::UnknownProvider(p.clone()))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let ranges = variable
                .shared_ranges(
                    providers,
                    counts.get(&field.entity_type).copied().unwrap_or(0),
                )
                .map_err(|_| error())?;
            managed.insert((field.entity_type, field.variable), ranges);
        }
        for entity in self.schema().entities() {
            for variable in entity.genuine_variables() {
                if !managed.contains_key(&(entity.entity_type(), variable.name().into())) {
                    return Err(SolutionBindingError::MissingFieldRange {
                        entity: entity.entity_type(),
                        variable: variable.name().into(),
                    });
                }
            }
        }
        let mut variables = vec![];
        let mut bound_entities = vec![];
        let mut ordinals = HashMap::new();
        for (record, values) in rows {
            let ordinal = ordinals.entry(record.entity_type).or_insert(0usize);
            let entity = self.schema().entity_type(record.entity_type).unwrap();
            let mut slots = vec![];
            for (field, initial) in entity.genuine_variables().zip(values) {
                let mut variable = managed[&(record.entity_type, field.name().into())]
                    .variable(*ordinal)
                    .unwrap();
                if let Some(value) = initial {
                    variable = variable.with_initial(value);
                }
                slots.push((field.name().into(), variables.len()));
                variables.push(variable);
            }
            *ordinal += 1;
            bound_entities.push(BoundSolutionEntity {
                object: record.object,
                entity_type: record.entity_type,
                slots,
            });
        }
        let problem = FiniteProblem::new(variables).map_err(SolutionBindingError::Model)?;
        Ok(BoundBasicSolution {
            problem,
            entities: bound_entities,
            collections: member_rows,
            facts: None,
        })
    }
}
