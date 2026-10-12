//! Explicit construction order for indexed conditions followed by tuple filters.
use crate::{
    AdditiveScore, GraphError, JoinCondition, JoinConditions, JoinedStreamPort, StreamGraphBuilder,
    StreamPort,
};
type Filter<'a, L, R> = dyn Fn(&L, &R) -> bool + 'a;
enum Condition<'a, L, R> {
    Indexed(JoinCondition<'a, L, R>),
    Filter(Box<Filter<'a, L, R>>),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OrderedJoinError {
    IndexedAfterFiltering,
    Graph(GraphError),
}
impl std::fmt::Display for OrderedJoinError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IndexedAfterFiltering => {
                f.write_str("indexed join conditions must precede filtering conditions")
            }
            Self::Graph(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for OrderedJoinError {}
/// Stores actual ordered callbacks. Validate the entire sequence before adding
/// graph nodes, then use native compound indexes and short-circuiting filters.
/// Direct JoinConditions APIs retain their existing normalization behavior.
pub struct OrderedJoinConditions<'a, L, R> {
    conditions: Vec<Condition<'a, L, R>>,
}
impl<L, R> Default for OrderedJoinConditions<'_, L, R> {
    fn default() -> Self {
        Self { conditions: vec![] }
    }
}
impl<'a, L, R> OrderedJoinConditions<'a, L, R> {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn indexed(mut self, condition: JoinCondition<'a, L, R>) -> Self {
        self.conditions.push(Condition::Indexed(condition));
        self
    }
    pub fn filtering(mut self, predicate: impl Fn(&L, &R) -> bool + 'a) -> Self {
        self.conditions.push(Condition::Filter(Box::new(predicate)));
        self
    }
}
impl<'a, L: Clone + 'static, R: Clone + 'static> OrderedJoinConditions<'a, L, R> {
    pub fn build<I: Ord + Clone + 'static, K: Ord + Clone + 'static, S: AdditiveScore + 'static>(
        self,
        builder: &mut StreamGraphBuilder<'a, S>,
        left: &StreamPort<I, L>,
        right: &StreamPort<K, R>,
    ) -> Result<JoinedStreamPort<I, K, L, R>, OrderedJoinError> {
        let mut indexed = vec![];
        let mut filters = vec![];
        for condition in self.conditions {
            match condition {
                Condition::Indexed(c) => {
                    if !filters.is_empty() {
                        return Err(OrderedJoinError::IndexedAfterFiltering);
                    }
                    indexed.push(c);
                }
                Condition::Filter(f) => filters.push(f),
            }
        }
        let joined = JoinConditions::new(indexed)
            .apply(builder, left, right)
            .map_err(OrderedJoinError::Graph)?;
        if filters.is_empty() {
            return Ok(joined);
        }
        builder
            .filter(&joined, move |(l, r)| {
                filters.iter().all(|filter| filter(l, r))
            })
            .map_err(OrderedJoinError::Graph)
    }
}
