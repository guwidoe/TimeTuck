//! Preflight a supplied entity set, including dependencies of omitted entities.
use crate::{
    CompiledSourcePath, ConsistentShadowUpdateError, ShadowSessionError, ShadowSourceReference,
    ShadowVariableKey,
};
use std::{
    any::Any,
    collections::{HashMap, HashSet, VecDeque},
    hash::Hash,
};

/// Find references to declarative shadow owners absent from `given`, following
/// those owners transitively. Unlike session-level missing-source inspection,
/// this also inspects entities that have no registered session nodes.
///
/// Supply all source paths for the domain from one schema. Only declarative
/// references are followed; builtin fields and ordinary facts are not missing
/// declarative entities. Results follow breadth-first entity order and supplied
/// path/reference order, with duplicate field pairs removed. Cycles terminate.
/// A missing entity's own references may also appear in the report.
///
/// Identity keys must be stable and unique within each concrete entity type.
/// Repeated references to the same object are allowed; distinct objects sharing
/// a type/key are rejected instead of silently omitting part of the graph.
/// Every given or reached entity must have a key. A given type without source
/// paths is permitted. Null links contribute no references.
///
/// This only traverses immutable getters: no terminal values, shadow suppliers,
/// score calculations or setters run. Callback failures propagate, never yielding
/// a partial report. Reports own their keys and names and may outlive the world.
pub fn missing_referenced_shadow_entities<K: Clone + Eq + Hash>(
    paths: &[&CompiledSourcePath<'_, '_>],
    given: &[&dyn Any],
    mut identify: impl FnMut(&dyn Any) -> Option<K>,
) -> Result<Vec<ShadowSourceReference<K>>, ShadowSessionError> {
    if let Some(first) = paths.first() {
        for path in paths {
            if !std::ptr::eq(
                first.target().entity().solution(),
                path.target().entity().solution(),
            ) {
                return Err(ShadowSessionError::Metadata(
                    "source paths belong to different schemas",
                ));
            }
        }
    }
    let mut by_type = HashMap::new();
    for path in paths {
        by_type
            .entry(path.root_type().id())
            .or_insert_with(Vec::new)
            .push(*path);
    }
    let mut present = HashSet::new();
    let mut seen = HashMap::new();
    let mut queue = VecDeque::new();
    for &entity in given {
        let key = identify(entity).ok_or(ShadowSessionError::UnregisteredSource)?;
        let identity = (entity.type_id(), key.clone());
        present.insert(identity.clone());
        if let Some(&previous) = seen.get(&identity) {
            if !std::ptr::addr_eq(previous, entity) {
                return Err(ShadowSessionError::Metadata(
                    "different entities have the same type and key",
                ));
            }
        } else {
            seen.insert(identity, entity);
            queue.push_back((entity, key));
        }
    }
    let mut emitted = HashSet::new();
    let mut missing = Vec::new();
    while let Some((entity, key)) = queue.pop_front() {
        let Some(entity_paths) = by_type.get(&entity.type_id()) else {
            continue;
        };
        for path in entity_paths {
            for reference in path.references().filter(|r| r.is_declarative()) {
                for source in reference
                    .entities(entity)
                    .map_err(ShadowSessionError::SourcePath)?
                {
                    let source_key =
                        identify(source).ok_or(ShadowSessionError::UnregisteredSource)?;
                    let identity = (source.type_id(), source_key.clone());
                    if !present.contains(&identity)
                        && emitted.insert((
                            entity.type_id(),
                            key.clone(),
                            path.target().name(),
                            source.type_id(),
                            source_key.clone(),
                            reference.variable().name(),
                        ))
                    {
                        missing.push(ShadowSourceReference {
                            target: ShadowVariableKey {
                                entity_type: path.root_type(),
                                entity: key.clone(),
                                variable: path.target().name().to_owned(),
                            },
                            source: ShadowVariableKey {
                                entity_type: reference.variable().entity().entity_type(),
                                entity: source_key.clone(),
                                variable: reference.variable().name().to_owned(),
                            },
                        });
                    }
                    if let Some(&previous) = seen.get(&identity) {
                        if !std::ptr::addr_eq(previous, source) {
                            return Err(ShadowSessionError::Metadata(
                                "different entities have the same type and key",
                            ));
                        }
                    } else {
                        seen.insert(identity, source);
                        queue.push_back((source, source_key));
                    }
                }
            }
        }
    }
    Ok(missing)
}

/// Reject an incomplete entity set before refreshing shadows. This can be used
/// in `ShadowSolutionManager::with_preflight`; both strict and tolerant refreshes
/// must reject missing entities. It does not assert that a session contains all
/// fields: session-level preflight still performs that separate check.
pub fn validate_referenced_shadow_entities<K: Clone + Eq + Hash>(
    paths: &[&CompiledSourcePath<'_, '_>],
    given: &[&dyn Any],
    identify: impl FnMut(&dyn Any) -> Option<K>,
) -> Result<(), ConsistentShadowUpdateError<K>> {
    let missing = missing_referenced_shadow_entities(paths, given, identify)
        .map_err(ConsistentShadowUpdateError::Session)?;
    if missing.is_empty() {
        Ok(())
    } else {
        Err(ConsistentShadowUpdateError::MissingSources(missing))
    }
}
