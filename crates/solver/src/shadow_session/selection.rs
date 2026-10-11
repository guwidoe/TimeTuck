//! Compile entity assignment and consistency predicates over registered fields.
use super::*;
use crate::PlanningEntityRef;

/// Default entity streams require assignment and consistency. Including
/// unassigned entities relaxes assignment only; inconsistent entities stay out.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShadowEntitySelection {
    AssignedAndConsistent,
    ConsistentIncludingUnassigned,
}
#[derive(Clone)]
struct EntityFields {
    representative: usize,
    basic: Vec<usize>,
    derived: Vec<usize>,
    owner: Option<usize>,
}
/// An immutable entity-selection plan tied to one complete session registration.
/// Compile after registering all fields. Later registrations invalidate the plan
/// rather than silently omitting new fields. Entity identity includes its native
/// planning type; equal keys of different types never share selection state.
///
/// Every basic variable must be registered for each selected entity. A basic None
/// is unassigned even when its domain permits unassigned values. Planning-list
/// owners are assigned even with empty lists. List values require an explicit
/// ownership input/shadow for assigned selection; None means unassigned. A missing
/// binding is an error, not evidence that a value is assigned. Consistency-only
/// selection does not require ownership. Any inconsistent derived field excludes
/// the entity; consistent None shadows do not. External shadow inputs alone do
/// not create a declarative consistency predicate.
///
/// Construction groups Eq-only keys using linear searches, potentially quadratic
/// in registered fields. Subsequent queries use compiled indices, inspect only
/// selected fields, and never invoke readers, suppliers, or compare entity keys.
#[derive(Clone)]
pub struct ShadowEntityFilter {
    identity: Rc<()>,
    registered: usize,
    fields: Vec<EntityFields>,
    group_of: Vec<Option<usize>>,
    list_value: bool,
}
impl ShadowEntityFilter {
    /// Whether declarative suppliers require a consistency predicate. A false
    /// result corresponds to no consistency restriction beyond assignment.
    pub fn requires_consistency(&self) -> bool {
        self.fields.iter().any(|fields| !fields.derived.is_empty())
    }
    pub fn entity_count(&self) -> usize {
        self.fields.len()
    }
}
impl<K: Eq, W> ShadowSessionBuilder<'_, '_, K, W> {
    /// Compile assignment/consistency selection for one canonical entity type.
    /// `owners` contains at most one shadow handle per list value, representing
    /// its owner (native IDs are allowed). Every handle must belong to that value
    /// type and this builder. Omit owners for ordinary basic/list-owner entities.
    pub fn entity_filter(
        &self,
        entity: PlanningEntityRef<'_>,
        owners: &[ShadowNodeRef],
    ) -> Result<ShadowEntityFilter, ShadowSessionError> {
        if !std::ptr::eq(entity.solution(), self.schema) {
            return Err(ShadowSessionError::Metadata(
                "entity belongs to another schema",
            ));
        }
        let list_value = self.schema.entities().any(|owner| {
            owner.variables().any(|v| {
                matches!(v.kind(), PlanningVariableKind::List { .. })
                    && v.value_type() == entity.entity_type()
            })
        });
        if !list_value && !owners.is_empty() {
            return Err(ShadowSessionError::Metadata(
                "ownership bindings require a list value entity",
            ));
        }
        let mut owner_indices = BTreeSet::new();
        for owner in owners {
            let index = lookup(&self.identity, self.nodes.len(), owner)?;
            let variable = self.nodes[index].variable;
            if variable.entity() != entity || variable.kind() != PlanningVariableKind::Shadow {
                return Err(ShadowSessionError::Metadata(
                    "ownership must be a shadow on the selected list value type",
                ));
            }
            if !owner_indices.insert(index) {
                return Err(ShadowSessionError::Metadata("duplicate ownership binding"));
            }
        }
        let groups = self.entity_groups();
        let mut fields = vec![];
        let mut group_of = vec![None; self.nodes.len()];
        for members in groups.groups {
            let representative = members[0];
            if self.nodes[representative].variable.entity() != entity {
                continue;
            }
            let mut basic = vec![];
            for variable in entity
                .variables()
                .filter(|v| matches!(v.kind(), PlanningVariableKind::Basic { .. }))
            {
                let index = members
                    .iter()
                    .copied()
                    .find(|&i| self.nodes[i].variable == variable)
                    .ok_or(ShadowSessionError::Metadata(
                        "all basic variables must be registered for entity selection",
                    ))?;
                basic.push(index);
            }
            let derived = members
                .iter()
                .copied()
                .filter(|&i| matches!(self.nodes[i].operation, Operation::Derived(_)))
                .collect();
            let mut bound = members
                .iter()
                .copied()
                .filter(|i| owner_indices.contains(i));
            let owner = bound.next();
            if bound.next().is_some() {
                return Err(ShadowSessionError::Metadata(
                    "multiple ownership bindings for one list value",
                ));
            }
            for index in members {
                group_of[index] = Some(fields.len());
            }
            fields.push(EntityFields {
                representative,
                basic,
                derived,
                owner,
            });
        }
        Ok(ShadowEntityFilter {
            identity: self.identity.clone(),
            registered: self.nodes.len(),
            fields,
            group_of,
            list_value,
        })
    }
}
impl<K: Eq, W> ShadowSession<'_, '_, K, W> {
    pub(crate) fn validate_entity_filter(
        &self,
        filter: &ShadowEntityFilter,
    ) -> Result<(), ShadowSessionError> {
        if !Rc::ptr_eq(&self.identity, &filter.identity) {
            return Err(ShadowSessionError::ForeignNode);
        }
        if self.nodes.len() != filter.registered {
            return Err(ShadowSessionError::Metadata(
                "entity filter predates the complete session registration",
            ));
        }
        Ok(())
    }
    fn eligible(
        &self,
        filter: &ShadowEntityFilter,
        fields: &EntityFields,
        mode: ShadowEntitySelection,
    ) -> Result<bool, ShadowSessionError> {
        let assigned = if matches!(mode, ShadowEntitySelection::AssignedAndConsistent) {
            let owned = if filter.list_value {
                let owner = fields.owner.ok_or(ShadowSessionError::Metadata(
                    "list value has no ownership binding",
                ))?;
                self.cache.values()[&owner].value.is_some()
            } else {
                true
            };
            owned
                && fields
                    .basic
                    .iter()
                    .all(|i| self.cache.values()[i].value.is_some())
        } else {
            true
        };
        Ok(assigned
            && fields
                .derived
                .iter()
                .all(|i| !self.cache.values()[i].inconsistent))
    }
    /// Select cached entity keys in first registration order. Pending work or
    /// topology edits reject the query. Filtering reads the exact current cache,
    /// including after checkpoint restore; there is no independently mutable flag.
    pub fn selected_entities<'r>(
        &'r self,
        filter: &ShadowEntityFilter,
        mode: ShadowEntitySelection,
    ) -> Result<Vec<&'r K>, ShadowSessionError> {
        self.validate_entity_filter(filter)?;
        self.ready()?;
        let mut keys = vec![];
        for fields in &filter.fields {
            if self.eligible(filter, fields, mode)? {
                keys.push(&self.nodes[fields.representative].entity);
            }
        }
        Ok(keys)
    }
    /// Test the owner of any registered field handle without searching its key.
    pub fn entity_is_selected(
        &self,
        filter: &ShadowEntityFilter,
        node: &ShadowNodeRef,
        mode: ShadowEntitySelection,
    ) -> Result<bool, ShadowSessionError> {
        self.validate_entity_filter(filter)?;
        let index = lookup(&self.identity, self.nodes.len(), node)?;
        let group = filter.group_of[index].ok_or(ShadowSessionError::Metadata(
            "field is not on the selected entity type",
        ))?;
        self.ready()?;
        self.eligible(filter, &filter.fields[group], mode)
    }
}
