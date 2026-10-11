//! Typed heterogeneous shadow scheduling over the native dependency cache.
use crate::{
    CompiledSourcePath, DependencyCache, DependencyError, DependencyTopology, DerivedValue,
    PlanningSchema, PlanningType, PlanningVariableKind, PlanningVariableRef, SnapshotMap,
    SourcePathError,
};
use std::{
    any::Any, cell::OnceCell, collections::BTreeSet, error::Error, fmt, marker::PhantomData, rc::Rc,
};

mod callback_error;
mod entities;
mod keyed_index;
mod selection;
pub use callback_error::{ShadowCallbackError, ShadowCallbackStage};
use entities::EntityGroups;
use keyed_index::KeyedSourceIndex;
pub use selection::{ShadowEntityFilter, ShadowEntitySelection};

#[derive(Clone, Copy)]
pub(crate) enum RefreshMode {
    All,
    Notified,
}
impl RefreshMode {
    fn includes(self, index: usize, pending: &BTreeSet<usize>) -> bool {
        matches!(self, Self::All) || pending.contains(&index)
    }
}

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
    Callback(ShadowCallbackError),
}
impl fmt::Display for ShadowSessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Callback(error) => error.fmt(f),
            Self::FixedDependencyCycle(variables) => write!(
                f,
                "fixed dependency loops for shadow variables: {}",
                variables.join(", ")
            ),
            _ => write!(f, "shadow session: {self:?}"),
        }
    }
}
impl Error for ShadowSessionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Callback(error) => Some(error),
            _ => None,
        }
    }
}
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
    // The sole constructor receives DependencyCache::Plan::derive's sorted
    // unique dependency indices. Session topology keys are exactly 0..nodes.len(),
    // so their key values are also sorted registration IDs, including on restore.
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
        let position = self
            .values
            .binary_search_by_key(&node.node.index, |(index, _)| **index)
            .map_err(|_| ShadowSessionError::NotDependency)?;
        self.values[position]
            .1
            .value
            .as_ref()
            .map(Stored::get)
            .transpose()
    }
}
type CallbackResult = Result<Option<Stored>, Box<dyn Error + Send + Sync>>;
fn callback_result(
    result: CallbackResult,
    index: usize,
    variable: PlanningVariableRef<'_>,
    stage: ShadowCallbackStage,
) -> Result<Option<Stored>, ShadowSessionError> {
    result.map_err(|cause| {
        ShadowSessionError::Callback(ShadowCallbackError::new(index, variable, stage, cause))
    })
}
type Reader<'a, W> = dyn Fn(&W) -> CallbackResult + 'a;
type Supplier<'a, W> = dyn for<'c> FnMut(&W, ShadowInputs<'c>) -> CallbackResult + 'a;
enum Operation<'a, W> {
    Input(Box<Reader<'a, W>>),
    Derived(Box<Supplier<'a, W>>),
}
type KeyedSourceResolver<'a, 's, K, W> =
    dyn Fn(&W) -> Result<Vec<(PlanningVariableRef<'s>, K)>, ShadowSessionError> + 'a;
type IndexedSourceResolver<'a, W> = dyn Fn(&W) -> Result<Vec<usize>, ShadowSessionError> + 'a;
enum SourceResolver<'a, 's, K, W> {
    Native(Box<IndexedSourceResolver<'a, W>>),
    Paths(Box<KeyedSourceResolver<'a, 's, K, W>>),
}
struct Node<'a, 's, K, W> {
    variable: PlanningVariableRef<'s>,
    entity: K,
    operation: Operation<'a, W>,
    sources: Option<SourceResolver<'a, 's, K, W>>,
    fixed_sources: Vec<PlanningVariableRef<'s>>,
}
/// Explicit typed field registration; native entity keys identify model objects.
/// Metadata is borrowed, and handles cannot be used with a different builder/session.
pub struct ShadowSessionBuilder<'a, 's, K, W> {
    schema: &'s PlanningSchema,
    identity: Rc<()>,
    nodes: Vec<Node<'a, 's, K, W>>,
    source_index: Option<KeyedSourceIndex<K>>,
    edges: BTreeSet<(usize, usize)>,
}
impl<'a, 's, K: Eq, W> ShadowSessionBuilder<'a, 's, K, W> {
    pub fn new(schema: &'s PlanningSchema) -> Self {
        Self {
            schema,
            identity: Rc::new(()),
            nodes: vec![],
            source_index: None,
            edges: BTreeSet::new(),
        }
    }
    /// Index compiled source-path lookups by canonical field and entity key.
    /// The default needs only `Eq` and searches registrations in order. This
    /// opt-in uses `Hash` to narrow each lookup, then verifies `Eq`; hash collisions
    /// preserve the same first-registration result and missing-source diagnostics.
    /// Keys must obey the usual `Eq`/`Hash` contract (equal keys hash equally), and
    /// registered identity must remain immutable for the session's lifetime.
    ///
    /// Enabling this mode indexes all prior registrations; later successful
    /// registrations append to that index. Repeated opt-in preserves the existing
    /// index, which is moved into the session at `build`/`try_build`. This method
    /// consumes the builder: a Hash panic while indexing prior registrations drops
    /// it; the retry guarantee below applies to `input`/`derive` registration.
    /// Hash and Eq checks finish before a registration is published, so callback
    /// panics or duplicate errors leave the builder retryable. Registered key identity must
    /// remain stable from enabling this mode onward, including during construction.
    /// Keys are never cloned for indexing. Expected duplicate detection, owner
    /// grouping and fixed-source lookup work becomes linear in their respective
    /// registration/reference counts, using O(registered fields) extra space.
    /// Collision-heavy keys can still require quadratic construction work.
    /// Native handle bindings need no runtime key lookup, but their registration
    /// and owner grouping also benefit from indexed construction.
    ///
    /// Each new registration built by a dynamic shadow-program factory gets its
    /// own index. Checkpoints reuse their session's immutable registration/index;
    /// they do not rebuild it or run hash callbacks during internal restoration.
    pub fn with_keyed_source_index(mut self) -> Self
    where
        K: std::hash::Hash,
    {
        if self.source_index.is_none() {
            self.source_index = Some(KeyedSourceIndex::new(keyed_index::hash::<K>, &self.nodes));
        }
        self
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
        // Run all user Hash/Eq callbacks before changing either registration
        // structure. Primitive bucket insertion below invokes no user callbacks.
        let indexed_key = self
            .source_index
            .as_ref()
            .map(|index| index.key(variable, &entity));
        let duplicate = if let (Some(index), Some(key)) = (&self.source_index, indexed_key) {
            index
                .find_key(&self.nodes, variable, &entity, key)
                .is_some()
        } else {
            self.nodes
                .iter()
                .any(|n| n.variable == variable && n.entity == entity)
        };
        if duplicate {
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
        if let (Some(index), Some(key)) = (&mut self.source_index, indexed_key) {
            index.insert(key);
        }
        Ok(node)
    }
    /// Read a basic variable or an externally maintained shadow. All readers
    /// run once per ordinary [`ShadowSession::update`]; a notified refresh samples
    /// only marked inputs. Equality determines downstream recalculation.
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
            Operation::Input(Box::new(move |w| Ok(read(w).map(Stored::new)))),
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
            Operation::Input(Box::new(move |w| Ok(Some(Stored::new(read(w)))))),
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
            Operation::Derived(Box::new(move |w, i| Ok(supplier(w, i).map(Stored::new)))),
        )
    }
    /// Register a reader that can reject invalid working data without panicking.
    /// `Ok(None)` is an ordinary absent value; `Err` aborts the refresh before
    /// suppliers run. The original error is retained in
    /// [`ShadowSessionError::Callback`], with the field and registration index.
    /// The error must be Send + Sync; the reader and world need not be.
    ///
    /// Failed refreshes preserve cached values, applied dependencies, change
    /// reports and pending notifications. Retry after correcting the working
    /// data. Callback side effects outside the session cannot be rolled back.
    pub fn try_input<T: Eq + 'static, E: Error + Send + Sync + 'static>(
        &mut self,
        variable: PlanningVariableRef<'s>,
        entity: K,
        read: impl Fn(&W) -> Result<Option<T>, E> + 'a,
    ) -> Result<ShadowNode<T>, ShadowSessionError> {
        if matches!(variable.kind(), PlanningVariableKind::List { .. })
            || variable.value_type() != PlanningType::of::<T>()
        {
            return Err(ShadowSessionError::WrongType);
        }
        self.register(
            variable,
            entity,
            Operation::Input(Box::new(move |w| {
                read(w)
                    .map(|value| value.map(Stored::new))
                    .map_err(|error| Box::new(error) as _)
            })),
        )
    }
    /// Fallible counterpart of [`Self::list_input`]. An empty list is a valid
    /// value; an error aborts publication with the same contract as try_input.
    pub fn try_list_input<T: Eq + 'static, E: Error + Send + Sync + 'static>(
        &mut self,
        variable: PlanningVariableRef<'s>,
        entity: K,
        read: impl Fn(&W) -> Result<Vec<T>, E> + 'a,
    ) -> Result<ShadowNode<Vec<T>>, ShadowSessionError> {
        if !matches!(variable.kind(), PlanningVariableKind::List { .. })
            || variable.value_type() != PlanningType::of::<T>()
        {
            return Err(ShadowSessionError::WrongType);
        }
        self.register(
            variable,
            entity,
            Operation::Input(Box::new(move |w| {
                read(w)
                    .map(|value| Some(Stored::new(value)))
                    .map_err(|error| Box::new(error) as _)
            })),
        )
    }
    /// Register a fallible supplier. The first error stops evaluation immediately,
    /// including all later/downstream callbacks; no cache or topology prefix is
    /// published. Cyclic or inconsistent dependencies still suppress the supplier.
    /// Error and panic recovery preserve the same pending notifications. External
    /// callback effects are not rolled back. Successful None remains consistent.
    pub fn try_derive<T: Eq + 'static, E: Error + Send + Sync + 'static>(
        &mut self,
        variable: PlanningVariableRef<'s>,
        entity: K,
        mut supplier: impl for<'c> FnMut(&W, ShadowInputs<'c>) -> Result<Option<T>, E> + 'a,
    ) -> Result<ShadowNode<T>, ShadowSessionError> {
        if variable.kind() != PlanningVariableKind::Shadow
            || variable.value_type() != PlanningType::of::<T>()
        {
            return Err(ShadowSessionError::WrongType);
        }
        self.register(
            variable,
            entity,
            Operation::Derived(Box::new(move |w, inputs| {
                supplier(w, inputs)
                    .map(|value| value.map(Stored::new))
                    .map_err(|error| Box::new(error) as _)
            })),
        )
    }
    /// Resolve dependencies directly from native working state on every ordinary
    /// update, or when this target is marked dirty for a notified refresh.
    /// Register all possible source nodes before binding. Returned handles must
    /// belong to this builder and have been registered at binding time. This is
    /// useful for list predecessors and application links without object getters.
    /// Resolver errors occur before readers/suppliers; cycles remain runtime
    /// states. Do not mutate captured facts or the supplied world in a resolver.
    /// Manual edges and compiled paths cannot also control the same target.
    /// Native handles resolve directly to node indices without cloning or
    /// comparing entity keys. Each binding captures its current registration
    /// boundary; adding nodes never expands an earlier binding's boundary.
    pub fn bind_dependencies<T>(
        &mut self,
        target: &ShadowNode<T>,
        resolve: impl Fn(&W) -> Result<Vec<ShadowNodeRef>, ShadowSessionError> + 'a,
    ) -> Result<(), ShadowSessionError> {
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
        let registered = self.nodes.len();
        self.nodes[index].sources = Some(SourceResolver::Native(Box::new(move |world| {
            resolve(world)?
                .iter()
                .map(|node| lookup(&identity, registered, node))
                .collect()
        })));
        Ok(())
    }

    /// Resolve these compiled paths on every ordinary update, or when this target
    /// is marked dirty for a notified refresh. The root and every source
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
        self.nodes[index].sources = Some(SourceResolver::Paths(Box::new(move |world| {
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
        })));
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
                let source = if let Some(index) = &self.source_index {
                    index.find(&self.nodes, *field, &node.entity)
                } else {
                    self.nodes.iter().position(|candidate| {
                        candidate.variable == *field && candidate.entity == node.entity
                    })
                }
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
        let entity_groups = self.entity_groups();
        let source_index = self.source_index;
        let fields = self.nodes.iter().map(|node| node.variable).collect();
        ShadowSession {
            source_index,
            fields,
            entity_groups,
            last_changed: Rc::from([]),
            identity: self.identity,
            nodes: self.nodes,
            edges: Rc::new(self.edges),
            bindings: Rc::new(BindingState::flat(BTreeSet::new(), vec![])),
            manual_applied: Rc::default(),
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
/// Checkpoints share immutable edge sets and binding signatures as well as values;
/// pending notification sets are copied. Manual rewires detach the shared edge set.
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
    source_index: Option<KeyedSourceIndex<K>>,
    fields: Vec<PlanningVariableRef<'s>>,
    entity_groups: EntityGroups,
    last_changed: Rc<[ShadowNodeRef]>,
    identity: Rc<()>,
    nodes: Vec<Node<'a, 's, K, W>>,
    edges: Rc<BTreeSet<(usize, usize)>>,
    bindings: Rc<BindingState>,
    manual_applied: Rc<BTreeSet<(usize, usize)>>,
    pending: BTreeSet<usize>,
    cache: DependencyCache<usize, Stored>,
}
#[derive(Clone)]
pub struct ShadowSessionCheckpoint {
    last_changed: Rc<[ShadowNodeRef]>,
    identity: Rc<()>,
    cache: DependencyCache<usize, Stored>,
    bindings: Rc<BindingState>,
    manual_edges: Rc<BTreeSet<(usize, usize)>>,
    requested_edges: Rc<BTreeSet<(usize, usize)>>,
    pending: BTreeSet<usize>,
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
            Rc::make_mut(&mut self.edges),
            &target.node,
            sources,
        )
    }
    pub fn mark_dirty<T>(&mut self, node: &ShadowNode<T>) -> Result<(), ShadowSessionError> {
        self.pending
            .insert(lookup(&self.identity, self.nodes.len(), &node.node)?);
        Ok(())
    }
    pub(crate) fn invalidate_all(&mut self) {
        self.pending.extend(0..self.nodes.len());
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
        manual: &Rc<BTreeSet<(usize, usize)>>,
    ) -> Result<ResolvedSources, ShadowSessionError> {
        self.resolve_sources_mode(world, manual, RefreshMode::All)
    }
    fn resolve_sources_mode(
        &self,
        world: &W,
        manual: &Rc<BTreeSet<(usize, usize)>>,
        mode: RefreshMode,
    ) -> Result<ResolvedSources, ShadowSessionError> {
        if self.can_resolve_notified(manual, mode) {
            let updates = self
                .pending
                .iter()
                .filter(|&&i| self.nodes[i].sources.is_some())
                .map(|&index| {
                    self.resolve_signature(index, world)
                        .map(|signature| (index, signature))
                })
                .collect::<Result<Vec<_>, _>>()?;
            return Ok(self.prepare_binding_updates(updates));
        }
        let mut edges = manual.as_ref().clone();
        let mut signatures = vec![];
        for index in 0..self.nodes.len() {
            let signature = if !mode.includes(index, &self.pending) {
                self.bindings.signature(index).cloned()
            } else {
                self.resolve_signature(index, world)?
            };
            if let Some(sources) = &signature {
                edges.extend(sources.iter().map(|&source| (index, source)));
            }
            signatures.push(signature);
        }
        Ok(self.share_resolved_sources(edges, signatures))
    }
    // Keep the Eq-only scan out of the much larger diagnostic/source-resolution
    // bodies so indexed dispatch does not spill the scan key on every iteration.
    #[inline(never)]
    fn lookup_source(&self, field: PlanningVariableRef<'_>, entity: &K) -> Option<usize> {
        if let Some(index) = &self.source_index {
            index.find(&self.nodes, field, entity)
        } else {
            self.nodes
                .iter()
                .position(|node| node.variable == field && &node.entity == entity)
        }
    }
    fn resolve_signature(
        &self,
        index: usize,
        world: &W,
    ) -> Result<Option<Vec<usize>>, ShadowSessionError> {
        match &self.nodes[index].sources {
            Some(SourceResolver::Native(resolve)) => Ok(Some(resolve(world)?)),
            Some(SourceResolver::Paths(resolve)) => {
                let mut signature = vec![];
                for (variable, entity) in resolve(world)? {
                    let source = self
                        .lookup_source(variable, &entity)
                        .ok_or(ShadowSessionError::UnregisteredSource)?;
                    signature.push(source);
                }
                Ok(Some(signature))
            }
            None => Ok(None),
        }
    }
    fn share_resolved_sources(
        &self,
        edges: BTreeSet<(usize, usize)>,
        signatures: Vec<Option<Vec<usize>>>,
    ) -> ResolvedSources {
        if let BindingState::Flat {
            edges: old_edges,
            signatures: old_signatures,
            ..
        } = self.bindings.as_ref()
        {
            if &edges == old_edges && &signatures == old_signatures {
                return ResolvedSources::unchanged(self.bindings.clone());
            }
            let dirty = signatures
                .iter()
                .enumerate()
                .filter_map(|(index, signature)| {
                    (signature.is_some() && old_signatures.get(index) != Some(signature))
                        .then_some(index)
                })
                .collect();
            let removals = old_edges.difference(&edges).copied().collect();
            let additions = edges.difference(old_edges).copied().collect();
            let bindings = Rc::new(BindingState::flat(edges, signatures));
            return ResolvedSources {
                bindings,
                dirty,
                removals,
                additions,
            };
        }
        let previous = self.bindings.sparse();
        // Compare complete preparations in sorted order, retaining linear scans
        // for ordinary polling instead of looking up every unchanged signature.
        let mut old = previous.signatures.iter().peekable();
        let updates = signatures
            .into_iter()
            .enumerate()
            .filter_map(|(index, signature)| {
                let signature = signature?;
                if old.peek().is_some_and(|(i, _)| **i == index) {
                    let (_, previous) = old.next().unwrap();
                    if previous == &signature {
                        return None;
                    }
                }
                Some((index, Some(signature)))
            })
            .collect();
        let mut resolved = self.prepare_binding_updates(updates);
        // Dynamic targets and manual targets are disjoint. Full preparation must
        // also include manual edits, including checked restore's saved topology.
        let (removals, additions) = edge_changes(previous.edges.keys(), edges.iter());
        if removals != resolved.removals || additions != resolved.additions {
            let mut next = resolved.bindings.sparse().clone();
            // Manual edits are the only difference from the already prepared
            // binding changes; publish the requested complete edge set.
            next.edges = previous.edges.clone();
            for edge in &removals {
                next.edges.remove(edge);
            }
            for &edge in &additions {
                next.edges.insert(edge, ());
            }
            resolved.bindings = Rc::new(BindingState::Sparse(next));
            resolved.removals = removals;
            resolved.additions = additions;
        }
        resolved
    }
    // Build an unpublished persistent delta: signature order/multiplicity still
    // invalidates the target even when its deduplicated incoming edges stay equal.
    fn prepare_binding_updates(
        &self,
        updates: Vec<(usize, Option<Vec<usize>>)>,
    ) -> ResolvedSources {
        if updates.is_empty() {
            return ResolvedSources::unchanged(self.bindings.clone());
        }
        let previous = self.bindings.sparse();
        let mut next = previous.clone();
        let mut dirty = vec![];
        let mut removals = vec![];
        let mut additions = vec![];
        for (index, signature) in updates {
            let Some(signature) = signature else { continue };
            let old_signature = previous.signatures.get(&index);
            if old_signature == Some(&signature) {
                continue;
            }
            dirty.push(index);
            let old: BTreeSet<_> = old_signature.into_iter().flatten().copied().collect();
            let new: BTreeSet<_> = signature.iter().copied().collect();
            for &source in old.difference(&new) {
                let edge = (index, source);
                next.edges.remove(&edge);
                removals.push(edge);
            }
            for &source in new.difference(&old) {
                let edge = (index, source);
                next.edges.insert(edge, ());
                additions.push(edge);
            }
            next.signatures.insert(index, signature);
        }
        ResolvedSources {
            bindings: if dirty.is_empty() {
                self.bindings.clone()
            } else {
                Rc::new(BindingState::Sparse(next))
            },
            dirty,
            removals,
            additions,
        }
    }
    // Tiny universes and dense notifications keep the flat preparation path.
    // Value-only updates
    // can reuse either representation without promotion or metadata traversal.
    fn can_resolve_notified(
        &self,
        manual: &Rc<BTreeSet<(usize, usize)>>,
        mode: RefreshMode,
    ) -> bool {
        matches!(mode, RefreshMode::Notified)
            && self.bindings.len() == self.nodes.len()
            && manual == &self.manual_applied
            && ((self.nodes.len() > 32 && self.pending.len() <= self.nodes.len() / 4)
                || self
                    .pending
                    .iter()
                    .all(|&i| self.nodes[i].sources.is_none()))
    }
    /// Return handles for changed derived fields, including inconsistency changes.
    /// Successful refreshes also replace [`Self::changed_entities`]'s report;
    /// a no-change refresh publishes an empty report. Failed refreshes retain it.
    pub fn update(&mut self, world: &W) -> Result<Vec<ShadowNodeRef>, ShadowSessionError> {
        let resolved = self.resolve_sources(world, &self.edges)?;
        self.update_resolved(world, resolved, RefreshMode::All)
    }
    /// Refresh only inputs and dynamic bindings explicitly marked dirty through
    /// [`Self::mark_dirty`] or [`Self::mark_entities_dirty`]. The first refresh
    /// initializes every node. Later calls retain unnotified input values and
    /// binding signatures, while changed values still propagate to all dependents.
    /// Manual dependency edits are always applied, even with no notifications.
    /// After initialization with unchanged manual dependencies, preparation visits
    /// only notified inputs and binding targets in large sessions. These lazily
    /// promote flat metadata to persistent snapshots once per retained snapshot;
    /// subsequent sparse updates neither scan nor copy unchanged registrations.
    /// Tiny sessions and dense notifications retain full preparation. Keyed paths still
    /// search registered
    /// entities, and topology edits may reorder whole components. Propagation,
    /// explicit consistency diagnostics and caller projection may still touch
    /// many or all nodes; this is not an overall
    /// constant-time refresh guarantee.
    ///
    /// The caller must notify every input whose reader result may differ, every
    /// target whose dynamic binding may differ, and every supplier that directly
    /// reads changed external facts. A changed source does not automatically
    /// notify its consumers' bindings. Violating this contract can leave stale
    /// values or edges; use [`Self::update`] when complete notifications are not
    /// available. Independently requested diagnostics and checked restores still
    /// read the full world.
    ///
    /// Resolver callbacks precede readers, both in registration order. Failures
    /// and callback panics preserve cached state and pending notifications for
    /// retry; external callback effects are not rolled back. Change reports use
    /// the same publication and no-change rules as [`Self::update`].
    ///
    /// ```
    /// use timetuck::*;
    /// struct Model; struct Task;
    /// let schema = PlanningSchema::new::<Model>(vec![PlanningEntityDefinition::of::<Task>(vec![
    ///     PlanningVariableDefinition::basic::<i64>("value", true),
    ///     PlanningVariableDefinition::shadow::<i64>("twice"),
    /// ])])?;
    /// let fields = schema.entity::<Task>()?;
    /// let mut builder = ShadowSessionBuilder::<usize, i64>::new(&schema);
    /// let input = builder.input(fields.variable("value")?, 0, |world| Some(*world))?;
    /// let source = input.clone();
    /// let output = builder.derive(fields.variable("twice")?, 0, move |_, values| {
    ///     values.get(&source).unwrap().map(|value| value * 2)
    /// })?;
    /// builder.set_dependencies(&output, &[input.erase()])?;
    /// let mut session = builder.build();
    /// session.update_notified(&3)?;
    /// session.mark_dirty(&input)?;
    /// session.update_notified(&5)?;
    /// assert_eq!(session.read(&output)?.value, Some(&10));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn update_notified(&mut self, world: &W) -> Result<Vec<ShadowNodeRef>, ShadowSessionError> {
        let resolved = self.resolve_sources_mode(world, &self.edges, RefreshMode::Notified)?;
        self.update_resolved(world, resolved, RefreshMode::Notified)
    }
    fn update_resolved(
        &mut self,
        world: &W,
        resolved: ResolvedSources,
        mode: RefreshMode,
    ) -> Result<Vec<ShadowNodeRef>, ShadowSessionError> {
        let prepared = match mode {
            RefreshMode::All => PreparedInputs::All(
                self.nodes
                    .iter()
                    .enumerate()
                    .map(|(index, node)| match &node.operation {
                        Operation::Input(read) => callback_result(
                            read(world),
                            index,
                            node.variable,
                            ShadowCallbackStage::Input,
                        ),
                        Operation::Derived(_) => Ok(None),
                    })
                    .collect::<Result<_, _>>()?,
            ),
            RefreshMode::Notified => PreparedInputs::Notified(
                self.pending
                    .iter()
                    .filter_map(|&index| match &self.nodes[index].operation {
                        Operation::Input(read) => Some(
                            callback_result(
                                read(world),
                                index,
                                self.nodes[index].variable,
                                ShadowCallbackStage::Input,
                            )
                            .map(|value| (index, value)),
                        ),
                        Operation::Derived(_) => None,
                    })
                    .collect::<Result<_, _>>()?,
            ),
        };
        let mut dirty = self.pending.clone();
        dirty.extend(&resolved.dirty);
        match &prepared {
            PreparedInputs::All(values) => {
                for (index, node) in self.nodes.iter().enumerate() {
                    if matches!(node.operation, Operation::Input(_))
                        && self.cache.values()[&index].value != values[index]
                    {
                        dirty.insert(index);
                    }
                }
            }
            PreparedInputs::Notified(values) => {
                for (index, value) in values {
                    if &self.cache.values()[index].value != value {
                        dirty.insert(*index);
                    }
                }
            }
        }
        let nodes = &mut self.nodes;
        let identity = &self.identity;
        // Prepare potentially allocating publication data before installing a new cache.
        let manual_applied = self.edges.clone();
        let bindings = resolved.bindings;
        let mut next = self.cache.clone();
        let cached = &self.cache;
        let changed = next.try_update(
            &resolved.removals,
            &resolved.additions,
            &dirty.into_iter().collect::<Vec<_>>(),
            |index, values| {
                let variable = nodes[*index].variable;
                match &mut nodes[*index].operation {
                    // Cycle recovery and manual rewires can schedule an input that
                    // was not notified. It must retain its last sampled value.
                    Operation::Input(_) => Ok(prepared
                        .get(index)
                        .cloned()
                        .unwrap_or_else(|| cached.values()[index].value.clone())),
                    Operation::Derived(supplier) => callback_result(
                        supplier(
                            world,
                            ShadowInputs {
                                identity,
                                values,
                                fields: &self.fields,
                            },
                        ),
                        *index,
                        variable,
                        ShadowCallbackStage::Derived,
                    ),
                }
            },
        )?;
        let changed: Vec<_> = changed
            .into_iter()
            .filter(|&i| matches!(nodes[i].operation, Operation::Derived(_)))
            .map(|index| ShadowNodeRef {
                index,
                identity: identity.clone(),
            })
            .collect();
        let last_changed = changed.iter().cloned().collect();
        self.cache = next;
        self.last_changed = last_changed;
        self.bindings = bindings;
        self.manual_applied = manual_applied;
        self.pending.clear();
        Ok(changed)
    }
    pub fn checkpoint(&self) -> Result<ShadowSessionCheckpoint, ShadowSessionError> {
        self.ready()?;
        Ok(self.capture())
    }
    pub(crate) fn capture(&self) -> ShadowSessionCheckpoint {
        ShadowSessionCheckpoint {
            last_changed: self.last_changed.clone(),
            identity: self.identity.clone(),
            cache: self.cache.clone(),
            bindings: self.bindings.clone(),
            manual_edges: self.manual_applied.clone(),
            requested_edges: self.edges.clone(),
            pending: self.pending.clone(),
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
        if !resolved.bindings.equivalent(&saved.bindings) {
            return Err(ShadowSessionError::SnapshotMismatch);
        }
        for (index, node) in self.nodes.iter().enumerate() {
            if let Operation::Input(read) = &node.operation
                && callback_result(
                    read(world),
                    index,
                    node.variable,
                    ShadowCallbackStage::Input,
                )? != saved.cache.values()[&index].value
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
        self.last_changed = saved.last_changed.clone();
        self.edges = saved.requested_edges.clone();
        self.bindings = saved.bindings.clone();
        self.manual_applied = saved.manual_edges.clone();
        self.pending = saved.pending.clone();
    }
}

enum PreparedInputs {
    All(Vec<Option<Stored>>),
    Notified(Vec<(usize, Option<Stored>)>),
}
impl PreparedInputs {
    fn get(&self, index: &usize) -> Option<&Option<Stored>> {
        match self {
            Self::All(values) => values.get(*index),
            Self::Notified(values) => values
                .binary_search_by_key(index, |(key, _)| *key)
                .ok()
                .map(|at| &values[at].1),
        }
    }
}

// Ordinary polling retains compact flat metadata. Large sparse updates lazily
// promote it once; the shared cell belongs to the immutable snapshot so restoring
// an older checkpoint does not repeatedly rebuild the same persistent trees.
enum BindingState {
    Flat {
        edges: BTreeSet<(usize, usize)>,
        signatures: Vec<Option<Vec<usize>>>,
        sparse: OnceCell<SparseBindings>,
    },
    Sparse(SparseBindings),
}
#[derive(Clone)]
struct SparseBindings {
    edges: SnapshotMap<(usize, usize), ()>,
    signatures: SnapshotMap<usize, Vec<usize>>,
    len: usize,
}
impl BindingState {
    fn flat(edges: BTreeSet<(usize, usize)>, signatures: Vec<Option<Vec<usize>>>) -> Self {
        Self::Flat {
            edges,
            signatures,
            sparse: OnceCell::new(),
        }
    }
    fn len(&self) -> usize {
        match self {
            Self::Flat { signatures, .. } => signatures.len(),
            Self::Sparse(s) => s.len,
        }
    }
    fn signature(&self, index: usize) -> Option<&Vec<usize>> {
        match self {
            Self::Flat { signatures, .. } => signatures.get(index).and_then(Option::as_ref),
            Self::Sparse(s) => s.signatures.get(&index),
        }
    }
    fn sparse(&self) -> &SparseBindings {
        match self {
            Self::Sparse(s) => s,
            Self::Flat {
                edges,
                signatures,
                sparse,
            } => sparse.get_or_init(|| {
                let mut next = SparseBindings {
                    edges: SnapshotMap::new(),
                    signatures: SnapshotMap::new(),
                    len: signatures.len(),
                };
                for &edge in edges {
                    next.edges.insert(edge, ());
                }
                for (index, signature) in signatures.iter().enumerate() {
                    if let Some(signature) = signature {
                        next.signatures.insert(index, signature.clone());
                    }
                }
                next
            }),
        }
    }
    fn equivalent(&self, other: &Self) -> bool {
        if std::ptr::eq(self, other) {
            return true;
        }
        match (self, other) {
            (
                Self::Flat {
                    edges: a,
                    signatures: sa,
                    ..
                },
                Self::Flat {
                    edges: b,
                    signatures: sb,
                    ..
                },
            ) => a == b && sa == sb,
            _ => {
                let a = self.sparse();
                let b = other.sparse();
                a.len == b.len && a.edges == b.edges && a.signatures == b.signatures
            }
        }
    }
}
type EdgeDelta = (Vec<(usize, usize)>, Vec<(usize, usize)>);
fn edge_changes<'a>(
    old: impl Iterator<Item = &'a (usize, usize)>,
    new: impl Iterator<Item = &'a (usize, usize)>,
) -> EdgeDelta {
    let mut old = old.peekable();
    let mut new = new.peekable();
    let mut removals = vec![];
    let mut additions = vec![];
    loop {
        match (old.peek(), new.peek()) {
            (Some(a), Some(b)) if a == b => {
                old.next();
                new.next();
            }
            (Some(a), Some(b)) if a < b => {
                removals.push(**a);
                old.next();
            }
            (_, Some(b)) => {
                additions.push(**b);
                new.next();
            }
            (Some(a), None) => {
                removals.push(**a);
                old.next();
            }
            (None, None) => break,
        }
    }
    (removals, additions)
}
struct ResolvedSources {
    bindings: Rc<BindingState>,
    dirty: Vec<usize>,
    removals: Vec<(usize, usize)>,
    additions: Vec<(usize, usize)>,
}
impl ResolvedSources {
    fn unchanged(bindings: Rc<BindingState>) -> Self {
        Self {
            bindings,
            dirty: vec![],
            removals: vec![],
            additions: vec![],
        }
    }
}

#[path = "shadow_diagnostics.rs"]
mod diagnostics;
pub use diagnostics::{
    ConsistentShadowUpdateError, ShadowConsistencyPolicy, ShadowConsistencyReport,
    ShadowVariableKey, shadow_consistency, update_consistent_shadows, update_shadows_with_policy,
};

pub(crate) use diagnostics::update_shadows_mode;
pub use diagnostics::{ShadowSourceReference, missing_shadow_sources};

#[cfg(test)]
#[path = "shadow_session/binding_snapshots.rs"]
mod binding_snapshots;
