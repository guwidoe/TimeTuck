//! Explicit before-change graph detachment and opt-in committed edge reports.
use super::*;
/// A real applied dependency, oriented from prerequisite to dependent.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShadowDependencyEdge {
    pub source: ShadowNodeRef,
    pub target: ShadowNodeRef,
}
pub struct ShadowDependencyUpdate {
    pub changed: Vec<ShadowNodeRef>,
    pub removed: Vec<ShadowDependencyEdge>,
    pub added: Vec<ShadowDependencyEdge>,
}
impl<K: Eq, W> ShadowSession<'_, '_, K, W> {
    fn edge_reports(&self, edges: &[(usize, usize)]) -> Vec<ShadowDependencyEdge> {
        edges
            .iter()
            .map(|&(target, source)| ShadowDependencyEdge {
                source: ShadowNodeRef {
                    index: source,
                    identity: self.identity.clone(),
                },
                target: ShadowNodeRef {
                    index: target,
                    identity: self.identity.clone(),
                },
            })
            .collect()
    }
    /// Inspect actual applied edges, including removals at a before-change
    /// boundary. Cached values remain unreadable until all pending work flushes.
    pub fn dependency_edges(&self) -> Vec<ShadowDependencyEdge> {
        let topology = self.cache.topology();
        let edges: Vec<_> = (0..self.nodes.len())
            .flat_map(|target| {
                topology
                    .dependency_indices(target)
                    .iter()
                    .map(move |&source| (target, source))
            })
            .collect();
        self.edge_reports(&edges)
    }
    pub fn assert_current(&self) -> Result<(), ShadowSessionError> {
        self.ready()
    }
    /// Before external facts change, detach all dynamically resolved incoming
    /// dependencies of these targets from the actual applied graph. Callers map
    /// their changed facts to the targets whose resolvers read them. Manual/fixed
    /// dependencies cannot be detached here. Validate the entire selection before
    /// mutation; duplicates/repeated notifications remove each edge once. Readers
    /// and suppliers do not run. Pending refresh resolves replacements from the
    /// final world. Failure/panic in that refresh preserves detached pending state.
    pub fn before_dependencies_changed(
        &mut self,
        targets: &[ShadowNodeRef],
    ) -> Result<Vec<ShadowDependencyEdge>, ShadowSessionError> {
        let mut selected = BTreeSet::new();
        for target in targets {
            let index = lookup(&self.identity, self.nodes.len(), target)?;
            if self.nodes[index].sources.is_none() {
                return Err(ShadowSessionError::Metadata(
                    "before-change target requires resolved dependencies",
                ));
            }
            selected.insert(index);
        }
        let resolved =
            self.prepare_binding_updates(selected.iter().map(|&i| (i, Some(vec![]))).collect());
        let reports = self.edge_reports(&resolved.removals);
        let mut next = self.cache.clone();
        let mut pending = self.pending.clone();
        pending.extend(selected);
        pending.extend(next.detach_edges(&resolved.removals)?);
        self.cache = next;
        self.bindings = resolved.bindings;
        self.pending = pending;
        Ok(reports)
    }
    /// Flush notified work and report only committed graph changes. All report
    /// allocations precede publication. Ordinary update/update_notified avoid
    /// this reporting cost. No edges are reported for a failed refresh.
    pub fn update_notified_with_dependency_events(
        &mut self,
        world: &W,
    ) -> Result<ShadowDependencyUpdate, ShadowSessionError> {
        let resolved = self.resolve_sources_mode(world, &self.edges, RefreshMode::Notified)?;
        let removed = self.edge_reports(&resolved.removals);
        let added = self.edge_reports(&resolved.additions);
        let changed = self.update_resolved(world, resolved, RefreshMode::Notified)?;
        Ok(ShadowDependencyUpdate {
            changed,
            removed,
            added,
        })
    }
}
