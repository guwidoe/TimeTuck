//! Resolve typed planning metadata into executable basic, list and sublist neighborhoods.
use crate::*;
use std::{collections::BTreeSet, error::Error, fmt};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SchemaMoveKind {
    Change,
    Swap,
    ListChange,
    ListSwap,
    SubListChange(SubListChangeConfig),
    SubListSwap(SubListSwapConfig),
}
impl SchemaMoveKind {
    fn list_only(self) -> bool {
        !matches!(self, Self::Change | Self::Swap)
    }
    fn accepts_secondary(self) -> bool {
        matches!(self, Self::Swap | Self::ListSwap | Self::SubListSwap(_))
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SchemaMoveBranchKind {
    BasicChange,
    BasicSwap,
    ListChange,
    ListSwap,
    SubListChange(SubListChangeConfig),
    SubListSwap(SubListSwapConfig),
}
/// Select entity types and genuine variable names. Empty names select all genuine
/// variables; shadow variables never form neighborhoods. List-only kinds ignore
/// basic fields during deduction and reject explicitly selected basic fields.
/// Sublist configurations retain their bounds, reversal and unassignment policy.
/// Selection preserves schema declaration order. Random order interleaves finite
/// child neighborhoods; it does
/// not promise Java random traces or random ordering inside a child.
///
/// ```
/// use timetuck::*;
/// struct Schedule; struct Task;
/// let schema = PlanningSchema::new::<Schedule>(vec![PlanningEntityDefinition::of::<Task>(
///     vec![PlanningVariableDefinition::basic::<i32>("worker", false)])])?;
/// let field = schema.genuine_entity::<Task>()?.genuine_variable("worker")?;
/// let plan = SchemaMoveConfig::new(SchemaMoveKind::Change).resolve(&schema)?;
/// let provider = plan.bind(&[SchemaVariableSlots { variable: field, slots: vec![0] }])?;
/// let problem = FiniteProblem::new(vec![Variable::new(vec![0i32, 1]).with_initial(0)])?;
/// let assignment = problem.initial_assignment();
/// assert_eq!(provider.try_basic_moves(&problem, &assignment, 0)?.count(), 1);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug)]
pub struct SchemaMoveConfig {
    pub kind: SchemaMoveKind,
    pub entity: Option<PlanningType>,
    pub secondary_entity: Option<PlanningType>,
    pub variables: Vec<String>,
    pub order: UnionOrder,
    pub selected_count_limit: Option<usize>,
}
impl SchemaMoveConfig {
    pub fn new(kind: SchemaMoveKind) -> Self {
        Self {
            kind,
            entity: None,
            secondary_entity: None,
            variables: Vec::new(),
            order: UnionOrder::Sequential,
            selected_count_limit: None,
        }
    }
    pub fn resolve<'s>(
        &self,
        schema: &'s PlanningSchema,
    ) -> Result<SchemaMovePlan<'s>, SchemaMoveError> {
        if !self.kind.accepts_secondary() && self.secondary_entity.is_some() {
            return Err(SchemaMoveError::UnexpectedSecondaryEntity);
        }
        if let (Some(a), Some(b)) = (self.entity, self.secondary_entity)
            && a != b
        {
            return Err(SchemaMoveError::IncompatibleEntities { left: a, right: b });
        }
        let mut names = BTreeSet::new();
        for name in &self.variables {
            if !names.insert(name.as_str()) {
                return Err(SchemaMoveError::DuplicateName(name.clone()));
            }
        }
        let selected = self.entity.or(self.secondary_entity);
        let entities = if let Some(entity) = selected {
            vec![
                schema
                    .entity_type(entity)
                    .map_err(SchemaMoveError::Schema)?,
            ]
        } else {
            schema.genuine_entities().collect()
        };
        let mut branches = Vec::new();
        let mut non_list_field = None;
        for entity in entities {
            let all: Vec<_> = entity.genuine_variables().collect();
            if !self
                .variables
                .iter()
                .all(|n| all.iter().any(|v| v.name() == n))
            {
                if selected.is_some() {
                    let name = self
                        .variables
                        .iter()
                        .find(|n| !all.iter().any(|v| v.name() == *n))
                        .unwrap();
                    return Err(SchemaMoveError::UnknownVariable(name.clone()));
                }
                continue;
            }
            let mut fields: Vec<_> = all
                .into_iter()
                .filter(|v| names.is_empty() || names.contains(v.name()))
                .collect();
            if self.kind.list_only() {
                if !names.is_empty()
                    && let Some(field) = fields
                        .iter()
                        .find(|v| !matches!(v.kind(), PlanningVariableKind::List { .. }))
                {
                    non_list_field.get_or_insert_with(|| field.name().to_owned());
                    continue;
                }
                fields.retain(|v| matches!(v.kind(), PlanningVariableKind::List { .. }));
            }
            let basics: Vec<_> = fields
                .iter()
                .copied()
                .filter(|v| matches!(v.kind(), PlanningVariableKind::Basic { .. }))
                .collect();
            if self.kind == SchemaMoveKind::Swap && !basics.is_empty() {
                branches.push(SchemaMoveBranch {
                    kind: SchemaMoveBranchKind::BasicSwap,
                    variables: basics,
                });
            }
            for field in fields {
                let kind = match (self.kind, field.kind()) {
                    (SchemaMoveKind::Change, PlanningVariableKind::Basic { .. }) => {
                        SchemaMoveBranchKind::BasicChange
                    }
                    (
                        SchemaMoveKind::Change | SchemaMoveKind::ListChange,
                        PlanningVariableKind::List { .. },
                    ) => SchemaMoveBranchKind::ListChange,
                    (
                        SchemaMoveKind::Swap | SchemaMoveKind::ListSwap,
                        PlanningVariableKind::List { .. },
                    ) => SchemaMoveBranchKind::ListSwap,
                    (SchemaMoveKind::SubListChange(config), PlanningVariableKind::List { .. }) => {
                        SchemaMoveBranchKind::SubListChange(config)
                    }
                    (SchemaMoveKind::SubListSwap(config), PlanningVariableKind::List { .. }) => {
                        SchemaMoveBranchKind::SubListSwap(config)
                    }
                    _ => continue,
                };
                branches.push(SchemaMoveBranch {
                    kind,
                    variables: vec![field],
                });
            }
        }
        if branches.is_empty() {
            if self.kind.list_only() {
                if let Some(name) = non_list_field {
                    return Err(SchemaMoveError::ListVariableRequired(name));
                }
                return Err(SchemaMoveError::CannotDeduceListVariable);
            }
            return Err(SchemaMoveError::NoMatchingVariables);
        }
        Ok(SchemaMovePlan {
            schema,
            branches,
            order: self.order,
            selected_count_limit: self.selected_count_limit,
        })
    }
}
#[derive(Clone, Debug)]
pub struct SchemaMoveBranch<'s> {
    pub kind: SchemaMoveBranchKind,
    pub variables: Vec<PlanningVariableRef<'s>>,
}
#[derive(Debug)]
pub struct SchemaMovePlan<'s> {
    schema: &'s PlanningSchema,
    branches: Vec<SchemaMoveBranch<'s>>,
    pub order: UnionOrder,
    pub selected_count_limit: Option<usize>,
}
/// Map one declared field to native variable slots (basic) or entity slots (list),
/// in entity order. Different basic columns for one type must have equal lengths.
#[derive(Clone, Debug)]
pub struct SchemaVariableSlots<'s> {
    pub variable: PlanningVariableRef<'s>,
    pub slots: Vec<usize>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SchemaMoveError {
    Schema(PlanningSchemaError),
    UnexpectedSecondaryEntity,
    IncompatibleEntities {
        left: PlanningType,
        right: PlanningType,
    },
    DuplicateName(String),
    UnknownVariable(String),
    NoMatchingVariables,
    CannotDeduceListVariable,
    ListVariableRequired(String),
    ForeignSchema,
    DuplicateBinding(String),
    MissingBinding(String),
    AliasedSlot {
        list: bool,
        slot: usize,
    },
    BindingLength(String),
    WrongModel,
    WrongValueType {
        expected: PlanningType,
        actual: PlanningType,
    },
    SlotOutOfBounds {
        list: bool,
        slot: usize,
        count: usize,
    },
    UnassignedPolicy(String),
    InvalidModel(String),
}
impl fmt::Display for SchemaMoveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CannotDeduceListVariable => f.write_str("cannot unfold list selector: a planning list variable cannot be deduced automatically"),
            Self::ListVariableRequired(name) => write!(f, "{name} is not a planning list variable; a list selector cannot be deduced automatically"),
            _ => write!(f, "schema move selection: {self:?}"),
        }
    }
}
impl Error for SchemaMoveError {}
impl<'s> SchemaMovePlan<'s> {
    pub fn branches(&self) -> &[SchemaMoveBranch<'s>] {
        &self.branches
    }
    pub fn is_union(&self) -> bool {
        self.branches.len() > 1
    }
    /// Bind once, then reuse against current assignments. Binding does not inspect
    /// or score a solution. Fallible opening validates model types, slot bounds,
    /// optional-value policy and assignment before yielding any move.
    pub fn bind(
        &self,
        bindings: &[SchemaVariableSlots<'s>],
    ) -> Result<SchemaMoveProvider, SchemaMoveError> {
        let mut basic_slots = BTreeSet::new();
        let mut list_slots = BTreeSet::new();
        for (index, binding) in bindings.iter().enumerate() {
            if !std::ptr::eq(binding.variable.entity().solution(), self.schema) {
                return Err(SchemaMoveError::ForeignSchema);
            }
            if !binding.variable.is_genuine() {
                return Err(SchemaMoveError::UnknownVariable(
                    binding.variable.name().into(),
                ));
            }
            for prior in &bindings[..index] {
                if prior.variable == binding.variable {
                    return Err(SchemaMoveError::DuplicateBinding(
                        binding.variable.name().into(),
                    ));
                }
                if prior.variable.entity() == binding.variable.entity()
                    && prior.slots.len() != binding.slots.len()
                {
                    return Err(SchemaMoveError::BindingLength(
                        binding.variable.name().into(),
                    ));
                }
            }
            let list = matches!(binding.variable.kind(), PlanningVariableKind::List { .. });
            let used = if list {
                &mut list_slots
            } else {
                &mut basic_slots
            };
            for &slot in &binding.slots {
                if !used.insert(slot) {
                    return Err(SchemaMoveError::AliasedSlot { list, slot });
                }
            }
        }
        let mut branches = Vec::new();
        for branch in &self.branches {
            let mut fields = Vec::new();
            for var in &branch.variables {
                let slots = bindings
                    .iter()
                    .find(|b| b.variable == *var)
                    .ok_or_else(|| SchemaMoveError::MissingBinding(var.name().into()))?
                    .slots
                    .clone();
                fields.push(BoundField {
                    name: var.name().into(),
                    kind: var.kind(),
                    value_type: var.value_type(),
                    slots,
                });
            }
            let swap = if branch.kind == SchemaMoveBranchKind::BasicSwap {
                let count = fields[0].slots.len();
                let rows = (0..count)
                    .map(|i| fields.iter().map(|f| f.slots[i]).collect())
                    .collect();
                Some(
                    EntitySwapProvider::new(rows, EntitySwapOrder::Unordered((0..count).collect()))
                        .map_err(|e| SchemaMoveError::InvalidModel(e.to_string()))?,
                )
            } else {
                None
            };
            branches.push(BoundBranch {
                kind: branch.kind,
                fields,
                swap,
            });
        }
        Ok(SchemaMoveProvider {
            branches,
            order: self.order,
            selected_count_limit: self.selected_count_limit,
        })
    }
}
#[derive(Clone, Debug)]
struct BoundField {
    name: String,
    kind: PlanningVariableKind,
    value_type: PlanningType,
    slots: Vec<usize>,
}
#[derive(Clone, Debug)]
struct BoundBranch {
    kind: SchemaMoveBranchKind,
    fields: Vec<BoundField>,
    swap: Option<EntitySwapProvider>,
}
/// Executable metadata-derived neighborhoods, shared by full, incremental and
/// stream scoring. Candidates are generated lazily from the current assignment.
/// Every opening validates its model. Trait-based solver use requires valid
/// bindings and panics on misuse, like the existing native move providers; use
/// try_*_moves for fallible validation before installing a provider in a solver.
#[derive(Clone, Debug)]
pub struct SchemaMoveProvider {
    branches: Vec<BoundBranch>,
    order: UnionOrder,
    selected_count_limit: Option<usize>,
}
impl BoundField {
    fn check_type<V: 'static>(&self) -> Result<(), SchemaMoveError> {
        let actual = PlanningType::of::<V>();
        if actual != self.value_type {
            Err(SchemaMoveError::WrongValueType {
                expected: self.value_type,
                actual,
            })
        } else {
            Ok(())
        }
    }
    fn check_basic<V: Eq + 'static>(&self, p: &FiniteProblem<V>) -> Result<(), SchemaMoveError> {
        self.check_type::<V>()?;
        let PlanningVariableKind::Basic { allows_unassigned } = self.kind else {
            return Err(SchemaMoveError::WrongModel);
        };
        for &slot in &self.slots {
            let v = p
                .variables()
                .get(slot)
                .ok_or(SchemaMoveError::SlotOutOfBounds {
                    list: false,
                    slot,
                    count: p.variables().len(),
                })?;
            if v.allows_unassigned() != allows_unassigned {
                return Err(SchemaMoveError::UnassignedPolicy(self.name.clone()));
            }
        }
        Ok(())
    }
    fn check_list<V: Eq + Clone + 'static>(
        &self,
        p: &ListProblem<V>,
    ) -> Result<(), SchemaMoveError> {
        self.check_type::<V>()?;
        let PlanningVariableKind::List { allows_unassigned } = self.kind else {
            return Err(SchemaMoveError::WrongModel);
        };
        if p.allows_unassigned() != allows_unassigned {
            return Err(SchemaMoveError::UnassignedPolicy(self.name.clone()));
        }
        for &slot in &self.slots {
            if slot >= p.entities().len() {
                return Err(SchemaMoveError::SlotOutOfBounds {
                    list: true,
                    slot,
                    count: p.entities().len(),
                });
            }
        }
        Ok(())
    }
}
impl BoundBranch {
    fn basic<'a, V: Eq + Clone + 'static>(
        &'a self,
        p: &'a FiniteProblem<V>,
        a: &'a Assignment<V>,
        step: u64,
    ) -> Result<Box<dyn Iterator<Item = Move<V>> + 'a>, SchemaMoveError> {
        for field in &self.fields {
            field.check_basic(p)?;
        }
        match self.kind {
            SchemaMoveBranchKind::BasicChange => {
                Ok(Box::new(crate::neighborhood::change_moves_ordered(
                    p,
                    a,
                    self.fields[0].slots.iter().copied(),
                )))
            }
            SchemaMoveBranchKind::BasicSwap => Ok(Box::new(
                self.swap
                    .as_ref()
                    .unwrap()
                    .select(p, a, step)
                    .map_err(|e| SchemaMoveError::InvalidModel(e.to_string()))?
                    .map(|c| c.movement),
            )),
            _ => Err(SchemaMoveError::WrongModel),
        }
    }
    fn list<'a, V: Eq + Clone + 'static>(
        &'a self,
        p: &'a ListProblem<V>,
        a: &'a ListAssignment<V>,
    ) -> Result<Box<dyn Iterator<Item = ListMove<V>> + 'a>, SchemaMoveError> {
        for field in &self.fields {
            field.check_list(p)?;
        }
        let order = self.fields[0].slots.iter().copied();
        match self.kind {
            SchemaMoveBranchKind::ListChange => Ok(Box::new(
                crate::list_neighborhood::changes(p, a, order.clone())
                    .chain(crate::list_neighborhood::assigns(p, a, order.clone()))
                    .chain(crate::list_neighborhood::unassigns(p, a, order)),
            )),
            SchemaMoveBranchKind::ListSwap => Ok(crate::list_neighborhood::ordered(
                p,
                a,
                Neighborhood::UniqueSwap,
                order,
            )),
            SchemaMoveBranchKind::SubListChange(config) => {
                config
                    .validate(p)
                    .map_err(|e| SchemaMoveError::InvalidModel(e.to_string()))?;
                Ok(crate::list_neighborhood::ordered(
                    p,
                    a,
                    Neighborhood::SubListChange(config),
                    order,
                ))
            }
            SchemaMoveBranchKind::SubListSwap(config) => Ok(crate::list_neighborhood::ordered(
                p,
                a,
                Neighborhood::SubListSwap(config),
                order,
            )),
            _ => Err(SchemaMoveError::WrongModel),
        }
    }
}
impl SchemaMoveProvider {
    fn finish<'a, M: 'a>(
        &self,
        children: Vec<Box<dyn Iterator<Item = M> + 'a>>,
        step: u64,
    ) -> Box<dyn Iterator<Item = M> + 'a> {
        let order = match self.order {
            UnionOrder::Random { seed } => UnionOrder::Random {
                seed: seed.wrapping_add(step),
            },
            x => x,
        };
        Box::new(
            MoveUnion::new(children.into_iter().map(|c| (c, 1)), order)
                .take(self.selected_count_limit.unwrap_or(usize::MAX)),
        )
    }
    pub fn try_basic_moves<'a, V: Eq + Clone + 'static>(
        &'a self,
        p: &'a FiniteProblem<V>,
        a: &'a Assignment<V>,
        step: u64,
    ) -> Result<Box<dyn Iterator<Item = Move<V>> + 'a>, SchemaMoveError> {
        p.validate(a)
            .map_err(|e| SchemaMoveError::InvalidModel(e.to_string()))?;
        let children = self
            .branches
            .iter()
            .map(|b| b.basic(p, a, step))
            .collect::<Result<_, _>>()?;
        Ok(self.finish(children, step))
    }
    pub fn try_list_moves<'a, V: Eq + Clone + 'static>(
        &'a self,
        p: &'a ListProblem<V>,
        a: &'a ListAssignment<V>,
        step: u64,
    ) -> Result<Box<dyn Iterator<Item = ListMove<V>> + 'a>, SchemaMoveError> {
        p.validate(a)
            .map_err(|e| SchemaMoveError::InvalidModel(e.to_string()))?;
        let children = self
            .branches
            .iter()
            .map(|b| b.list(p, a))
            .collect::<Result<_, _>>()?;
        Ok(self.finish(children, step))
    }
    pub fn try_mixed_moves<'a, B: Eq + Clone + 'static, L: Eq + Clone + 'static>(
        &'a self,
        p: &'a MixedProblem<B, L>,
        a: &'a MixedAssignment<B, L>,
        step: u64,
    ) -> Result<Box<dyn Iterator<Item = MixedMove<B, L>> + 'a>, SchemaMoveError> {
        p.validate(a)
            .map_err(|e| SchemaMoveError::InvalidModel(e.to_string()))?;
        let mut children: Vec<Box<dyn Iterator<Item = MixedMove<B, L>> + 'a>> = Vec::new();
        for branch in &self.branches {
            match branch.kind {
                SchemaMoveBranchKind::BasicChange | SchemaMoveBranchKind::BasicSwap => children
                    .push(Box::new(branch.basic(p.basic(), a.basic(), step)?.map(
                        |basic| MixedMove {
                            basic,
                            lists: ListMove::Composite(vec![]),
                        },
                    ))),
                _ => children.push(Box::new(branch.list(p.lists(), a.lists())?.map(|lists| {
                    MixedMove {
                        basic: Move::Composite(vec![]),
                        lists,
                    }
                }))),
            }
        }
        Ok(self.finish(children, step))
    }
}
impl<V: Eq + Clone + 'static> MoveProvider<FiniteProblem<V>, Assignment<V>, Move<V>>
    for SchemaMoveProvider
{
    fn moves<'a>(
        &'a mut self,
        p: &'a FiniteProblem<V>,
        a: &'a Assignment<V>,
        step: u64,
    ) -> Box<dyn Iterator<Item = Move<V>> + 'a>
    where
        Move<V>: 'a,
    {
        self.try_basic_moves(p, a, step)
            .expect("valid schema move binding")
    }
}
impl<V: Eq + Clone + 'static> MoveProvider<ListProblem<V>, ListAssignment<V>, ListMove<V>>
    for SchemaMoveProvider
{
    fn moves<'a>(
        &'a mut self,
        p: &'a ListProblem<V>,
        a: &'a ListAssignment<V>,
        step: u64,
    ) -> Box<dyn Iterator<Item = ListMove<V>> + 'a>
    where
        ListMove<V>: 'a,
    {
        self.try_list_moves(p, a, step)
            .expect("valid schema move binding")
    }
}
impl<B: Eq + Clone + 'static, L: Eq + Clone + 'static>
    MoveProvider<MixedProblem<B, L>, MixedAssignment<B, L>, MixedMove<B, L>>
    for SchemaMoveProvider
{
    fn moves<'a>(
        &'a mut self,
        p: &'a MixedProblem<B, L>,
        a: &'a MixedAssignment<B, L>,
        step: u64,
    ) -> Box<dyn Iterator<Item = MixedMove<B, L>> + 'a>
    where
        MixedMove<B, L>: 'a,
    {
        self.try_mixed_moves(p, a, step)
            .expect("valid schema move binding")
    }
}
