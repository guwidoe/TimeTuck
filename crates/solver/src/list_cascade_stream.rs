//! Cascading list shadows inside transactional scoring graphs.
use crate::{
    AdditiveScore, DerivedValue, GraphError, ListCascadeDirection, ListPosition, ListProblem,
    ListStreamSources, ListValueRow, StreamGraphBuilder, StreamPort,
};

/// Inputs for a deterministic list shadow. The selected neighbor's shadow has
/// already been derived; the other direction's shadow is None. A selected neighbor
/// may also have no derived value; inspect its identity to distinguish a boundary.
/// Captured problem facts must remain immutable while the graph is in use.
pub struct ListCascadeStep<'a, V, H> {
    pub value: &'a V,
    pub position: ListPosition,
    pub previous: Option<&'a V>,
    pub next: Option<&'a V>,
    pub previous_shadow: Option<&'a H>,
    /// Updated successor shadow for Next cascades; previous_shadow is then None.
    pub next_shadow: Option<&'a H>,
}
/// A stream keyed by the model's stable value-range index.
pub type ListCascadeStream<V, H> = StreamPort<usize, (ListValueRow<V>, DerivedValue<H>)>;

/// Connect a list cascade to scoring. The output keeps each value's registry ID,
/// owner/index/neighbors, derived value and dependency-consistency marker. Every
/// registered value has a row; unassigned values have no shadow and do not invoke
/// the callback. The same sources/model must be used to construct the director.
/// Works with ListStreamSources directly or MixedStreamSources::lists.
///
/// Previous-value edges drive the existing incremental dependency node. Changed
/// value rows are recalculated; changed shadows propagate to successors. Updates,
/// scoring, saved candidates and nested move sessions share graph rollback, which
/// restores shadows without calling the supplier. Source notifications can produce
/// intermediate incomplete/inconsistent rows; the completed scored move is coherent.
/// This does not discover annotations or import partially initialized shadows.
///
/// ```
/// use timetuck::*;
/// let p=ListProblem::new(vec![2i64,3],vec![ListEntity::new(vec![2,3])])?;
/// let mut b=StreamGraphBuilder::<SimpleScore>::new();
/// let sources=ListStreamSources::new(&mut b);
/// let arrivals=list_cascade_stream(&mut b,&sources,&p,|c| {
///     Some(c.previous_shadow.copied().unwrap_or(0)+*c.value)
/// })?;
/// b.score("arrival",&arrivals,|(_,v)|SimpleScore::new(-v.value.unwrap_or(0)))?;
/// let d=ListStreamScoreDirector::new(&p,b.build(),sources,p.initial_assignment())?;
/// assert_eq!(d.score(),SimpleScore::new(-7));
/// # Ok::<(),Box<dyn std::error::Error>>(())
/// ```
pub fn list_cascade_stream<'a, V, H, S>(
    builder: &mut StreamGraphBuilder<'a, S>,
    sources: &ListStreamSources<V>,
    problem: &ListProblem<V>,
    evaluate: impl Fn(ListCascadeStep<'_, V, H>) -> Option<H> + 'a,
) -> Result<ListCascadeStream<V, H>, GraphError>
where
    V: Eq + Clone + 'static,
    H: Eq + Clone + 'static,
    S: AdditiveScore + 'static,
{
    list_cascade_stream_with_direction(
        builder,
        sources,
        problem,
        ListCascadeDirection::Previous,
        evaluate,
    )
}

/// Bind forward or backward list propagation to transactional scoring. Only the
/// selected neighbor supplies a shadow; both neighbor identities remain visible.
/// Next cascades propagate changed shadows towards the head. Unassigned values
/// remain absent and do not invoke suppliers, just as in list_cascade_stream.
///
/// ```
/// use timetuck::*;
/// let p = ListProblem::new(vec![2i64, 3], vec![ListEntity::new(vec![2, 3])])?;
/// let mut b = StreamGraphBuilder::<SimpleScore>::new();
/// let sources = ListStreamSources::new(&mut b);
/// let remaining = list_cascade_stream_with_direction(&mut b, &sources, &p,
///     ListCascadeDirection::Next, |c| Some(c.next_shadow.copied().unwrap_or(0) + *c.value))?;
/// b.score("remaining", &remaining, |(_, v)| SimpleScore::new(-v.value.unwrap_or(0)))?;
/// let d = ListStreamScoreDirector::new(&p, b.build(), sources, p.initial_assignment())?;
/// assert_eq!(d.score(), SimpleScore::new(-8));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn list_cascade_stream_with_direction<'a, V, H, S>(
    builder: &mut StreamGraphBuilder<'a, S>,
    sources: &ListStreamSources<V>,
    problem: &ListProblem<V>,
    direction: ListCascadeDirection,
    evaluate: impl Fn(ListCascadeStep<'_, V, H>) -> Option<H> + 'a,
) -> Result<ListCascadeStream<V, H>, GraphError>
where
    V: Eq + Clone + 'static,
    H: Eq + Clone + 'static,
    S: AdditiveScore + 'static,
{
    let values = problem.values().to_vec();
    builder.derive_dependencies(
        sources.values.port(),
        0..values.len(),
        move |row| {
            match direction {
                ListCascadeDirection::Previous => &row.previous,
                ListCascadeDirection::Next => &row.next,
            }
            .as_ref()
            .map(|v| {
                values
                    .iter()
                    .position(|x| x == v)
                    .expect("cascade neighbor belongs to the bound model")
            })
            .into_iter()
            .collect()
        },
        move |_, row, dependencies| {
            let position = row.position?;
            evaluate(ListCascadeStep {
                value: &row.value,
                position,
                previous: row.previous.as_ref(),
                next: row.next.as_ref(),
                previous_shadow: (direction == ListCascadeDirection::Previous)
                    .then(|| dependencies.first().and_then(|(_, v)| v.value.as_ref()))
                    .flatten(),
                next_shadow: (direction == ListCascadeDirection::Next)
                    .then(|| dependencies.first().and_then(|(_, v)| v.value.as_ref()))
                    .flatten(),
            })
        },
    )
}
