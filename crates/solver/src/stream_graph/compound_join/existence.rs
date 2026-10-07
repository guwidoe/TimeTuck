use super::*;
use std::collections::BTreeSet;

impl<'a, L: Clone + 'static, R: Clone + 'static> JoinConditions<'a, L, R> {
    /// Retain each left occurrence once when the conjunction has a right match,
    /// or no right match in NotExists mode. Empty conditions test right presence.
    /// All equalities form one partition key; the first inequality uses an ordered
    /// range index. Remaining inequalities are independent predicates on indexed
    /// candidates. Equality-only definitions use a linear-state count index.
    /// Ordered definitions retain accepted pair IDs and can require quadratic space
    /// for dense matches. Only left occurrences are emitted downstream.
    ///
    /// Equivalent normalized conditions share within each mode and ordered parent
    /// pair. Join, Exists and NotExists outputs remain distinct. Left payload
    /// updates, right visibility transitions, activity pruning and transactions
    /// follow the native existence nodes' rules. Projection callbacks must be pure.
    ///
    /// ```
    /// use timetuck::*;
    /// let key = JoinProjection::new(|v: &i32| *v);
    /// let conditions = JoinConditions::new([
    ///     JoinCondition::new(JoinComparison::Equal, &key, &key),
    /// ]);
    /// let mut b = StreamGraphBuilder::<SimpleScore>::new();
    /// let demands = b.source::<usize, i32>();
    /// let supplies = b.source::<usize, i32>();
    /// let unmet = conditions.apply_existence(
    ///     &mut b, demands.port(), supplies.port(), ExistenceMode::NotExists)?;
    /// b.score("unmet demand", &unmet, |_| SimpleScore::new(-1))?;
    /// let mut graph = b.build();
    /// let mut tx = graph.transaction();
    /// tx.apply(&demands, TupleChange::put(0, 7))?;
    /// tx.apply(&demands, TupleChange::put(1, 8))?;
    /// tx.apply(&supplies, TupleChange::put(0, 7))?;
    /// tx.commit()?;
    /// assert_eq!(graph.score(), SimpleScore::new(-1));
    /// # Ok::<(), GraphError>(())
    /// ```
    pub fn apply_existence<
        I: Ord + Clone + 'static,
        J: Ord + Clone + 'static,
        S: AdditiveScore + 'static,
    >(
        &self,
        b: &mut StreamGraphBuilder<'a, S>,
        left: &StreamPort<I, L>,
        right: &StreamPort<J, R>,
        mode: ExistenceMode,
    ) -> Result<StreamPort<I, L>, GraphError> {
        b.validate(left)?;
        b.validate(right)?;
        let normalized = self.equals_first();
        let parents = [left.node, right.node];
        let identities: Vec<_> = normalized.0.iter().map(|c| c.identity.clone()).collect();
        if let Some(entry) = b.compound_joins.0.iter().find(|entry| {
            entry.mode == Some(mode) && entry.parents == parents && entry.conditions == identities
        }) {
            return Ok(StreamPort {
                owner: b.owner.clone(),
                node: entry.output,
                marker: PhantomData,
            });
        }
        let equal_count = normalized
            .0
            .iter()
            .take_while(|c| c.comparison() == JoinComparison::Equal)
            .count();
        let lc = normalized.clone();
        let rc = normalized.clone();
        let left_key = move |row: &L| {
            lc.0[..equal_count]
                .iter()
                .map(|c| (c.left)(row))
                .collect::<Vec<_>>()
        };
        let right_key = move |row: &R| {
            rc.0[..equal_count]
                .iter()
                .map(|c| (c.right)(row))
                .collect::<Vec<_>>()
        };
        let out = if equal_count == normalized.0.len() {
            b.existence_equal(left, right, mode, left_key, right_key)?
        } else {
            let ordered_left = normalized.0[equal_count].left.clone();
            let ordered_right = normalized.0[equal_count].right.clone();
            let comparison = normalized.0[equal_count].comparison();
            let out = b.push(IndexedExistence::<I, J, L, R> {
                join: PartitionedJoinNode::new(
                    comparison,
                    move |row| (left_key(row), ordered_left(row)),
                    move |row| (right_key(row), ordered_right(row)),
                ),
                state: ExistenceState::new(),
                mode,
                conditions: normalized,
                residual_start: equal_count + 1,
            });
            if mode == ExistenceMode::NotExists {
                b.activity_rules[out.node] = ActivityRule::Left;
            }
            b.connect(left.node, out.node, false);
            b.connect(right.node, out.node, true);
            out
        };
        b.compound_joins.0.push(Registration {
            mode: Some(mode),
            parents,
            conditions: identities,
            output: out.node,
        });
        Ok(out)
    }
}

