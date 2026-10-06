//! Typed heterogeneous shadow scheduling over the native dependency cache.
use crate::{
    DependencyCache, DependencyError, DependencyTopology, DerivedValue, PlanningSchema,
    PlanningType, PlanningVariableKind, PlanningVariableRef,
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
    Dependency(DependencyError<usize>),
}
impl fmt::Display for ShadowSessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "shadow session: {self:?}")
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
}
impl ShadowInputs<'_> {
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
struct Node<'a, 's, K, W> {
    variable: PlanningVariableRef<'s>,
    entity: K,
    operation: Operation<'a, W>,
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
    pub fn build(self) -> ShadowSession<'a, 's, K, W> {
        let topology = DependencyTopology::new(0..self.nodes.len()).expect("unique node indices");
        let pending = (0..self.nodes.len()).collect();
        ShadowSession {
            identity: self.identity,
            nodes: self.nodes,
            edges: self.edges,
            applied: BTreeSet::new(),
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
    pending: BTreeSet<usize>,
    cache: DependencyCache<usize, Stored>,
}
pub struct ShadowSessionCheckpoint {
    identity: Rc<()>,
    cache: DependencyCache<usize, Stored>,
    edges: BTreeSet<(usize, usize)>,
}
impl<K, W> ShadowSession<'_, '_, K, W> {
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
        if self.pending.is_empty() && self.edges == self.applied {
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
    /// Return handles for changed derived fields, including inconsistency changes.
    pub fn update(&mut self, world: &W) -> Result<Vec<ShadowNodeRef>, ShadowSessionError> {
        let prepared = self
            .nodes
            .iter()
            .map(|n| match &n.operation {
                Operation::Input(read) => read(world),
                Operation::Derived(_) => None,
            })
            .collect::<Vec<_>>();
        let mut dirty = self.pending.clone();
        for (index, node) in self.nodes.iter().enumerate() {
            if matches!(node.operation, Operation::Input(_))
                && self.cache.values()[&index].value != prepared[index]
            {
                dirty.insert(index);
            }
        }
        let removals = self
            .applied
            .difference(&self.edges)
            .copied()
            .collect::<Vec<_>>();
        let additions = self
            .edges
            .difference(&self.applied)
            .copied()
            .collect::<Vec<_>>();
        let nodes = &mut self.nodes;
        let identity = &self.identity;
        // Prepare potentially allocating publication data before installing a new cache.
        let applied = self.edges.clone();
        let mut next = self.cache.clone();
        let changed = next.update(
            &removals,
            &additions,
            &dirty.into_iter().collect::<Vec<_>>(),
            |index, values| match &mut nodes[*index].operation {
                Operation::Input(_) => prepared[*index].clone(),
                Operation::Derived(supplier) => supplier(world, ShadowInputs { identity, values }),
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
        self.pending.clear();
        Ok(changed)
    }
    pub fn checkpoint(&self) -> Result<ShadowSessionCheckpoint, ShadowSessionError> {
        self.ready()?;
        Ok(ShadowSessionCheckpoint {
            identity: self.identity.clone(),
            cache: self.cache.clone(),
            edges: self.applied.clone(),
        })
    }
    pub fn restore(
        &mut self,
        saved: &ShadowSessionCheckpoint,
        world: &W,
    ) -> Result<(), ShadowSessionError> {
        if !Rc::ptr_eq(&self.identity, &saved.identity) {
            return Err(ShadowSessionError::ForeignNode);
        }
        for (index, node) in self.nodes.iter().enumerate() {
            if let Operation::Input(read) = &node.operation
                && read(world) != saved.cache.values()[&index].value
            {
                return Err(ShadowSessionError::SnapshotMismatch);
            }
        }
        let edges = saved.edges.clone();
        let applied = edges.clone();
        self.cache = saved.cache.clone();
        self.edges = edges;
        self.applied = applied;
        self.pending.clear();
        Ok(())
    }
}
