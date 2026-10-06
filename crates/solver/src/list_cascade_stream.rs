//! Cascading list shadows inside transactional scoring graphs.
use crate::{
    AdditiveScore, DerivedValue, GraphError, ListPosition, ListProblem, ListStreamSources,
    ListValueRow, StreamGraphBuilder, StreamPort,
};

/// Inputs for a deterministic list shadow. The predecessor's shadow has already
/// been derived. None also represents an assigned predecessor with no derived
/// value; inspect previous to distinguish it from the head of a list.
/// Captured problem facts must remain immutable while the graph is in use.
pub struct ListCascadeStep<'a, V, H> {
    pub value: &'a V,
    pub position: ListPosition,
    pub previous: Option<&'a V>,
    pub next: Option<&'a V>,
    pub previous_shadow: Option<&'a H>,
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
    let values = problem.values().to_vec();
    builder.derive_dependencies(
        sources.values.port(),
        0..values.len(),
        move |row| {
            row.previous
                .as_ref()
                .map(|v| {
                    values
                        .iter()
                        .position(|x| x == v)
                        .expect("cascade predecessor belongs to the bound model")
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
                previous_shadow: dependencies.first().and_then(|(_, v)| v.value.as_ref()),
            })
        },
    )
}
