//! Typed heterogeneous shadow scheduling over the native dependency cache.
use crate::{
    CompiledSourcePath, DependencyCache, DependencyError, DependencyTopology, DerivedValue,
    PlanningSchema, PlanningType, PlanningVariableKind, PlanningVariableRef, SourcePathError,
};
use std::{any::Any, collections::BTreeSet, error::Error, fmt, marker::PhantomData, rc::Rc};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ShadowSessionError {
    Metadata(&'static str),
    DuplicateNode,
    ForeignNode,
    UnknownNode,
    WrongType,
    NotDependency,
    NotCurrent,
    SnapshotMismatch,
    SourcePath(SourcePathError),
    UnregisteredSource,
    FixedDependencyCycle(Vec<String>),
    Dependency(DependencyError<usize>),
}
impl fmt::Display for ShadowSessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FixedDependencyCycle(variables) => write!(
                f,
                "fixed dependency loops for shadow variables: {}",
                variables.join(", ")
            ),
            _ => write!(f, "shadow session: {self:?}"),
        }
    }
}
impl Error for ShadowSessionError {}
impl From<DependencyError<usize>> for ShadowSessionError {
    fn from(e: DependencyError<usize>) -> Self {
        Self::Dependency(e)
    }
}
#[derive(Clone)]
pub struct ShadowNodeRef {
    index: usize,
    identity: Rc<()>,
}
pub struct ShadowNode<T> {
    node: ShadowNodeRef,
    marker: PhantomData<fn() -> T>,
}
impl<T> Clone for ShadowNode<T> {
    fn clone(&self) -> Self {
        Self {
            node: self.node.clone(),
            marker: PhantomData,
        }
    }
}
impl<T> ShadowNode<T> {
    pub fn erase(&self) -> ShadowNodeRef {
        self.node.clone()
    }
}
#[derive(Clone)]
struct Stored {
    value: Rc<dyn Any>,
    equal: fn(&dyn Any, &dyn Any) -> bool,
}
impl Stored {
    fn new<T: Eq + 'static>(value: T) -> Self {
        Self {
            value: Rc::new(value),
            equal: |a, b| a.downcast_ref::<T>() == b.downcast_ref::<T>(),
        }
    }
    fn get<T: 'static>(&self) -> Result<&T, ShadowSessionError> {
        self.value
            .downcast_ref()
            .ok_or(ShadowSessionError::WrongType)
    }
}
impl PartialEq for Stored {
    fn eq(&self, other: &Self) -> bool {
        self.value.as_ref().type_id() == other.value.as_ref().type_id()
            && (self.equal)(self.value.as_ref(), other.value.as_ref())
    }
}
impl Eq for Stored {}
pub struct ShadowRead<'a, T> {
    pub value: Option<&'a T>,
    pub inconsistent: bool,
}
pub struct ShadowInputs<'a> {
    identity: &'a Rc<()>,
    values: &'a [(&'a usize, &'a DerivedValue<Stored>)],
    fields: &'a [PlanningVariableRef<'a>],
}
impl ShadowInputs<'_> {
    /// Read the registered dependencies for a canonical field, in registration
    /// order. Repeated references to one node contribute once; path traversal
    /// multiplicity is not a second cached field value.
    pub fn for_field<T: 'static>(
        &self,
        field: PlanningVariableRef<'_>,
    ) -> Result<Vec<Option<&T>>, ShadowSessionError> {
        if self.fields.first().is_some_and(|registered| {
            !std::ptr::eq(registered.entity().solution(), field.entity().solution())
        }) {
            return Err(ShadowSessionError::Metadata(
                "field belongs to another schema",
            ));
        }
        if !matches!(field.kind(), PlanningVariableKind::List { .. })
            && field.value_type() != PlanningType::of::<T>()
        {
            return Err(ShadowSessionError::WrongType);
        }
        self.values
            .iter()
            .filter(|(index, _)| self.fields[**index] == field)
            .map(|(_, value)| value.value.as_ref().map(Stored::get).transpose())
            .collect()
    }
    /// Only declared incoming dependencies can be read. None is a consistent
    /// absent value; inconsistent dependencies suppress the entire supplier.
    pub fn get<T: 'static>(&self, node: &ShadowNode<T>) -> Result<Option<&T>, ShadowSessionError> {
        if !Rc::ptr_eq(self.identity, &node.node.identity) {
            return Err(ShadowSessionError::ForeignNode);
        }
        let value = self
            .values
            .iter()
            .find(|(index, _)| **index == node.node.index)
            .ok_or(ShadowSessionError::NotDependency)?
            .1;
        value.value.as_ref().map(Stored::get).transpose()
    }
}
type Reader<'a, W> = dyn Fn(&W) -> Option<Stored> + 'a;
type Supplier<'a, W> = dyn for<'c> FnMut(&W, ShadowInputs<'c>) -> Option<Stored> + 'a;
enum Operation<'a, W> {
    Input(Box<Reader<'a, W>>),
    Derived(Box<Supplier<'a, W>>),
}
type SourceResolver<'a, 's, K, W> =
    dyn Fn(&W) -> Result<Vec<(PlanningVariableRef<'s>, K)>, ShadowSessionError> + 'a;
struct Node<'a, 's, K, W> {
    variable: PlanningVariableRef<'s>,
    entity: K,
    operation: Operation<'a, W>,
    sources: Option<Box<SourceResolver<'a, 's, K, W>>>,
    fixed_sources: Vec<PlanningVariableRef<'s>>,
}
/// Explicit typed field registration; native entity keys identify model objects.
/// Metadata is borrowed, and handles cannot be used with a different builder/session.
pub struct ShadowSessionBuilder<'a, 's, K, W> {
    schema: &'s PlanningSchema,
    identity: Rc<()>,
    nodes: Vec<Node<'a, 's, K, W>>,
    edges: BTreeSet<(usize, usize)>,
}
impl<'a, 's, K: Eq, W> ShadowSessionBuilder<'a, 's, K, W> {
    pub fn new(schema: &'s PlanningSchema) -> Self {
        Self {
            schema,
            identity: Rc::new(()),
            nodes: vec![],
            edges: BTreeSet::new(),
        }
    }
    fn register<T: 'static>(
        &mut self,
        variable: PlanningVariableRef<'s>,
        entity: K,
        operation: Operation<'a, W>,
    ) -> Result<ShadowNode<T>, ShadowSessionError> {
        if !std::ptr::eq(variable.entity().solution(), self.schema) {
            return Err(ShadowSessionError::Metadata(
                "field belongs to another schema",
            ));
        }
        if self
            .nodes
            .iter()
            .any(|n| n.variable == variable && n.entity == entity)
        {
            return Err(ShadowSessionError::DuplicateNode);
        }
        let node = ShadowNode {
            node: ShadowNodeRef {
                index: self.nodes.len(),
                identity: self.identity.clone(),
            },
            marker: PhantomData,
        };
        self.nodes.push(Node {
            variable,
            entity,
            operation,
            sources: None,
            fixed_sources: vec![],
        });
        Ok(node)
    }
    /// Read a basic variable or an externally maintained shadow. All readers
    /// run once per update; equality
    /// determines which downstream suppliers need recalculation.
    pub fn input<T: Eq + 'static>(
        &mut self,
        variable: PlanningVariableRef<'s>,
        entity: K,
        read: impl Fn(&W) -> Option<T> + 'a,
    ) -> Result<ShadowNode<T>, ShadowSessionError> {
        if matches!(variable.kind(), PlanningVariableKind::List { .. })
            || variable.value_type() != PlanningType::of::<T>()
        {
            return Err(ShadowSessionError::WrongType);
        }
        self.register(
            variable,
            entity,
            Operation::Input(Box::new(move |w| read(w).map(Stored::new))),
        )
    }
    /// Planning-list metadata records its element type; the native input is Vec<T>.
    pub fn list_input<T: Eq + 'static>(
        &mut self,
        variable: PlanningVariableRef<'s>,
        entity: K,
        read: impl Fn(&W) -> Vec<T> + 'a,
    ) -> Result<ShadowNode<Vec<T>>, ShadowSessionError> {
        if !matches!(variable.kind(), PlanningVariableKind::List { .. })
            || variable.value_type() != PlanningType::of::<T>()
        {
            return Err(ShadowSessionError::WrongType);
        }
        self.register(
            variable,
            entity,
            Operation::Input(Box::new(move |w| Some(Stored::new(read(w))))),
        )
    }
    pub fn derive<T: Eq + 'static>(
        &mut self,
        variable: PlanningVariableRef<'s>,
        entity: K,
        mut supplier: impl for<'c> FnMut(&W, ShadowInputs<'c>) -> Option<T> + 'a,
    ) -> Result<ShadowNode<T>, ShadowSessionError> {
        if variable.kind() != PlanningVariableKind::Shadow
            || variable.value_type() != PlanningType::of::<T>()
        {
            return Err(ShadowSessionError::WrongType);
        }
        self.register(
            variable,
            entity,
            Operation::Derived(Box::new(move |w, i| supplier(w, i).map(Stored::new))),
        )
    }
    /// Resolve dependencies directly from native working state on every update.
    /// Register all possible source nodes before binding. Returned handles must
    /// belong to this builder and have been registered at binding time. This is
    /// useful for list predecessors and application links without object getters.
    /// Resolver errors occur before readers/suppliers; cycles remain runtime
    /// states. Do not mutate captured facts or the supplied world in a resolver.
    /// Manual edges and compiled paths cannot also control the same target.
    pub fn bind_dependencies<T>(
        &mut self,
        target: &ShadowNode<T>,
        resolve: impl Fn(&W) -> Result<Vec<ShadowNodeRef>, ShadowSessionError> + 'a,
    ) -> Result<(), ShadowSessionError>
    where
        K: Clone + 'a,
        's: 'a,
    {
        let index = lookup(&self.identity, self.nodes.len(), &target.node)?;
        if matches!(self.nodes[index].operation, Operation::Input(_))
            || self.nodes[index].sources.is_some()
            || self.edges.iter().any(|(target, _)| *target == index)
        {
            return Err(ShadowSessionError::Metadata(
                "target already bound or is an input",
            ));
        }
        let identity = self.identity.clone();
        let fields = self
            .nodes
            .iter()
            .map(|node| (node.variable, node.entity.clone()))
            .collect::<Vec<_>>();
        self.nodes[index].sources = Some(Box::new(move |world| {
            resolve(world)?
                .iter()
                .map(|node| {
                    let source = lookup(&identity, fields.len(), node)?;
                    Ok(fields[source].clone())
                })
                .collect()
        }));
        Ok(())
    }

    /// Resolve these compiled paths on every update. The root and every source
    /// object must map to a registered stable entity key. All referenced fields,
    /// including built-in invalidation fields, must be registered as nodes.
    /// Explicit dependencies and automatic paths cannot control the same target.
    pub fn bind_source_paths<T, R: 'static>(
        &mut self,
        target: &ShadowNode<T>,
        paths: Vec<&'a CompiledSourcePath<'a, 's>>,
        root: impl for<'w> Fn(&'w W) -> &'w R + 'a,
        identify: impl Fn(&dyn Any) -> Option<K> + 'a,
    ) -> Result<(), ShadowSessionError>
    where
        K: Clone + 'a,
        's: 'a,
    {
        let index = lookup(&self.identity, self.nodes.len(), &target.node)?;
        let node = &self.nodes[index];
        if matches!(node.operation, Operation::Input(_))
            || node.sources.is_some()
            || self.edges.iter().any(|(target, _)| *target == index)
        {
            return Err(ShadowSessionError::Metadata(
                "target already bound or is an input",
            ));
        }
        if paths.is_empty()
            || paths
                .iter()
                .any(|p| p.target() != node.variable || p.root_type() != PlanningType::of::<R>())
        {
            return Err(ShadowSessionError::Metadata(
                "source path target/root differs from node",
            ));
        }
        let fixed_sources = paths
            .iter()
            .flat_map(|path| path.references())
            .filter(|reference| reference.on_root())
            .map(|reference| reference.variable())
            .collect();
        let expected = node.entity.clone();
        self.nodes[index].fixed_sources = fixed_sources;
        self.nodes[index].sources = Some(Box::new(move |world| {
            let root = root(world);
            if identify(root).as_ref() != Some(&expected) {
                return Err(ShadowSessionError::Metadata(
                    "source root has a different entity identity",
                ));
            }
            let mut resolved = vec![];
            for path in &paths {
                for reference in path.references() {
                    for entity in reference
                        .entities(root)
                        .map_err(ShadowSessionError::SourcePath)?
                    {
                        let key = identify(entity).ok_or(ShadowSessionError::UnregisteredSource)?;
                        resolved.push((reference.variable(), key));
                    }
                }
            }
            Ok(resolved)
        }));
        Ok(())
    }
    pub fn set_dependencies<T>(
        &mut self,
        target: &ShadowNode<T>,
        sources: &[ShadowNodeRef],
    ) -> Result<(), ShadowSessionError> {
        replace_dependencies(
            &self.identity,
            &self.nodes,
            &mut self.edges,
            &target.node,
            sources,
        )
    }
    /// Reject source-path cycles that cannot be broken by changing a world.
    /// References owned by the root always address this node's stable entity
    /// key. Links through previous elements, groups and other objects remain
    /// dynamic and may become inconsistent at runtime instead. Manual edges are
    /// intentionally mutable and do not participate in this fixed-path check.
    /// Validation invokes no root accessors, readers or suppliers.
    pub fn validate_fixed_dependencies(&self) -> Result<(), ShadowSessionError> {
        let mut topology =
            DependencyTopology::new(0..self.nodes.len()).map_err(ShadowSessionError::Dependency)?;
        let mut edges = vec![];
        for (target, node) in self.nodes.iter().enumerate() {
            for field in &node.fixed_sources {
                let source = self
                    .nodes
                    .iter()
                    .position(|candidate| {
                        candidate.variable == *field && candidate.entity == node.entity
                    })
                    .ok_or(ShadowSessionError::UnregisteredSource)?;
                edges.push((target, source));
            }
        }
        topology
            .update(&[], &edges)
            .map_err(ShadowSessionError::Dependency)?;
        let mut variables = BTreeSet::new();
        for (index, node) in self.nodes.iter().enumerate() {
            if topology
                .is_cyclic(&index)
                .map_err(ShadowSessionError::Dependency)?
            {
                variables.insert(node.variable.name().to_owned());
            }
        }
        if variables.is_empty() {
            Ok(())
        } else {
            Err(ShadowSessionError::FixedDependencyCycle(
                variables.into_iter().collect(),
            ))
        }
    }
    /// Build a session after rejecting statically guaranteed source-path cycles.
    pub fn try_build(self) -> Result<ShadowSession<'a, 's, K, W>, ShadowSessionError> {
        self.validate_fixed_dependencies()?;
        Ok(self.build())
    }
    /// Build a low-level session permitting inconsistent fixed cycles. Use
    /// try_build for model validation, as the stream scoring integration does.
    pub fn build(self) -> ShadowSession<'a, 's, K, W> {
        let topology = DependencyTopology::new(0..self.nodes.len()).expect("unique node indices");
        let pending = (0..self.nodes.len()).collect();
        ShadowSession {
            identity: self.identity,
            nodes: self.nodes,
            edges: self.edges,
            applied: BTreeSet::new(),
            manual_applied: BTreeSet::new(),
            resolved: vec![],
            pending,
            cache: DependencyCache::new(topology, |_, _| None),
        }
    }
}
fn lookup(
    identity: &Rc<()>,
    len: usize,
    node: &ShadowNodeRef,
) -> Result<usize, ShadowSessionError> {
    if !Rc::ptr_eq(identity, &node.identity) {
        return Err(ShadowSessionError::ForeignNode);
    }
    if node.index >= len {
        return Err(ShadowSessionError::UnknownNode);
    }
    Ok(node.index)
}
fn replace_dependencies<K, W>(
    identity: &Rc<()>,
    nodes: &[Node<'_, '_, K, W>],
    edges: &mut BTreeSet<(usize, usize)>,
    target: &ShadowNodeRef,
    sources: &[ShadowNodeRef],
) -> Result<(), ShadowSessionError> {
    let index = lookup(identity, nodes.len(), target)?;
    if nodes[index].sources.is_some() {
        return Err(ShadowSessionError::Metadata(
            "compiled paths control this target",
        ));
    }
    if matches!(nodes[index].operation, Operation::Input(_)) {
        return Err(ShadowSessionError::Metadata(
            "input readers cannot have dependencies",
        ));
    }
    let replacements = sources
        .iter()
        .map(|s| lookup(identity, nodes.len(), s).map(|n| (index, n)))
        .collect::<Result<BTreeSet<_>, _>>()?;
    edges.retain(|(target, _)| *target != index);
    edges.extend(replacements);
    Ok(())
}
/// A fixed set of heterogeneous typed fields with mutable dependency edges.
///
/// Input readers sample the latest caller-owned world. Changed inputs propagate
/// through registered dependencies in topological order; equal results stop
/// propagation. Each affected supplier runs at most once per flush. Cycles and
/// their descendants receive absent, inconsistent results without supplier calls.
/// Explicit dependencies may be rewired between updates, including cycle recovery.
///
/// Compiled source paths can instead resolve incoming dependencies from each world
/// snapshot. Paths validate canonical target/root types and stable object identity.
/// Every referenced variable/entity must be registered. Resolution finishes before
/// readers or suppliers run. Ordered source signatures retain duplicate/group order
/// changes even when graph edges are unchanged; graph edges remain deduplicated.
/// Restore validates these signatures as well as input values. Registration remains
/// explicit, and all bound roots must remain present in the fixed node universe.
///
/// Suppliers must read declared inputs and immutable world facts. Changes to facts
/// not represented by input readers require mark_dirty on their direct consumers.
/// Passing W supports solution-parameter suppliers without reflection. Do not
/// mutate W or captured model state inside callbacks. Reader/supplier panic leaves
/// cached values and pending work intact; external callback effects are not undone.
///
/// Reads expose the last successfully published snapshot, not a live view of W.
/// Update before reading after world edits. Pending graph edits/dirty nodes reject
/// reads. Checkpoints share non-Clone values and restore after registered inputs
/// match again, invoking readers for validation but no derived suppliers. The
/// session does not mutate assignments or apply Java setters automatically.
///
/// ```
/// use timetuck::*;
/// struct Solution; struct Entity;
/// let schema=PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Entity>(vec![
///     PlanningVariableDefinition::basic::<i64>("input",true),
///     PlanningVariableDefinition::shadow::<String>("label")])])?;
/// let entity=schema.entity::<Entity>()?;
/// let mut builder=ShadowSessionBuilder::<usize,i64>::new(&schema);
/// let input=builder.input(entity.variable("input")?,0,|value|Some(*value))?;
/// let source=input.clone();
/// let label=builder.derive(entity.variable("label")?,0,move |_,values| {
///     values.get(&source).unwrap().map(|value|value.to_string())
/// })?;
/// builder.set_dependencies(&label,&[input.erase()])?;
/// let mut session=builder.build();
/// session.update(&42)?;
/// assert_eq!(session.read(&label)?.value.map(String::as_str),Some("42"));
/// # Ok::<(),Box<dyn std::error::Error>>(())
/// ```
pub struct ShadowSession<'a, 's, K, W> {
    identity: Rc<()>,
    nodes: Vec<Node<'a, 's, K, W>>,
    edges: BTreeSet<(usize, usize)>,
    applied: BTreeSet<(usize, usize)>,
    manual_applied: BTreeSet<(usize, usize)>,
    resolved: Vec<Option<Vec<usize>>>,
    pending: BTreeSet<usize>,
    cache: DependencyCache<usize, Stored>,
}
#[derive(Clone)]
pub struct ShadowSessionCheckpoint {
    identity: Rc<()>,
    cache: DependencyCache<usize, Stored>,
    edges: BTreeSet<(usize, usize)>,
    manual_edges: BTreeSet<(usize, usize)>,
    requested_edges: BTreeSet<(usize, usize)>,
    pending: BTreeSet<usize>,
    resolved: Vec<Option<Vec<usize>>>,
}
impl<K: Eq, W> ShadowSession<'_, '_, K, W> {
    pub fn variable(
        &self,
        node: &ShadowNodeRef,
    ) -> Result<PlanningVariableRef<'_>, ShadowSessionError> {
        Ok(self.nodes[lookup(&self.identity, self.nodes.len(), node)?].variable)
    }
    pub fn entity(&self, node: &ShadowNodeRef) -> Result<&K, ShadowSessionError> {
        Ok(&self.nodes[lookup(&self.identity, self.nodes.len(), node)?].entity)
    }
    pub fn set_dependencies<T>(
        &mut self,
        target: &ShadowNode<T>,
        sources: &[ShadowNodeRef],
    ) -> Result<(), ShadowSessionError> {
        replace_dependencies(
            &self.identity,
            &self.nodes,
            &mut self.edges,
            &target.node,
            sources,
        )
    }
    pub fn mark_dirty<T>(&mut self, node: &ShadowNode<T>) -> Result<(), ShadowSessionError> {
        self.pending
            .insert(lookup(&self.identity, self.nodes.len(), &node.node)?);
        Ok(())
    }
    fn ready(&self) -> Result<(), ShadowSessionError> {
        if self.pending.is_empty() && self.edges == self.manual_applied {
            Ok(())
        } else {
            Err(ShadowSessionError::NotCurrent)
        }
    }
    pub fn read<T: 'static>(
        &self,
        node: &ShadowNode<T>,
    ) -> Result<ShadowRead<'_, T>, ShadowSessionError> {
        let index = lookup(&self.identity, self.nodes.len(), &node.node)?;
        self.ready()?;
        let value = &self.cache.values()[&index];
        Ok(ShadowRead {
            value: value.value.as_ref().map(Stored::get).transpose()?,
            inconsistent: value.inconsistent,
        })
    }
    fn resolve_sources(
        &self,
        world: &W,
        manual: &BTreeSet<(usize, usize)>,
    ) -> Result<ResolvedSources, ShadowSessionError> {
        let mut edges = manual.clone();
        let mut signatures = vec![];
        for (index, node) in self.nodes.iter().enumerate() {
            let signature = if let Some(resolve) = &node.sources {
                let mut signature = vec![];
                for (variable, entity) in resolve(world)? {
                    let source = self
                        .nodes
                        .iter()
                        .position(|n| n.variable == variable && n.entity == entity)
                        .ok_or(ShadowSessionError::UnregisteredSource)?;
                    edges.insert((index, source));
                    signature.push(source);
                }
                Some(signature)
            } else {
                None
            };
            signatures.push(signature);
        }
        Ok(ResolvedSources { edges, signatures })
    }
    /// Return handles for changed derived fields, including inconsistency changes.
    pub fn update(&mut self, world: &W) -> Result<Vec<ShadowNodeRef>, ShadowSessionError> {
        let resolved = self.resolve_sources(world, &self.edges)?;
        self.update_resolved(world, resolved)
    }
    fn update_resolved(
        &mut self,
        world: &W,
        resolved: ResolvedSources,
    ) -> Result<Vec<ShadowNodeRef>, ShadowSessionError> {
        let prepared = self
            .nodes
            .iter()
            .map(|n| match &n.operation {
                Operation::Input(read) => read(world),
                Operation::Derived(_) => None,
            })
            .collect::<Vec<_>>();
        let mut dirty = self.pending.clone();
        for (index, signature) in resolved.signatures.iter().enumerate() {
            if signature.is_some() && self.resolved.get(index) != Some(signature) {
                dirty.insert(index);
            }
        }
        for (index, node) in self.nodes.iter().enumerate() {
            if matches!(node.operation, Operation::Input(_))
                && self.cache.values()[&index].value != prepared[index]
            {
                dirty.insert(index);
            }
        }
        let removals = self
            .applied
            .difference(&resolved.edges)
            .copied()
            .collect::<Vec<_>>();
        let additions = resolved
            .edges
            .difference(&self.applied)
            .copied()
            .collect::<Vec<_>>();
        let fields = self
            .nodes
            .iter()
            .map(|node| node.variable)
            .collect::<Vec<_>>();
        let nodes = &mut self.nodes;
        let identity = &self.identity;
        // Prepare potentially allocating publication data before installing a new cache.
        let manual_applied = self.edges.clone();
        let applied = resolved.edges;
        let mut next = self.cache.clone();
        let changed = next.update(
            &removals,
            &additions,
            &dirty.into_iter().collect::<Vec<_>>(),
            |index, values| match &mut nodes[*index].operation {
                Operation::Input(_) => prepared[*index].clone(),
                Operation::Derived(supplier) => supplier(
                    world,
                    ShadowInputs {
                        identity,
                        values,
                        fields: &fields,
                    },
                ),
            },
        )?;
        let changed = changed
            .into_iter()
            .filter(|&i| matches!(nodes[i].operation, Operation::Derived(_)))
            .map(|index| ShadowNodeRef {
                index,
                identity: identity.clone(),
            })
            .collect();
        self.cache = next;
        self.applied = applied;
        self.manual_applied = manual_applied;
        self.resolved = resolved.signatures;
        self.pending.clear();
        Ok(changed)
    }
    pub fn checkpoint(&self) -> Result<ShadowSessionCheckpoint, ShadowSessionError> {
        self.ready()?;
        Ok(self.capture())
    }
    pub(crate) fn capture(&self) -> ShadowSessionCheckpoint {
        ShadowSessionCheckpoint {
            identity: self.identity.clone(),
            cache: self.cache.clone(),
            edges: self.applied.clone(),
            manual_edges: self.manual_applied.clone(),
            requested_edges: self.edges.clone(),
            pending: self.pending.clone(),
            resolved: self.resolved.clone(),
        }
    }
    pub fn restore(
        &mut self,
        saved: &ShadowSessionCheckpoint,
        world: &W,
    ) -> Result<(), ShadowSessionError> {
        if !Rc::ptr_eq(&self.identity, &saved.identity) {
            return Err(ShadowSessionError::ForeignNode);
        }
        let resolved = self.resolve_sources(world, &saved.manual_edges)?;
        if resolved.edges != saved.edges || resolved.signatures != saved.resolved {
            return Err(ShadowSessionError::SnapshotMismatch);
        }
        for (index, node) in self.nodes.iter().enumerate() {
            if let Operation::Input(read) = &node.operation
                && read(world) != saved.cache.values()[&index].value
            {
                return Err(ShadowSessionError::SnapshotMismatch);
            }
        }
        self.install(saved);
        Ok(())
    }
    // Internal graph snapshots already own the matching source world. Restoring
    // these must neither sample input readers nor invoke derived suppliers.
    pub(crate) fn install(&mut self, saved: &ShadowSessionCheckpoint) {
        assert!(Rc::ptr_eq(&self.identity, &saved.identity));
        self.cache = saved.cache.clone();
        self.edges = saved.requested_edges.clone();
        self.applied = saved.edges.clone();
        self.manual_applied = saved.manual_edges.clone();
        self.resolved = saved.resolved.clone();
        self.pending = saved.pending.clone();
    }
}

struct ResolvedSources {
    edges: BTreeSet<(usize, usize)>,
    signatures: Vec<Option<Vec<usize>>>,
}

#[path = "shadow_diagnostics.rs"]
mod diagnostics;
pub use diagnostics::{
    ConsistentShadowUpdateError, ShadowConsistencyPolicy, ShadowConsistencyReport,
    ShadowVariableKey, shadow_consistency, update_consistent_shadows, update_shadows_with_policy,
};

pub use diagnostics::{ShadowSourceReference, missing_shadow_sources};