#[derive(Clone)]
struct ExistenceState<I, J, L> {
    rows: SnapshotMap<I, L>,
    links: SnapshotMap<(I, J), ()>,
    counts: SnapshotMap<I, usize>,
}
impl<I, J, L> ExistenceState<I, J, L> {
    fn new() -> Self {
        Self {
            rows: SnapshotMap::new(),
            links: SnapshotMap::new(),
            counts: SnapshotMap::new(),
        }
    }
}
impl<I: Ord, J, L> ExistenceState<I, J, L> {
    fn visible(&self, id: &I, mode: ExistenceMode) -> bool {
        self.rows.contains_key(id)
            && (self.counts.contains_key(id) == (mode == ExistenceMode::Exists))
    }
}

// This node is private to the graph: graph checkpoints cover index, accepted-pair
// counts and payloads together, including callback panics during event reduction.
struct IndexedExistence<'a, I, J, L, R> {
    join: PartitionedJoinNode<'a, I, J, L, R, Vec<Key>, Key>,
    state: ExistenceState<I, J, L>,
    mode: ExistenceMode,
    conditions: JoinConditions<'a, L, R>,
    residual_start: usize,
}
impl<
    I: Ord + Clone + 'static,
    J: Ord + Clone + 'static,
    L: Clone + 'static,
    R: Clone + 'static,
    S: AdditiveScore,
> Node<S> for IndexedExistence<'_, I, J, L, R>
{
    fn clear(&mut self) {
        self.join.clear();
        self.state = ExistenceState::new();
    }
    fn checkpoint(&self) -> Box<dyn Any> {
        Box::new((self.join.checkpoint(), self.state.clone()))
    }
    fn restore(&mut self, saved: Box<dyn Any>) {
        let (join, state) = *saved
            .downcast::<(Box<dyn Any>, ExistenceState<I, J, L>)>()
            .expect("compound existence checkpoint");
        self.join.restore(join);
        self.state = state;
    }
    fn apply(
        &mut self,
        right: bool,
        event: &Event,
        _arithmetic: NodeArithmetic,
    ) -> Result<Outputs, GraphError> {
        let before = self.state.clone();
        let mut affected = BTreeSet::new();
        let mut left_update = None;
        let changes = if right {
            self.join.apply_right(typed::<J, R>(event).clone())
        } else {
            let change = typed::<I, L>(event).clone();
            left_update = Some(change.id.clone());
            affected.insert(change.id.clone());
            if let Some(row) = &change.value {
                self.state.rows.insert(change.id.clone(), row.clone());
            } else {
                self.state.rows.remove(&change.id);
            }
            self.join.apply_left(change)
        };
        for change in changes {
            let id = change.id.0.clone();
            affected.insert(id.clone());
            let was = self.state.links.contains_key(&change.id);
            let accepted = change.value.as_ref().is_some_and(|(l, r)| {
                self.conditions.0[self.residual_start..]
                    .iter()
                    .all(|c| c.matches(l, r))
            });
            if was != accepted {
                if accepted {
                    self.state.links.insert(change.id, ());
                    let count = self
                        .state
                        .counts
                        .get(&id)
                        .copied()
                        .unwrap_or(0)
                        .checked_add(1)
                        .expect("existence match count overflow");
                    self.state.counts.insert(id, count);
                } else {
                    self.state.links.remove(&change.id);
                    let count = self.state.counts[&id] - 1;
                    if count == 0 {
                        self.state.counts.remove(&id);
                    } else {
                        self.state.counts.insert(id, count);
                    }
                }
            }
        }
        let mut output = Vec::new();
        for id in affected {
            let was_visible = before.visible(&id, self.mode);
            let visible = self.state.visible(&id, self.mode);
            if visible && (!was_visible || left_update.as_ref() == Some(&id)) {
                output.push(TupleChange::put(id.clone(), self.state.rows[&id].clone()));
            } else if was_visible && !visible {
                output.push(TupleChange::retract(id));
            }
        }
        Ok(erase(output))
    }
}
