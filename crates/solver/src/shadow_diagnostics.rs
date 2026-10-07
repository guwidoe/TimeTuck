//! Inspect runtime shadow loops and reject inconsistent refreshes atomically.
use super::{ShadowNodeRef, ShadowSession, ShadowSessionCheckpoint, ShadowSessionError};
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
}
impl<K: fmt::Debug> fmt::Display for ConsistentShadowUpdateError<K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Session(e) => e.fmt(f),
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
            Self::Inconsistent(_) => None,
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

/// Update only if every shadow is consistent. An error reports owned loop and
/// affected-field identities from the rejected snapshot, then restores the old
/// cache, dependency edges and pending notifications without invoking callbacks.
/// Callback and diagnostic-construction panics also restore that state. Panics
/// from destructors while disposing of a successful update are excluded. This
/// works before the first update as well as after successful updates. External world/callback effects
/// are not rolled back. For scoring that tolerates inconsistency, use the ordinary
/// `ShadowSession::update` instead.
pub fn update_consistent_shadows<K: Eq + Clone, W>(
    session: &mut ShadowSession<'_, '_, K, W>,
    world: &W,
) -> Result<Vec<ShadowNodeRef>, ConsistentShadowUpdateError<K>> {
    let saved = session.capture();
    let mut guard = RestoreOnDrop {
        session,
        saved: Some(saved),
    };
    let changed = guard.session.update(world)?;
    let report = shadow_consistency(guard.session)?;
    if !report.is_consistent() {
        return Err(ConsistentShadowUpdateError::Inconsistent(report));
    }
    guard.saved = None;
    Ok(changed)
}
