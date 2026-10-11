//! Inspect runtime shadow loops and reject inconsistent refreshes atomically.
use super::{
    RefreshMode, ResolvedSources, ShadowNodeRef, ShadowSession, ShadowSessionCheckpoint,
    ShadowSessionError, SourceResolver,
};
use crate::PlanningType;
use std::{error::Error, fmt};

/// An owned identity remains usable after the session or its schema is dropped.
/// Entity type distinguishes equal keys used by different planning entity types.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShadowVariableKey<K> {
    pub entity_type: PlanningType,
    pub entity: K,
    pub variable: String,
}

/// Runtime cycles and all fields made inconsistent by them. Loops contain only
/// mutually reachable members; downstream dependents occur only in `affected`.
/// Both lists, and members within a loop, follow node registration order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShadowConsistencyReport<K> {
    pub loops: Vec<Vec<ShadowVariableKey<K>>>,
    pub affected: Vec<ShadowVariableKey<K>>,
}
impl<K> ShadowConsistencyReport<K> {
    pub fn is_consistent(&self) -> bool {
        self.affected.is_empty()
    }
}

/// Inspect the most recently evaluated snapshot without running any callbacks.
/// Like reads, this rejects pending changes. Changing an external world does not
/// itself notify a session; call update before inspecting that changed world.
pub fn shadow_consistency<K: Eq + Clone, W>(
    session: &ShadowSession<'_, '_, K, W>,
) -> Result<ShadowConsistencyReport<K>, ShadowSessionError> {
    session.ready()?;
    let key = |index: usize| {
        let node = &session.nodes[index];
        ShadowVariableKey {
            entity_type: node.variable.entity().entity_type(),
            entity: node.entity.clone(),
            variable: node.variable.name().to_owned(),
        }
    };
    let topology = session.cache.topology();
    let mut report = ShadowConsistencyReport {
        loops: Vec::new(),
        affected: Vec::new(),
    };
    for (index, value) in session.cache.values() {
        if value.inconsistent {
            report.affected.push(key(*index));
        }
        if topology.is_cyclic(index)? {
            let members = topology.component(index)?;
            if members.first() == Some(index) {
                report
                    .loops
                    .push(members.iter().copied().map(&key).collect());
            }
        }
    }
    Ok(report)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConsistentShadowUpdateError<K> {
    Session(ShadowSessionError),
    Inconsistent(ShadowConsistencyReport<K>),
    MissingSources(Vec<ShadowSourceReference<K>>),
}
impl<K: fmt::Debug> fmt::Display for ConsistentShadowUpdateError<K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Session(e) => e.fmt(f),
            Self::MissingSources(references) => {
                write!(f, "shadow refresh has missing source fields: ")?;
                for (i, reference) in references.iter().enumerate() {
                    if i != 0 {
                        write!(f, "; ")?;
                    }
                    let target = &reference.target;
                    let source = &reference.source;
                    write!(
                        f,
                        "{}({:?}).{} -> {}({:?}).{}",
                        target.entity_type.name(),
                        target.entity,
                        target.variable,
                        source.entity_type.name(),
                        source.entity,
                        source.variable
                    )?;
                }
                Ok(())
            }
            Self::Inconsistent(report) => {
                write!(
                    f,
                    "shadow refresh requires a consistent solution; dependency loops: "
                )?;
                for (i, group) in report.loops.iter().enumerate() {
                    if i != 0 {
                        write!(f, "; ")?;
                    }
                    write!(f, "[")?;
                    for (j, key) in group.iter().enumerate() {
                        if j != 0 {
                            write!(f, ", ")?;
                        }
                        write!(
                            f,
                            "{}({:?}).{}",
                            key.entity_type.name(),
                            key.entity,
                            key.variable
                        )?;
                    }
                    write!(f, "]")?;
                }
                write!(f, " ({} affected fields)", report.affected.len())
            }
        }
    }
}
impl<K: fmt::Debug> Error for ConsistentShadowUpdateError<K> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Session(e) => Some(e),
            Self::Inconsistent(_) | Self::MissingSources(_) => None,
        }
    }
}
impl<K> From<ShadowSessionError> for ConsistentShadowUpdateError<K> {
    fn from(value: ShadowSessionError) -> Self {
        Self::Session(value)
    }
}

struct RestoreOnDrop<'r, 'a, 's, K: Eq, W> {
    session: &'r mut ShadowSession<'a, 's, K, W>,
    saved: Option<ShadowSessionCheckpoint>,
}
impl<K: Eq, W> Drop for RestoreOnDrop<'_, '_, '_, K, W> {
    fn drop(&mut self) {
        if let Some(saved) = &self.saved {
            self.session.install(saved);
        }
    }
}

/// Whether a refreshed graph may publish cycle/affected-field flags for scoring.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ShadowConsistencyPolicy {
    #[default]
    RequireConsistent,
    AllowInconsistent,
}

/// Update only if every source is registered and every shadow is consistent.
/// Paths resolve once; missing sources are collected before input readers or
/// derived suppliers run. An error reports owned missing-source or loop and
/// affected-field identities from the rejected snapshot, then restores the old
/// cache, dependency edges and pending notifications without invoking callbacks.
/// Callback and diagnostic-construction panics also restore that state. Panics
/// from destructors while disposing of a successful update are excluded. This
/// works before the first update as well as after successful updates. External world/callback effects
/// are not rolled back. For scoring that tolerates inconsistency while retaining
/// missing-source diagnostics, use [`update_shadows_with_policy`].
pub fn update_consistent_shadows<K: Eq + Clone, W>(
    session: &mut ShadowSession<'_, '_, K, W>,
    world: &W,
) -> Result<Vec<ShadowNodeRef>, ConsistentShadowUpdateError<K>> {
    update_shadows_with_policy(session, world, ShadowConsistencyPolicy::RequireConsistent)
}

