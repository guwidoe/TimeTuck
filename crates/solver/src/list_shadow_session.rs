//! Metadata-bound deferred list shadows over the existing dependency evaluator.
use crate::{
    DependencyCache, DependencyError, DependencyTopology, DerivedValue, ListVariableState,
    PlanningType, PlanningVariableKind, PlanningVariableRef, SnapshotMap, VariableListLocation,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
    rc::Rc,
};
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ListShadowError<V> {
    Metadata(&'static str),
    UnknownValue(V),
    Dependency(DependencyError<V>),
    NotCurrent,
    ForeignCheckpoint,
    SnapshotMismatch,
}
impl<V: fmt::Debug> fmt::Display for ListShadowError<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "list shadow session: {self:?}")
    }
}
impl<V: fmt::Debug> Error for ListShadowError<V> {}
impl<V> From<DependencyError<V>> for ListShadowError<V> {
    fn from(value: DependencyError<V>) -> Self {
        Self::Dependency(value)
    }
}
/// All structural inputs are from one complete field snapshot. The predecessor
/// shadow is already updated. Unassigned values have no location or predecessor;
/// their supplier still runs when dirty, so it can clear or compute its result.
pub struct ListShadowContext<'c, E, V, H> {
    pub value: &'c V,
    pub location: Option<&'c VariableListLocation<E, V>>,
    pub previous_shadow: Option<&'c H>,
}
type Supplier<'a, E, V, H> = dyn for<'c> FnMut(ListShadowContext<'c, E, V, H>) -> Option<H> + 'a;
/// One typed shadow field cascading along the previous-element relation.
///
/// Bind a shared list state to a shadow field declared on the list's native
/// value type. Each value has a stable identity and one result. Several fields
/// can use independent sessions; jointly calculated results can use a struct H.
/// This does not infer arbitrary cross-field source paths or execute setters.
///
/// Source notifications accumulate without running suppliers. update_changed
/// reads the latest complete list snapshot, detects structural changes, rewires
/// predecessor dependencies and evaluates affected values once in dependency
/// order. Equal results stop propagation. Unassigned values remain in the fixed
/// universe and are processed individually, without conflating missing indices.
/// Owner additions/removals need no new session; a changed value universe does.
///
/// Suppliers may read their context and immutable captured facts. Explicitly
/// notify values affected by changed external facts. Do not mutate the bound
/// state during a supplier call. Validation errors and supplier panics preserve
/// cached results and pending notifications; external callback side effects are
/// not rolled back. Snapshots share persistent cached values, including non-Clone H.
///
/// ```
/// use timetuck::*;
/// struct Schedule; struct Route;
/// let schema=PlanningSchema::new::<Schedule>(vec![
///   PlanningEntityDefinition::of::<Route>(vec![PlanningVariableDefinition::list::<u32>("visits",false)]),
///   PlanningEntityDefinition::of::<u32>(vec![PlanningVariableDefinition::shadow::<i64>("ordinal")])])?;
/// let list=schema.entity::<Route>()?.variable("visits")?;
/// let shadow=schema.entity::<u32>()?.variable("ordinal")?;
/// let mut registry=VariableStateRegistry::new(&schema);
/// registry.reset_lists(list,[("route",vec![1u32,2,3])])?;
/// let mut session=ListShadowSession::new(registry.list::<&str,u32>(list)?,shadow,[1,2,3],|c| {
///   c.location.map(|_|c.previous_shadow.copied().unwrap_or(-1i64)+1)
/// })?;
/// assert_eq!(session.values()?[&3].value,Some(2));
/// registry.update_lists(list,[("route",vec![3u32,1,2])])?;
/// assert!(!session.is_current());
/// session.update_changed()?;
/// assert_eq!(session.values()?[&3].value,Some(0));
/// # Ok::<(),Box<dyn std::error::Error>>(())
/// ```
pub struct ListShadowSession<'a, 's, E, V, H> {
    state: ListVariableState<'s, E, V>,
    target: PlanningVariableRef<'s>,
    identity: Rc<()>,
    cache: DependencyCache<V, H>,
    positions: BTreeMap<V, VariableListLocation<E, V>>,
    rows: BTreeMap<E, Vec<V>>,
    pending: BTreeSet<V>,
    supplier: Box<Supplier<'a, E, V, H>>,
}
pub struct ListShadowCheckpoint<E, V, H> {
    identity: Rc<()>,
    cache: DependencyCache<V, H>,
    positions: BTreeMap<V, VariableListLocation<E, V>>,
    rows: BTreeMap<E, Vec<V>>,
    working_rows: BTreeMap<E, Vec<V>>,
    pending: BTreeSet<V>,
}
impl<'a, 's, E: Ord + Clone + 'static, V: Ord + Clone + 'static, H: Eq + 'static>
    ListShadowSession<'a, 's, E, V, H>
{
    pub fn new_deferred(
        state: ListVariableState<'s, E, V>,
        target: PlanningVariableRef<'s>,
        values: impl IntoIterator<Item = V>,
        supplier: impl for<'c> FnMut(ListShadowContext<'c, E, V, H>) -> Option<H> + 'a,
    ) -> Result<Self, ListShadowError<V>> {
        if !std::ptr::eq(
            state.variable().entity().solution(),
            target.entity().solution(),
        ) {
            return Err(ListShadowError::Metadata(
                "source and target belong to different schemas",
            ));
        }
        if target.kind() != PlanningVariableKind::Shadow {
            return Err(ListShadowError::Metadata(
                "target must be a shadow variable",
            ));
        }
        if target.entity().entity_type() != PlanningType::of::<V>() {
            return Err(ListShadowError::Metadata(
                "target entity must be the list value type",
            ));
        }
        if target.value_type() != PlanningType::of::<H>() {
            return Err(ListShadowError::Metadata(
                "supplier result type differs from target metadata",
            ));
        }
        let topology = DependencyTopology::new(values)?;
        let pending = topology.keys().iter().cloned().collect();
        let cache = DependencyCache::new(topology, |_, _| None);
        let session = Self {
            state,
            target,
            identity: Rc::new(()),
            cache,
            positions: BTreeMap::new(),
            rows: BTreeMap::new(),
            pending,
            supplier: Box::new(supplier),
        };
        session.prepare_positions(&session.state.snapshot())?;
        Ok(session)
    }
    pub fn new(
        state: ListVariableState<'s, E, V>,
        target: PlanningVariableRef<'s>,
        values: impl IntoIterator<Item = V>,
        supplier: impl for<'c> FnMut(ListShadowContext<'c, E, V, H>) -> Option<H> + 'a,
    ) -> Result<Self, ListShadowError<V>> {
        let mut session = Self::new_deferred(state, target, values, supplier)?;
        session.update_changed()?;
        Ok(session)
    }
    pub fn target(&self) -> PlanningVariableRef<'s> {
        self.target
    }
    pub fn source(&self) -> PlanningVariableRef<'s> {
        self.state.variable()
    }
    pub fn is_current(&self) -> bool {
        self.pending.is_empty() && self.rows == self.state.snapshot()
    }
    pub fn assert_current(&self) -> Result<(), ListShadowError<V>> {
        if self.is_current() {
            Ok(())
        } else {
            Err(ListShadowError::NotCurrent)
        }
    }
    /// Refuse stale shadow reads. cached_values explicitly allows diagnostic or
    /// partially initialized inspection without asserting currentness.
    pub fn values(&self) -> Result<&SnapshotMap<V, DerivedValue<H>>, ListShadowError<V>> {
        self.assert_current()?;
        Ok(self.cache.values())
    }
    pub fn cached_values(&self) -> &SnapshotMap<V, DerivedValue<H>> {
        self.cache.values()
    }
    pub fn pending_values(&self) -> impl ExactSizeIterator<Item = &V> {
        self.pending.iter()
    }
    pub fn mark_dirty(&mut self, value: &V) -> Result<(), ListShadowError<V>> {
        if !self.cache.values().contains_key(value) {
            return Err(ListShadowError::UnknownValue(value.clone()));
        }
        self.pending.insert(value.clone());
        Ok(())
    }
    /// Notify any changed field on this native value entity. Multiple parent
    /// notifications for one value coalesce; list structural changes are also
    /// detected automatically even if no notification was sent.
    pub fn after_variable_changed(
        &mut self,
        variable: PlanningVariableRef<'s>,
        value: &V,
    ) -> Result<(), ListShadowError<V>> {
        if variable.entity() != self.target.entity() {
            return Err(ListShadowError::Metadata(
                "notification is not a field of the target value entity",
            ));
        }
        self.mark_dirty(value)
    }
    fn prepare_positions(
        &self,
        rows: &BTreeMap<E, Vec<V>>,
    ) -> Result<BTreeMap<V, VariableListLocation<E, V>>, ListShadowError<V>> {
        let mut positions = BTreeMap::new();
        for (entity, list) in rows {
            for (index, value) in list.iter().enumerate() {
                if !self.cache.values().contains_key(value) {
                    return Err(ListShadowError::UnknownValue(value.clone()));
                }
                positions.insert(
                    value.clone(),
                    VariableListLocation {
                        entity: entity.clone(),
                        index,
                        previous: index.checked_sub(1).map(|i| list[i].clone()),
                        next: list.get(index + 1).cloned(),
                    },
                );
            }
        }
        Ok(positions)
    }
    pub fn update_changed(&mut self) -> Result<Vec<V>, ListShadowError<V>> {
        let rows = self.state.snapshot();
        let positions = self.prepare_positions(&rows)?;
        let mut dirty = self.pending.clone();
        let mut removals = vec![];
        let mut additions = vec![];
        for value in self.cache.topology().keys() {
            let old = self.positions.get(value);
            let new = positions.get(value);
            if old != new {
                dirty.insert(value.clone());
            }
            let previous = old.and_then(|p| p.previous.as_ref());
            let next = new.and_then(|p| p.previous.as_ref());
            if previous != next {
                if let Some(v) = previous {
                    removals.push((value.clone(), v.clone()));
                }
                if let Some(v) = next {
                    additions.push((value.clone(), v.clone()));
                }
            }
        }
        let supplier = &mut self.supplier;
        let changed = self.cache.update(
            &removals,
            &additions,
            &dirty.into_iter().collect::<Vec<_>>(),
            |value, inputs| {
                supplier(ListShadowContext {
                    value,
                    location: positions.get(value),
                    previous_shadow: inputs.first().and_then(|(_, v)| v.value.as_ref()),
                })
            },
        )?;
        self.positions = positions;
        self.rows = rows;
        self.pending.clear();
        Ok(changed)
    }
    /// Save both cached shadow state and the current bound field snapshot. To
    /// restore, first restore that field through VariableStateRegistry. Foreign
    /// sessions or mismatched assignments are rejected without changing results.
    pub fn checkpoint(&self) -> ListShadowCheckpoint<E, V, H> {
        ListShadowCheckpoint {
            identity: self.identity.clone(),
            cache: self.cache.clone(),
            positions: self.positions.clone(),
            rows: self.rows.clone(),
            working_rows: self.state.snapshot(),
            pending: self.pending.clone(),
        }
    }
    pub fn restore(
        &mut self,
        saved: &ListShadowCheckpoint<E, V, H>,
    ) -> Result<(), ListShadowError<V>> {
        if !Rc::ptr_eq(&self.identity, &saved.identity) {
            return Err(ListShadowError::ForeignCheckpoint);
        }
        if self.state.snapshot() != saved.working_rows {
            return Err(ListShadowError::SnapshotMismatch);
        }
        self.cache = saved.cache.clone();
        self.positions = saved.positions.clone();
        self.rows = saved.rows.clone();
        self.pending = saved.pending.clone();
        Ok(())
    }
}
