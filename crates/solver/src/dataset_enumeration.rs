//! Typed planning enumeration over live stream captures.
//!
//! A synthetic null member belongs to this dataset view, not to the source or
//! scoring graph. It remains present even when all real rows disappear. Native
//! planning models have fixed entity/range membership; custom typed streams may
//! insert/retract members and include pin metadata in their payloads.
use crate::{
    AdditiveScore, FiniteProblem, GraphError, ListProblem, ListStreamSources, ListValueRow,
    NeighborhoodDataset, PlanningStreamSource, StreamDataset, StreamGraph, StreamGraphBuilder,
    StreamPort,
};

/// Defaults enumerate every real row, including pinned and unassigned members.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DatasetEnumerationOptions {
    /// Prepend one synthetic null, distinct from a row with an unassigned value.
    pub include_null: bool,
    pub exclude_pinned: bool,
}

/// Transactional real rows with an optional, permanent null member.
///
/// Iteration borrows rows in ascending identity order and performs no cloning or
/// pin reevaluation. Snapshot materialization owns its values for sampling/joining.
/// Distinct identities retain duplicate payloads. Facts and pin predicates must
/// be immutable/deterministic; deliver explicit tuple updates when facts change.
#[derive(Clone, Debug)]
pub struct EnumeratedDataset<I, T> {
    rows: StreamDataset<I, T>,
    include_null: bool,
}

/// Current basic entity rows keyed by variable index.
pub type BasicEntityDataset<V> = EnumeratedDataset<usize, (usize, Option<V>)>;
/// Current list entity rows keyed by entity index.
pub type ListEntityDataset<V> = EnumeratedDataset<usize, (usize, Vec<V>)>;

impl<I: Ord + Clone + 'static, T: Clone + PartialEq + 'static> EnumeratedDataset<I, T> {
    /// Capture a typed stream with explicit pin semantics. The predicate is only
    /// invoked when exclusion is enabled, and only for inserted/updated tuples.
    /// Graph rollback and consistency diagnostics include the filtered capture.
    ///
    /// ```
    /// use timetuck::*;
    /// let mut b = StreamGraphBuilder::<SimpleScore>::new();
    /// let source = b.source::<usize, (i32, bool)>();
    /// let data = EnumeratedDataset::capture(&mut b, source.port(),
    ///     DatasetEnumerationOptions { include_null: true, exclude_pinned: true },
    ///     |row| row.1)?;
    /// let mut graph = b.build();
    /// let mut tx = graph.transaction();
    /// tx.apply(&source, TupleChange::put(0, (5, false)))?;
    /// tx.apply(&source, TupleChange::put(1, (8, true)))?;
    /// tx.commit()?;
    /// assert_eq!(data.iter(&graph)?.collect::<Vec<_>>(), vec![None, Some(&(5, false))]);
    /// # Ok::<(), GraphError>(())
    /// ```
    pub fn capture<'a, S: AdditiveScore + 'static>(
        builder: &mut StreamGraphBuilder<'a, S>,
        input: &StreamPort<I, T>,
        options: DatasetEnumerationOptions,
        is_pinned: impl Fn(&T) -> bool + 'a,
    ) -> Result<Self, GraphError> {
        let rows = if options.exclude_pinned {
            let movable = builder.filter(input, move |row| !is_pinned(row))?;
            builder.dataset(&movable)?
        } else {
            builder.dataset(input)?
        };
        Ok(Self {
            rows,
            include_null: options.include_null,
        })
    }
}

impl<I: Ord + 'static, T: 'static> EnumeratedDataset<I, T> {
    pub fn len<S: AdditiveScore>(&self, graph: &StreamGraph<'_, S>) -> Result<usize, GraphError> {
        Ok(graph.dataset_rows(&self.rows)?.len() + usize::from(self.include_null))
    }

    pub fn is_empty<S: AdditiveScore>(
        &self,
        graph: &StreamGraph<'_, S>,
    ) -> Result<bool, GraphError> {
        Ok(self.len(graph)? == 0)
    }

    pub fn iter<'g, S: AdditiveScore>(
        &self,
        graph: &'g StreamGraph<'_, S>,
    ) -> Result<impl Iterator<Item = Option<&'g T>>, GraphError> {
        let rows = graph.dataset_rows(&self.rows)?;
        Ok(self
            .include_null
            .then_some(None)
            .into_iter()
            .chain(rows.values().map(Some)))
    }

    pub fn snapshot<S: AdditiveScore>(
        &self,
        graph: &StreamGraph<'_, S>,
    ) -> Result<NeighborhoodDataset<Option<T>>, GraphError>
    where
        T: Clone,
    {
        Ok(NeighborhoodDataset::new(
            self.iter(graph)?.map(|row| row.cloned()),
        ))
    }
}

/// Enumerate basic entities by variable index, retaining unassigned entities.
/// Use with the same problem/source passed to the score director. Pin metadata
/// is fixed for the session; the graph updates the current variable values.
pub fn basic_entity_dataset<V: Eq + Clone + 'static, S: AdditiveScore + 'static>(
    builder: &mut StreamGraphBuilder<'_, S>,
    source: &PlanningStreamSource<V>,
    problem: &FiniteProblem<V>,
    options: DatasetEnumerationOptions,
) -> Result<BasicEntityDataset<V>, GraphError> {
    let pins: Vec<_> = problem.variables().iter().map(|v| v.is_pinned()).collect();
    EnumeratedDataset::capture(builder, source.port(), options, move |row| pins[row.0])
}

/// Enumerate list entities. A pinned prefix alone does not exclude an entity;
/// only a whole-entity pin does. Use this with the matching problem/sources.
pub fn list_entity_dataset<V: Eq + Clone + 'static, S: AdditiveScore + 'static>(
    builder: &mut StreamGraphBuilder<'_, S>,
    sources: &ListStreamSources<V>,
    problem: &ListProblem<V>,
    options: DatasetEnumerationOptions,
) -> Result<ListEntityDataset<V>, GraphError> {
    let pins: Vec<_> = problem.entities().iter().map(|e| e.is_pinned()).collect();
    EnumeratedDataset::capture(builder, sources.entities.port(), options, move |row| {
        pins[row.0]
    })
}

/// Enumerate all registered list values, including unassigned values. Exclusion
/// removes values belonging to wholly pinned entities or their pinned prefixes.
/// Pin decisions use current shadow positions, so committed/trial moves update
/// this dataset with the score director. Use the matching problem/sources.
pub fn list_value_dataset<V: Eq + Clone + 'static, S: AdditiveScore + 'static>(
    builder: &mut StreamGraphBuilder<'_, S>,
    sources: &ListStreamSources<V>,
    problem: &ListProblem<V>,
    options: DatasetEnumerationOptions,
) -> Result<EnumeratedDataset<usize, ListValueRow<V>>, GraphError> {
    let pins: Vec<_> = problem
        .entities()
        .iter()
        .map(|e| (e.is_pinned(), e.pinned_prefix()))
        .collect();
    EnumeratedDataset::capture(builder, sources.values.port(), options, move |row| {
        row.position
            .is_some_and(|p| pins[p.entity].0 || p.index < pins[p.entity].1)
    })
}
