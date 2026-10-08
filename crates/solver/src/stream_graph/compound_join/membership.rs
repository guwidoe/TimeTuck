//! Equality-partitioned collection joins with residual ordered conditions.
use super::*;
impl<'a, L: Clone + 'static, R: Clone + 'static> JoinConditions<'a, L, R> {
    /// Match shared collection keys together with every condition in this definition.
    /// All equalities partition the membership index; ordered comparisons filter
    /// only candidate pairs within matching partitions and shared keys. A pair is
    /// emitted once even if keys repeat or several keys overlap. Empty collections
    /// match nothing. Payload replacement, rollback and pruning follow normal joins.
    ///
    /// Projections must be finite, deterministic and free of external side effects.
    /// These callback-based collection variants create independent nodes; they do
    /// not use the projection-identity sharing of `apply`. Collection-null behavior
    /// is explicit in the callbacks; Option keys can represent nullable elements.
    ///
    /// ```
    /// use timetuck::*;
    /// type Row = (u32, Vec<&'static str>);
    /// let team = JoinProjection::new(|r: &Row| r.0);
    /// let rules = JoinConditions::new([JoinCondition::new(JoinComparison::Equal, &team, &team)]);
    /// let mut b = StreamGraphBuilder::<SimpleScore>::new();
    /// let workers = b.source::<usize, Row>();
    /// let tasks = b.source::<usize, Row>();
    /// let matched = rules.apply_containing_any_of(&mut b, workers.port(), tasks.port(),
    ///     |r| r.1.clone(), |r| r.1.clone())?;
    /// b.score("qualified pairs", &matched, |_| SimpleScore::new(1))?;
    /// let mut g = b.build();
    /// let mut tx = g.transaction();
    /// tx.apply(&workers, TupleChange::put(0, (1, vec!["Rust", "Rust"])))?;
    /// tx.apply(&tasks, TupleChange::put(0, (1, vec!["Rust"])))?;
    /// tx.apply(&tasks, TupleChange::put(1, (2, vec!["Rust"])))?;
    /// tx.commit()?;
    /// assert_eq!(g.score(), SimpleScore::new(1));
    /// # Ok::<(), GraphError>(())
    /// ```
    pub fn apply_containing_any_of<
        I: Ord + Clone + 'static,
        J: Ord + Clone + 'static,
        K: Ord + Clone + 'static,
        S: AdditiveScore + 'static,
    >(
        &self,
        b: &mut StreamGraphBuilder<'a, S>,
        left: &StreamPort<I, L>,
        right: &StreamPort<J, R>,
        left_keys: impl Fn(&L) -> Vec<K> + 'a,
        right_keys: impl Fn(&R) -> Vec<K> + 'a,
    ) -> Result<JoinedStreamPort<I, J, L, R>, GraphError> {
        let normalized = self.equals_first();
        let equal_count = normalized
            .0
            .iter()
            .take_while(|c| c.comparison() == JoinComparison::Equal)
            .count();
        let lc = normalized.clone();
        let rc = normalized.clone();
        let joined = b.join_containing_any_of_partitioned(
            left,
            right,
            move |row| {
                (
                    lc.0[..equal_count]
                        .iter()
                        .map(|c| (c.left)(row))
                        .collect::<Vec<_>>(),
                    left_keys(row),
                )
            },
            move |row| {
                (
                    rc.0[..equal_count]
                        .iter()
                        .map(|c| (c.right)(row))
                        .collect::<Vec<_>>(),
                    right_keys(row),
                )
            },
        )?;
        if equal_count == normalized.0.len() {
            return Ok(joined);
        }
        b.filter(&joined, move |(l, r)| {
            normalized.0[equal_count..].iter().all(|c| c.matches(l, r))
        })
    }
    /// A left collection contains the right scalar, together with all compound
    /// conditions. Duplicate keys do not multiply matches. See
    /// `apply_containing_any_of` for indexing, null and sharing semantics.
    pub fn apply_containing<
        I: Ord + Clone + 'static,
        J: Ord + Clone + 'static,
        K: Ord + Clone + 'static,
        S: AdditiveScore + 'static,
    >(
        &self,
        b: &mut StreamGraphBuilder<'a, S>,
        left: &StreamPort<I, L>,
        right: &StreamPort<J, R>,
        left_keys: impl Fn(&L) -> Vec<K> + 'a,
        right_key: impl Fn(&R) -> K + 'a,
    ) -> Result<JoinedStreamPort<I, J, L, R>, GraphError> {
        self.apply_containing_any_of(b, left, right, left_keys, move |row| vec![right_key(row)])
    }
    /// A left scalar belongs to the right collection, together with all compound
    /// conditions. Empty right collections match nothing.
    pub fn apply_contained_in<
        I: Ord + Clone + 'static,
        J: Ord + Clone + 'static,
        K: Ord + Clone + 'static,
        S: AdditiveScore + 'static,
    >(
        &self,
        b: &mut StreamGraphBuilder<'a, S>,
        left: &StreamPort<I, L>,
        right: &StreamPort<J, R>,
        left_key: impl Fn(&L) -> K + 'a,
        right_keys: impl Fn(&R) -> Vec<K> + 'a,
    ) -> Result<JoinedStreamPort<I, J, L, R>, GraphError> {
        self.apply_containing_any_of(b, left, right, move |row| vec![left_key(row)], right_keys)
    }
}
