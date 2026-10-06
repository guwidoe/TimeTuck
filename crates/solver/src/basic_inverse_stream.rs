//! Inverse collections for selected basic-variable relationships.
use crate::{
    AdditiveScore, GraphError, PlanningStreamSource, SetCollector, SnapshotMap, StreamGraphBuilder,
    StreamPort,
};

/// Value-universe ID -> (value, ordered set of referencing entity identities).
pub type BasicInverseStream<K, V, E> = StreamPort<K, (V, SnapshotMap<E, ()>)>;

/// Derive inverse relationships for one chosen basic-variable field. `entity`
/// maps participating variable indices to stable entity IDs; None excludes other
/// fields. Unassigned variables contribute nothing. Multiple selected references
/// from the same entity have set membership with counted removal semantics.
/// Values in `universe` always emit a row, including empty inverse collections.
/// Values absent from the universe have no output row until they are inserted.
///
/// Universe rows retain their own identity; equal value payloads share membership.
/// Entity/value ordering defines identity and must remain stable. The entity
/// mapping must be deterministic over immutable metadata. Its inverse groups and
/// downstream derived fields share scored-move and session rollback automatically.
/// Works with basic planning sources and MixedStreamSources::basic.
///
/// ```
/// use timetuck::*;
/// let p=FiniteProblem::new(vec![Variable::new(vec![10,20]).with_initial(10)])?;
/// let mut b=StreamGraphBuilder::<SimpleScore>::new();
/// let basics=b.source();
/// let values=b.source::<usize,i32>();
/// let inverse=basic_inverse_stream(&mut b,&basics,values.port(),Some)?;
/// b.score("membership",&inverse,|(_,entities)|SimpleScore::new(entities.len() as i64))?;
/// let mut graph=b.build();
/// let mut facts=graph.transaction();
/// facts.apply(&values,TupleChange::put(0,10))?;
/// facts.apply(&values,TupleChange::put(1,20))?;
/// facts.commit()?;
/// let d=StreamScoreDirector::new(&p,graph,basics,p.initial_assignment())?;
/// assert_eq!(d.score(),SimpleScore::new(1));
/// # Ok::<(),Box<dyn std::error::Error>>(())
/// ```
pub fn basic_inverse_stream<'a, K, V, E, S>(
    builder: &mut StreamGraphBuilder<'a, S>,
    basics: &PlanningStreamSource<V>,
    universe: &StreamPort<K, V>,
    entity: impl Fn(usize) -> Option<E> + 'a,
) -> Result<BasicInverseStream<K, V, E>, GraphError>
where
    K: Ord + Clone + 'static,
    V: Ord + Clone + 'static,
    E: Ord + Clone + 'static,
    S: AdditiveScore + 'static,
{
    let references = builder.flat_map(basics.port(), move |(variable, value)| {
        entity(*variable)
            .zip(value.clone())
            .map(|(entity, value)| (value, entity))
    })?;
    let groups =
        builder.group_collect_with::<_, _, _, _, SetCollector<E>>(&references, |r| r.clone())?;
    let joined = builder.follow_reference(universe, &groups, |value| Some(value.clone()))?;
    builder.map(&joined, |(value, group)| {
        (
            value.clone(),
            group
                .as_ref()
                .map(|(_, entities)| entities.clone())
                .unwrap_or_default(),
        )
    })
}