/// Refresh with an explicit consistency policy. AllowInconsistent publishes
/// cycle and downstream inconsistency flags without running their suppliers;
/// project these flags into domain penalties when such states are scoreable.
/// Missing registered sources, resolver errors and callback panics still roll
/// back atomically, with the same diagnostics as strict refresh. Allowing cycles
/// does not treat an arbitrary callback or model error as a valid solution.
pub fn update_shadows_with_policy<K: Eq + Clone, W>(
    session: &mut ShadowSession<'_, '_, K, W>,
    world: &W,
    policy: ShadowConsistencyPolicy,
) -> Result<Vec<ShadowNodeRef>, ConsistentShadowUpdateError<K>> {
    update_shadows_mode(session, world, policy, RefreshMode::All)
}

pub(crate) fn update_shadows_mode<K: Eq + Clone, W>(
    session: &mut ShadowSession<'_, '_, K, W>,
    world: &W,
    policy: ShadowConsistencyPolicy,
    mode: RefreshMode,
) -> Result<Vec<ShadowNodeRef>, ConsistentShadowUpdateError<K>> {
    let saved = session.capture();
    let mut guard = RestoreOnDrop {
        session,
        saved: Some(saved),
    };
    let (resolved, missing) = resolve_with_diagnostics(guard.session, world, mode)?;
    if !missing.is_empty() {
        return Err(ConsistentShadowUpdateError::MissingSources(missing));
    }
    let changed = guard.session.update_resolved(world, resolved, mode)?;
    if policy == ShadowConsistencyPolicy::RequireConsistent {
        let report = shadow_consistency(guard.session)?;
        if !report.is_consistent() {
            return Err(ConsistentShadowUpdateError::Inconsistent(report));
        }
    }
    guard.saved = None;
    Ok(changed)
}

/// A registered target field referring to an entity field absent from the session.
/// Keys and field names are owned, so reports may outlive the evaluated world.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShadowSourceReference<K> {
    pub target: ShadowVariableKey<K>,
    pub source: ShadowVariableKey<K>,
}

/// Resolve compiled source paths and report all missing direct field references
/// from registered targets, in registration/path order. Duplicate target/source
/// pairs occur once. This is a preflight inspection, including before first update
/// or while edits are pending; it does not run input readers or shadow suppliers,
/// consume pending notifications, publish a cache, or invoke setters.
///
/// Only registered targets have path resolvers. This does not discover additional
/// dependencies of missing entities. Null/empty paths have no missing target.
/// Resolver/getter/identity callbacks run normally and must return promptly; their
/// side effects are not rolled back. Path or identity-resolution errors propagate
/// instead of returning an incomplete report. It is legal to inspect a different
/// world without refreshing the session, so this does not assert currentness.
pub fn missing_shadow_sources<K: Eq + Clone, W>(
    session: &ShadowSession<'_, '_, K, W>,
    world: &W,
) -> Result<Vec<ShadowSourceReference<K>>, ShadowSessionError> {
    Ok(resolve_with_diagnostics(session, world, RefreshMode::All)?.1)
}

fn resolve_with_diagnostics<K: Eq + Clone, W>(
    session: &ShadowSession<'_, '_, K, W>,
    world: &W,
    mode: RefreshMode,
) -> Result<(ResolvedSources, Vec<ShadowSourceReference<K>>), ShadowSessionError> {
    let sparse = session.can_resolve_notified(&session.edges, mode);
    let mut missing = Vec::new();
    let mut edges = if sparse {
        Default::default()
    } else {
        session.edges.as_ref().clone()
    };
    let mut signatures = Vec::new();
    let indices: Box<dyn Iterator<Item = usize> + '_> = if sparse {
        Box::new(
            session
                .pending
                .iter()
                .copied()
                .filter(|&i| session.nodes[i].sources.is_some()),
        )
    } else {
        Box::new(0..session.nodes.len())
    };
    for index in indices {
        let node = &session.nodes[index];
        if !mode.includes(index, &session.pending) {
            let signature = session.bindings.signature(index).cloned();
            if let Some(sources) = &signature {
                edges.extend(sources.iter().map(|&source| (index, source)));
            }
            signatures.push((index, signature));
            continue;
        }
        let resolve = match &node.sources {
            Some(SourceResolver::Native(resolve)) => {
                let signature = resolve(world)?;
                if !sparse {
                    edges.extend(signature.iter().map(|&source| (index, source)));
                }
                signatures.push((index, Some(signature)));
                continue;
            }
            Some(SourceResolver::Paths(resolve)) => resolve,
            None => {
                signatures.push((index, None));
                continue;
            }
        };
        let mut signature = Vec::new();
        for (field, entity) in resolve(world)? {
            if let Some(source) = session.lookup_source(field, &entity) {
                if !sparse {
                    edges.insert((index, source));
                }
                signature.push(source);
                continue;
            }
            let reference = ShadowSourceReference {
                target: ShadowVariableKey {
                    entity_type: node.variable.entity().entity_type(),
                    entity: node.entity.clone(),
                    variable: node.variable.name().to_owned(),
                },
                source: ShadowVariableKey {
                    entity_type: field.entity().entity_type(),
                    entity,
                    variable: field.name().to_owned(),
                },
            };
            if !missing.contains(&reference) {
                missing.push(reference);
            }
        }
        signatures.push((index, Some(signature)));
    }
    let resolved = if sparse {
        session.prepare_binding_updates(signatures)
    } else {
        session.share_resolved_sources(
            edges,
            signatures.into_iter().map(|(_, value)| value).collect(),
        )
    };
    Ok((resolved, missing))
}
