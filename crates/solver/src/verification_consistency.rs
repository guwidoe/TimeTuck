//! Read-only consistency preparation for constraint verification inputs.
use crate::{DependencyError, DependencyGraph};
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

/// A reference to a declarative shadow field on a stable entity identity.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct VerificationShadowRef<K> {
    pub entity: K,
    pub variable: String,
}
/// One shadow field and the shadow fields it reads. Genuine values and immutable
/// facts are not edges; an absent optional reference contributes no edge.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerificationShadow<K> {
    pub variable: String,
    pub dependencies: Vec<VerificationShadowRef<K>>,
}
/// Metadata captured from an entity without evaluating suppliers or writing fields.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerificationEntity<K> {
    pub id: K,
    pub inconsistent: Option<bool>,
    pub shadows: Vec<VerificationShadow<K>>,
}
/// A cross-entity dependency whose target entity was not included in `given`.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct MissingVerificationEntity<K> {
    pub from: VerificationShadowRef<K>,
    pub reference: VerificationShadowRef<K>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VerificationConsistencyError<K> {
    DuplicateEntity(K),
    DuplicateShadow(VerificationShadowRef<K>),
    UnknownGiven(K),
    UnknownShadow(MissingVerificationEntity<K>),
    MissingEntities(Vec<MissingVerificationEntity<K>>),
    Dependency(DependencyError<VerificationShadowRef<K>>),
}
impl<K: fmt::Debug> fmt::Display for VerificationConsistencyError<K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Self::MissingEntities(references) = self {
            write!(f, "Found referenced entities that were not given:")?;
            for entry in references.iter().take(5) {
                write!(
                    f,
                    "\nentity {:?} shadow variable {:?} refers to {:?} on non-given entity {:?}",
                    entry.from.entity,
                    entry.from.variable,
                    entry.reference.variable,
                    entry.reference.entity
                )?;
            }
            if references.len() > 5 {
                write!(f, "\n... and {} more references", references.len() - 5)?;
            }
            Ok(())
        } else {
            write!(f, "verification consistency: {self:?}")
        }
    }
}
impl<K: fmt::Debug> Error for VerificationConsistencyError<K> {}

/// Detached effective flags for the given entities. Lookup distinguishes unknown
/// identities from consistent entities. Assignment filtering remains the caller's
/// responsibility; unfiltered streams can use all entries, including inconsistent ones.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerificationConsistency<K> {
    flags: BTreeMap<K, bool>,
}
impl<K: Ord> VerificationConsistency<K> {
    pub fn is_inconsistent(&self, id: &K) -> Option<bool> {
        self.flags.get(id).copied()
    }
    pub fn iter(&self) -> impl Iterator<Item = (&K, bool)> {
        self.flags.iter().map(|(k, v)| (k, *v))
    }
}

/// Infer missing inconsistency flags without repairing supplied shadows.
///
/// `entities` is the metadata census, including referenced entities needed for
/// diagnostics. `given` selects which entities participate in verification. An
/// indirect reference to an omitted entity is reported too, provided its metadata
/// is in the census. Traversal stops at an unknown entity. Repeated references and
/// repeated given identities are deduplicated; duplicate entity/field definitions
/// are errors. Missing references fail even when a supplied flag is explicit.
///
/// Cycles and their downstream shadow fields are inconsistent. An entity with any
/// such field is inferred inconsistent. Its explicit flag, when present, overrides
/// its own inferred result; it does not remove edges or repair other entities.
/// Detection operates on fields, avoiding false cycles between independent fields
/// on the same entity. All given fields are checked, including those on entities
/// with explicit flags. An unknown field is rejected. No suppliers are invoked,
/// values are not calculated, and caller-owned flags are never modified.
///
/// ```
/// use timetuck::*;
/// let reference = |id| VerificationShadowRef { entity: id, variable: "end".into() };
/// let entities: Vec<_> = (0..2).map(|id| VerificationEntity {
///     id, inconsistent: if id == 0 { Some(false) } else { None },
///     shadows: vec![VerificationShadow { variable: "end".into(),
///         dependencies: vec![reference(1-id)] }],
/// }).collect();
/// let state = verification_consistency(&entities, [0, 1])?;
/// assert_eq!(state.is_inconsistent(&0), Some(false));
/// assert_eq!(state.is_inconsistent(&1), Some(true));
/// assert_eq!(entities[1].inconsistent, None);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn verification_consistency<K: Ord + Clone>(
    entities: &[VerificationEntity<K>],
    given: impl IntoIterator<Item = K>,
) -> Result<VerificationConsistency<K>, VerificationConsistencyError<K>> {
    let mut census = BTreeMap::new();
    let mut fields = BTreeMap::new();
    for entity in entities {
        if census.insert(entity.id.clone(), entity).is_some() {
            return Err(VerificationConsistencyError::DuplicateEntity(
                entity.id.clone(),
            ));
        }
        for shadow in &entity.shadows {
            let field = VerificationShadowRef {
                entity: entity.id.clone(),
                variable: shadow.variable.clone(),
            };
            if fields.insert(field.clone(), &shadow.dependencies).is_some() {
                return Err(VerificationConsistencyError::DuplicateShadow(field));
            }
        }
    }
    let given: BTreeSet<_> = given.into_iter().collect();
    for id in &given {
        if !census.contains_key(id) {
            return Err(VerificationConsistencyError::UnknownGiven(id.clone()));
        }
    }
    let mut pending: Vec<_> = given.iter().cloned().collect();
    let mut visited = BTreeSet::new();
    let mut missing = BTreeSet::new();
    while let Some(id) = pending.pop() {
        if !visited.insert(id.clone()) {
            continue;
        }
        let Some(entity) = census.get(&id) else {
            continue;
        };
        for shadow in &entity.shadows {
            for reference in &shadow.dependencies {
                if reference.entity != id && !given.contains(&reference.entity) {
                    missing.insert(MissingVerificationEntity {
                        from: VerificationShadowRef {
                            entity: id.clone(),
                            variable: shadow.variable.clone(),
                        },
                        reference: reference.clone(),
                    });
                }
                pending.push(reference.entity.clone());
            }
        }
    }
    if !missing.is_empty() {
        return Err(VerificationConsistencyError::MissingEntities(
            missing.into_iter().collect(),
        ));
    }
    let mut edges = Vec::new();
    let nodes: Vec<_> = fields
        .keys()
        .filter(|key| given.contains(&key.entity))
        .cloned()
        .collect();
    for from in &nodes {
        for reference in fields[from].iter() {
            if !fields.contains_key(reference) {
                return Err(VerificationConsistencyError::UnknownShadow(
                    MissingVerificationEntity {
                        from: from.clone(),
                        reference: reference.clone(),
                    },
                ));
            }
            edges.push((from.clone(), reference.clone()));
        }
    }
    let graph = DependencyGraph::new(nodes.into_iter().map(|node| (node, vec![])))
        .map_err(VerificationConsistencyError::Dependency)?;
    let inferred = graph
        .evaluate(&edges, |_, _| Some(()))
        .map_err(VerificationConsistencyError::Dependency)?;
    let mut flags: BTreeMap<_, _> = given.into_iter().map(|id| (id, false)).collect();
    for (field, state) in inferred {
        if state.inconsistent {
            flags.insert(field.entity, true);
        }
    }
    for (id, value) in &mut flags {
        if let Some(explicit) = census[id].inconsistent {
            *value = explicit;
        }
    }
    Ok(VerificationConsistency { flags })
}
