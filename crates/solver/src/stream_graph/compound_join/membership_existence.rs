//! Membership-indexed witnesses for compound existence conditions.
use super::existence::ExistenceState;
use super::*;
use crate::MembershipJoinNode;
struct MembershipExistence<'a, I, J, L, R, K> {
    join: MembershipJoinNode<'a, I, J, L, R, K, Vec<Key>>,
    state: ExistenceState<I, J, L>,
    conditions: JoinConditions<'a, L, R>,
    residual_start: usize,
    mode: ExistenceMode,
}
impl<
    I: Ord + Clone + 'static,
    J: Ord + Clone + 'static,
    L: Clone + 'static,
    R: Clone + 'static,
    K: Ord + Clone + 'static,
    S: AdditiveScore,
> Node<S> for MembershipExistence<'_, I, J, L, R, K>
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
            .expect("membership existence checkpoint");
        self.join.restore(join);
        self.state = state;
    }
    fn apply(
        &mut self,
        right: bool,
        event: &Event,
        _: NodeArithmetic,
    ) -> Result<Outputs, GraphError> {
        let left = (!right).then(|| typed::<I, L>(event));
        let changes = if right {
            self.join.apply_right(typed::<J, R>(event).clone())
        } else {
            self.join.apply_left(left.expect("left input").clone())
        };
        Ok(erase(self.state.reduce(
            left,
            changes,
            self.mode,
            |l, r| {
                self.conditions.0[self.residual_start..]
                    .iter()
                    .all(|c| c.matches(l, r))
            },
        )))
    }
}
impl<'a, L: Clone + 'static, R: Clone + 'static> JoinConditions<'a, L, R> {
    /// Keep each left row once when a right witness shares a collection key and
    /// satisfies every compound condition (Exists), or when no such witness
    /// exists (NotExists). Empty left collections therefore survive NotExists.
    /// Equality partitions and shared-key postings prune candidates before any
    /// ordered residual predicate. Multiple keys and right witnesses never multiply
    /// the emitted left row. Left payload replacements refresh visible output.
    ///
    /// Accepted pair identities are retained for incremental witness counts, so
    /// dense matches can use quadratic space. Candidate pairs are temporary;
    /// downstream nodes receive only left rows. Callback projections are pure,
    /// independent definitions, with no automatic node sharing. Transactions and
    /// activity pruning restore indexes, witnesses and payloads together.
    ///
    /// ```
    /// use timetuck::*;
    /// let mut b=StreamGraphBuilder::<SimpleScore>::new();
    /// let tasks=b.source::<usize,Vec<&'static str>>();
    /// let workers=b.source::<usize,Vec<&'static str>>();
    /// let missing=JoinConditions::new([]).apply_existence_containing_any_of(
    ///     &mut b,tasks.port(),workers.port(),ExistenceMode::NotExists,Clone::clone,Clone::clone)?;
    /// b.score("uncovered task",&missing,|_|SimpleScore::new(-1))?;
    /// let mut g=b.build();let mut tx=g.transaction();
    /// tx.apply(&tasks,TupleChange::put(0,vec!["Rust"]))?;
    /// tx.apply(&workers,TupleChange::put(0,vec!["Java"]))?;
    /// tx.commit()?;assert_eq!(g.score(),SimpleScore::new(-1));
    /// # Ok::<(),GraphError>(())
    /// ```
    pub fn apply_existence_containing_any_of<
        I: Ord + Clone + 'static,
        J: Ord + Clone + 'static,
        K: Ord + Clone + 'static,
        S: AdditiveScore + 'static,
    >(
        &self,
        b: &mut StreamGraphBuilder<'a, S>,
        left: &StreamPort<I, L>,
        right: &StreamPort<J, R>,
        mode: ExistenceMode,
        left_keys: impl Fn(&L) -> Vec<K> + 'a,
        right_keys: impl Fn(&R) -> Vec<K> + 'a,
    ) -> Result<StreamPort<I, L>, GraphError> {
        b.validate(left)?;
        b.validate(right)?;
        let conditions = self.equals_first();
        let equal_count = conditions
            .0
            .iter()
            .take_while(|c| c.comparison() == JoinComparison::Equal)
            .count();
        let lc = conditions.clone();
        let rc = conditions.clone();
        let join = MembershipJoinNode::<I, J, L, R, K, Vec<Key>>::partitioned(
            move |row| {
                (
                    lc.0[..equal_count].iter().map(|c| (c.left)(row)).collect(),
                    left_keys(row),
                )
            },
            move |row| {
                (
                    rc.0[..equal_count].iter().map(|c| (c.right)(row)).collect(),
                    right_keys(row),
                )
            },
        );
        let out = b.push(MembershipExistence {
            join,
            state: ExistenceState::new(),
            conditions,
            residual_start: equal_count,
            mode,
        });
        if mode == ExistenceMode::NotExists {
            b.activity_rules[out.node] = ActivityRule::Left;
        }
        b.connect(left.node, out.node, false);
        b.connect(right.node, out.node, true);
        Ok(out)
    }
    /// Compound existence with a left collection and right scalar key.
    pub fn apply_existence_containing<
        I: Ord + Clone + 'static,
        J: Ord + Clone + 'static,
        K: Ord + Clone + 'static,
        S: AdditiveScore + 'static,
    >(
        &self,
        b: &mut StreamGraphBuilder<'a, S>,
        left: &StreamPort<I, L>,
        right: &StreamPort<J, R>,
        mode: ExistenceMode,
        left_keys: impl Fn(&L) -> Vec<K> + 'a,
        right_key: impl Fn(&R) -> K + 'a,
    ) -> Result<StreamPort<I, L>, GraphError> {
        self.apply_existence_containing_any_of(b, left, right, mode, left_keys, move |row| {
            vec![right_key(row)]
        })
    }
    /// Compound existence with a left scalar key and right collection.
    pub fn apply_existence_contained_in<
        I: Ord + Clone + 'static,
        J: Ord + Clone + 'static,
        K: Ord + Clone + 'static,
        S: AdditiveScore + 'static,
    >(
        &self,
        b: &mut StreamGraphBuilder<'a, S>,
        left: &StreamPort<I, L>,
        right: &StreamPort<J, R>,
        mode: ExistenceMode,
        left_key: impl Fn(&L) -> K + 'a,
        right_keys: impl Fn(&R) -> Vec<K> + 'a,
    ) -> Result<StreamPort<I, L>, GraphError> {
        self.apply_existence_containing_any_of(
            b,
            left,
            right,
            mode,
            move |row| vec![left_key(row)],
            right_keys,
        )
    }
}
