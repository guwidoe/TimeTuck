//! Typed inverse-collection publication over shared basic-variable state.
use crate::{
    BasicVariableState, PlanningType, PlanningVariableKind, PlanningVariableRef,
    VariableStateError, VariableStateRegistry,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BasicInverseChange<'s, E, V> {
    pub variable: PlanningVariableRef<'s>,
    pub value: V,
    pub before: Vec<E>,
    pub after: Vec<E>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BasicStateUpdate<'s, E, V> {
    pub shadows: Vec<BasicInverseChange<'s, E, V>>,
    /// Without an externalized inverse field, each removed/added membership
    /// contributes one notification. Repeated value identities are intentional.
    pub notifications: Vec<V>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BasicSupplyError<E, V> {
    DuplicateValue(V),
    UnknownValue(V),
    DuplicateSeed(V),
    DuplicateEntity(E),
    Metadata(&'static str),
    State(VariableStateError),
}
impl<E: fmt::Debug, V: fmt::Debug> fmt::Display for BasicSupplyError<E, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "basic state supply: {self:?}")
    }
}
impl<E: fmt::Debug, V: fmt::Debug> Error for BasicSupplyError<E, V> {}
impl<E, V> From<VariableStateError> for BasicSupplyError<E, V> {
    fn from(error: VariableStateError) -> Self {
        Self::State(error)
    }
}
/// Publishes inverse collections for a fixed value universe and a shared field.
///
/// Live inverse queries immediately follow registry updates. Published collections
/// advance on `refresh_changed`, returning typed events for caller-owned storage.
/// Memberships are sorted by stable native entity identity. An optional shadow
/// binding must be a Vec<E> field on V in the same schema; binding twice fails.
/// Unbound supplies instead report membership additions/removals, including their
/// multiplicity. Initial refresh reports every assigned membership. Later refreshes
/// coalesce intermediate registry changes and report only the final difference.
///
/// Unknown values and invalid seeds fail before publication changes. Separate native
/// assignments still need explicit registry synchronization; this does not install
/// reflective setters or validate planning-model domains/pins.
///
/// ```
/// use timetuck::*;
/// struct Solution; struct Job;
/// let schema = PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Job>(
///     vec![PlanningVariableDefinition::basic::<u32>("worker", true)])])?;
/// let field = schema.entity::<Job>()?.variable("worker")?;
/// let mut registry = VariableStateRegistry::new(&schema);
/// registry.reset_basic(field, [(0usize, Some(1u32)), (1, Some(1))])?;
/// let mut supply = BasicStateSupply::new(registry.basic::<usize,u32>(field)?, [1,2])?;
/// assert_eq!(supply.refresh_changed()?.notifications, vec![1,1]);
/// assert_eq!(supply.change(&mut registry, 1, Some(2))?.notifications, vec![1,2]);
/// assert_eq!(supply.inverse(&1), vec![0]);
/// # Ok::<(),Box<dyn std::error::Error>>(())
/// ```
pub struct BasicStateSupply<'s, E, V> {
    state: BasicVariableState<'s, E, V>,
    values: BTreeSet<V>,
    binding: Option<PlanningVariableRef<'s>>,
    published: BTreeMap<V, Vec<E>>,
    observed: BTreeMap<E, Option<V>>,
}
impl<'s, E: Ord + Clone + 'static, V: Ord + Clone + 'static> BasicStateSupply<'s, E, V> {
    pub fn new(
        state: BasicVariableState<'s, E, V>,
        values: impl IntoIterator<Item = V>,
    ) -> Result<Self, BasicSupplyError<E, V>> {
        let mut universe = BTreeSet::new();
        for value in values {
            if !universe.insert(value.clone()) {
                return Err(BasicSupplyError::DuplicateValue(value));
            }
        }
        let published = universe.iter().cloned().map(|v| (v, vec![])).collect();
        let result = Self {
            state,
            values: universe,
            binding: None,
            published,
            observed: BTreeMap::new(),
        };
        result.validate(&result.state.snapshot())?;
        Ok(result)
    }
    pub fn source(&self) -> PlanningVariableRef<'s> {
        self.state.variable()
    }
    pub fn inverse(&self, value: &V) -> Vec<E> {
        if self.values.contains(value) {
            self.state.inverse(value)
        } else {
            vec![]
        }
    }
    /// Last explicitly published or seeded collection; it can lag live state.
    pub fn published_inverse(&self, value: &V) -> Option<&[E]> {
        self.published.get(value).map(Vec::as_slice)
    }
    pub fn externalize(
        &mut self,
        variable: PlanningVariableRef<'s>,
    ) -> Result<(), BasicSupplyError<E, V>> {
        if self.binding.is_some() {
            return Err(BasicSupplyError::Metadata("inverse field already bound"));
        }
        if !std::ptr::eq(
            variable.entity().solution(),
            self.source().entity().solution(),
        ) || variable.entity().entity_type() != PlanningType::of::<V>()
            || variable.kind() != PlanningVariableKind::Shadow
            || variable.value_type() != PlanningType::of::<Vec<E>>()
        {
            return Err(BasicSupplyError::Metadata(
                "inverse must be a Vec<E> shadow on V in the same schema",
            ));
        }
        self.binding = Some(variable);
        Ok(())
    }
    /// Import existing collections. Native identity order is canonical; duplicate
    /// memberships or value records fail atomically. Refresh repairs stale seeds.
    pub fn seed_inverse(
        &mut self,
        rows: impl IntoIterator<Item = (V, Vec<E>)>,
    ) -> Result<(), BasicSupplyError<E, V>> {
        let mut incoming = BTreeMap::new();
        for (value, members) in rows {
            if !self.values.contains(&value) {
                return Err(BasicSupplyError::UnknownValue(value));
            }
            let mut sorted = BTreeSet::new();
            for member in members {
                if !sorted.insert(member.clone()) {
                    return Err(BasicSupplyError::DuplicateEntity(member));
                }
            }
            if incoming
                .insert(value.clone(), sorted.into_iter().collect())
                .is_some()
            {
                return Err(BasicSupplyError::DuplicateSeed(value));
            }
        }
        self.published.extend(incoming);
        Ok(())
    }
    fn validate(&self, rows: &BTreeMap<E, Option<V>>) -> Result<(), BasicSupplyError<E, V>> {
        for value in rows.values().flatten() {
            if !self.values.contains(value) {
                return Err(BasicSupplyError::UnknownValue(value.clone()));
            }
        }
        Ok(())
    }
    pub fn refresh_changed(
        &mut self,
    ) -> Result<BasicStateUpdate<'s, E, V>, BasicSupplyError<E, V>> {
        let rows = self.state.snapshot();
        self.validate(&rows)?;
        let mut update = BasicStateUpdate {
            shadows: vec![],
            notifications: vec![],
        };
        let mut published = self.published.clone();
        if let Some(variable) = self.binding {
            let mut next: BTreeMap<V, Vec<E>> =
                self.values.iter().cloned().map(|v| (v, vec![])).collect();
            for (entity, value) in &rows {
                if let Some(value) = value {
                    next.get_mut(value)
                        .expect("validated value")
                        .push(entity.clone());
                }
            }
            for (value, after) in &next {
                let before = &self.published[value];
                if before != after {
                    update.shadows.push(BasicInverseChange {
                        variable,
                        value: value.clone(),
                        before: before.clone(),
                        after: after.clone(),
                    });
                }
            }
            published = next;
        } else {
            let entities: BTreeSet<_> = rows.keys().chain(self.observed.keys()).collect();
            for entity in entities {
                let before = self.observed.get(entity).and_then(Option::as_ref);
                let after = rows.get(entity).and_then(Option::as_ref);
                if before != after {
                    update.notifications.extend(before.cloned());
                    update.notifications.extend(after.cloned());
                }
            }
        }
        self.published = published;
        self.observed = rows;
        Ok(update)
    }
    /// Update this registry observation field and publish accumulated changes.
    /// Callers with a solver assignment must apply/validate the native move first.
    pub fn change(
        &mut self,
        registry: &mut VariableStateRegistry<'s>,
        entity: E,
        value: Option<V>,
    ) -> Result<BasicStateUpdate<'s, E, V>, BasicSupplyError<E, V>> {
        self.validate(&self.state.snapshot())?;
        if let Some(value) = &value
            && !self.values.contains(value)
        {
            return Err(BasicSupplyError::UnknownValue(value.clone()));
        }
        let bound = registry.basic::<E, V>(self.source())?;
        if !bound.same_instance(&self.state) {
            return Err(BasicSupplyError::Metadata(
                "registry does not own this state",
            ));
        }
        registry.change_basic(self.source(), entity, value)?;
        self.refresh_changed()
    }
}
