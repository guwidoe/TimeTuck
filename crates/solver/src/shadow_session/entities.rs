//! Immutable owner-to-field membership and entity-level shadow notifications.
use super::*;

pub(super) struct EntityGroups {
    pub(super) groups: Vec<Vec<usize>>,
    group_of: Vec<usize>,
}
impl EntityGroups {
    fn indexed<K: Eq, W>(nodes: &[Node<'_, '_, K, W>], index: &KeyedSourceIndex<K>) -> Self {
        // A bucket chains owner groups (not fields), retaining first-registration
        // order even under collisions. Canonical types separate equal native keys.
        let mut buckets = std::collections::HashMap::<(usize, u64), (usize, usize)>::new();
        let mut next = Vec::new();
        let mut groups: Vec<Vec<usize>> = Vec::new();
        let mut group_of = Vec::with_capacity(nodes.len());
        for (position, node) in nodes.iter().enumerate() {
            let owner = node.variable.entity();
            let key = (
                std::ptr::from_ref(owner.definition()) as usize,
                index.entity_hash(&node.entity),
            );
            let mut group = buckets.get(&key).map_or(usize::MAX, |b| b.0);
            while group != usize::MAX {
                let first = &nodes[groups[group][0]];
                if first.variable.entity() == owner && first.entity == node.entity {
                    break;
                }
                group = next[group];
            }
            if group == usize::MAX {
                group = groups.len();
                next.push(usize::MAX);
                buckets
                    .entry(key)
                    .and_modify(|bucket| {
                        next[bucket.1] = group;
                        bucket.1 = group;
                    })
                    .or_insert((group, group));
                groups.push(Vec::new());
            }
            groups[group].push(position);
            group_of.push(group);
        }
        Self { groups, group_of }
    }
    pub(super) fn new<K: Eq, W>(nodes: &[Node<'_, '_, K, W>]) -> Self {
        let mut groups: Vec<Vec<usize>> = Vec::new();
        let mut group_of = Vec::with_capacity(nodes.len());
        for (index, node) in nodes.iter().enumerate() {
            // K deliberately needs only Eq, matching session registration. Owner
            // comparisons happen once during construction, never on notifications.
            let group = groups.iter().position(|members| {
                let first = &nodes[members[0]];
                first.variable.entity() == node.variable.entity() && first.entity == node.entity
            });
            let group = group.unwrap_or_else(|| {
                groups.push(Vec::new());
                groups.len() - 1
            });
            groups[group].push(index);
            group_of.push(group);
        }
        Self { groups, group_of }
    }
}
impl<K: Eq, W> ShadowSessionBuilder<'_, '_, K, W> {
    pub(super) fn entity_groups(&self) -> EntityGroups {
        if let Some(index) = &self.source_index {
            EntityGroups::indexed(&self.nodes, index)
        } else {
            EntityGroups::new(&self.nodes)
        }
    }
}
impl<K: Eq, W> ShadowSession<'_, '_, K, W> {
    /// Refresh a nonempty batch of registered entity owners. Any registered
    /// field can represent its owner; repeated owners are processed once.
    /// Empty/foreign selections fail before dirty state changes or callbacks.
    /// Every selected owner's fields are notified, then update_notified refreshes
    /// them and propagates actual changes to dependents. Unselected external
    /// world facts must remain unchanged, as with update_notified; use update
    /// for a full refresh. Callback failures retain pending notifications for
    /// retry while preserving the previous successful cached results.
    pub fn update_entities(
        &mut self,
        world: &W,
        entities: &[ShadowNodeRef],
    ) -> Result<Vec<ShadowNodeRef>, ShadowSessionError> {
        if entities.is_empty() {
            return Err(ShadowSessionError::EmptyEntityArray);
        }
        self.mark_entities_dirty(entities)?;
        self.update_notified(world)
    }
    fn entity_group(&self, node: &ShadowNodeRef) -> Result<usize, ShadowSessionError> {
        Ok(self.entity_groups.group_of[lookup(&self.identity, self.nodes.len(), node)?])
    }
    fn group_nodes(&self, group: usize) -> Vec<ShadowNodeRef> {
        self.entity_groups.groups[group]
            .iter()
            .map(|&index| ShadowNodeRef {
                index,
                identity: self.identity.clone(),
            })
            .collect()
    }
    /// All registered fields on the owner of `node`, in registration order.
    /// Owner identity is the canonical entity type together with its native key;
    /// equal keys on different types remain distinct. Input and derived fields
    /// are included. Membership is fixed after construction and is available even
    /// before an update or while changes are pending. Foreign handles fail.
    /// Queries use an immutable index without comparing entity keys. Building
    /// that index uses linear owner searches by default because keys require only
    /// `Eq`: O(fields × owners), quadratic in the worst case. Opting into
    /// [`ShadowSessionBuilder::with_keyed_source_index`] gives expected O(fields)
    /// grouping; hash collisions retain the quadratic worst case.
    pub fn entity_nodes(
        &self,
        node: &ShadowNodeRef,
    ) -> Result<Vec<ShadowNodeRef>, ShadowSessionError> {
        Ok(self.group_nodes(self.entity_group(node)?))
    }
    /// Expand selected field handles into unique owner groups. Every group
    /// includes all its registered fields, not only the selected ones. Owners
    /// follow first registration order, and fields follow registration order.
    /// Duplicate or shuffled selections do not duplicate or reorder groups.
    /// This inspects metadata only, without invoking readers or suppliers.
    pub fn affected_entities(
        &self,
        nodes: &[ShadowNodeRef],
    ) -> Result<Vec<Vec<ShadowNodeRef>>, ShadowSessionError> {
        let groups = nodes
            .iter()
            .map(|node| self.entity_group(node))
            .collect::<Result<BTreeSet<_>, _>>()?;
        Ok(groups
            .into_iter()
            .map(|group| self.group_nodes(group))
            .collect())
    }
    /// Mark every registered field of the selected owners dirty. Use this after
    /// changing external facts read by several suppliers on an entity. Input
    /// equality still suppresses propagation; explicitly dirty derived fields
    /// reevaluate even if their declared inputs did not change. Dependents on
    /// other entities are reached by ordinary changed-value propagation.
    ///
    /// The entire selection is validated before pending work changes. Duplicate
    /// owners are processed once. Updates still resolve all dynamic bindings and
    /// sample all input readers, as with [`Self::update`]; this notification does
    /// not promise that unmentioned external world data remained unchanged.
    /// To provide that stronger notification contract and skip unnotified readers
    /// and bindings, call [`Self::update_notified`] explicitly.
    ///
    /// ```
    /// use timetuck::*;
    /// struct Model; struct Task;
    /// let schema = PlanningSchema::new::<Model>(vec![PlanningEntityDefinition::of::<Task>(vec![
    ///     PlanningVariableDefinition::shadow::<i64>("cost"),
    /// ])])?;
    /// let field = schema.entity::<Task>()?.variable("cost")?;
    /// let mut builder = ShadowSessionBuilder::<usize, i64>::new(&schema);
    /// let cost = builder.derive(field, 0, |price, _| Some(*price * 2))?;
    /// let mut session = builder.build();
    /// session.update(&3)?;
    /// session.mark_entities_dirty(&[cost.erase()])?;
    /// session.update(&5)?;
    /// assert_eq!(session.read(&cost)?.value, Some(&10));
    /// assert_eq!(session.changed_entities()?.len(), 1);
    /// session.update(&5)?;
    /// assert!(session.changed_entities()?.is_empty());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn mark_entities_dirty(
        &mut self,
        nodes: &[ShadowNodeRef],
    ) -> Result<(), ShadowSessionError> {
        let groups = nodes
            .iter()
            .map(|node| self.entity_group(node))
            .collect::<Result<BTreeSet<_>, _>>()?;
        for group in groups {
            self.pending
                .extend(self.entity_groups.groups[group].iter().copied());
        }
        Ok(())
    }
    /// Owner groups whose derived values or inconsistency flags changed in the
    /// last successful refresh. Each group includes every registered field of
    /// that owner, allowing projections to publish all its shadows together.
    /// Merely reading inputs, marking a field dirty, or obtaining the same derived
    /// result does not count as a change. Input-only changes are excluded.
    ///
    /// A successful no-change update clears this report. Failed updates preserve
    /// it; checkpoint restore and solution-manager rollback restore the report
    /// along with the cache. Pending notifications or graph edits reject reads
    /// with NotCurrent, matching [`Self::read`]. The report does not accumulate
    /// across updates and does not imply that unreported assignment facts stayed
    /// unchanged. It describes notification history, not a domain value: scores
    /// and `map_shadows` output tuples must not depend on the report itself.
    pub fn changed_entities(&self) -> Result<Vec<Vec<ShadowNodeRef>>, ShadowSessionError> {
        self.ready()?;
        self.affected_entities(&self.last_changed)
    }
}
