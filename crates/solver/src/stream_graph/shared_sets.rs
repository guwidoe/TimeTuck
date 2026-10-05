//! Reusable distinct and multiset concatenation pipelines.
use super::*;
use super::{
    shared::Registrations as UnaryRegistrations,
    shared_relations::Registrations as BinaryRegistrations,
};

/// Reusable distinct-value definition. Clones applied to the same parent share
/// one occurrence index; new definitions or parents stay independent. Ord defines
/// value classes and the lowest input ID supplies their representative payload.
/// Retractions and graph rollback retain the normal distinct-node guarantees.
/// Weak registrations retain no graph state and prune dead owners on application.
pub struct SharedDistinct<I, T> {
    registrations: UnaryRegistrations,
    marker: PhantomData<fn() -> (I, T)>,
}
impl<I, T> Clone for SharedDistinct<I, T> {
    fn clone(&self) -> Self {
        Self {
            registrations: self.registrations.clone(),
            marker: PhantomData,
        }
    }
}
impl<I, T> Default for SharedDistinct<I, T> {
    fn default() -> Self {
        Self {
            registrations: UnaryRegistrations::default(),
            marker: PhantomData,
        }
    }
}
impl<I: Ord + Clone + 'static, T: Ord + Clone + 'static> SharedDistinct<I, T> {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn apply<S: AdditiveScore + 'static>(
        &self,
        b: &mut StreamGraphBuilder<'_, S>,
        input: &StreamPort<I, T>,
    ) -> Result<StreamPort<T, T>, GraphError> {
        b.validate(input)?;
        if let Some(out) = self.registrations.find(&b.owner, input.node) {
            return Ok(out);
        }
        let out = b.distinct(input)?;
        self.registrations.remember(input.node, &out);
        Ok(out)
    }
}

type ConcatMarker<I, J, T> = PhantomData<fn() -> (I, J, T)>;

/// Reusable multiset concatenation. Reapplying this definition or a clone to the
/// same ordered parent pair shares one routing node. Branch-tagged IDs preserve
/// both occurrences when the parents are identical, with no intermediate maps.
/// Fresh definitions, swapped parents and other builders remain independent.
/// Both parents validate before lookup. Registrations hold weak graph identities,
/// prune dead owners on application and use linear lookup in active bindings.
pub struct SharedConcat<I, J, T> {
    registrations: BinaryRegistrations,
    marker: ConcatMarker<I, J, T>,
}
impl<I, J, T> Clone for SharedConcat<I, J, T> {
    fn clone(&self) -> Self {
        Self {
            registrations: self.registrations.clone(),
            marker: PhantomData,
        }
    }
}
impl<I, J, T> Default for SharedConcat<I, J, T> {
    fn default() -> Self {
        Self {
            registrations: BinaryRegistrations::default(),
            marker: PhantomData,
        }
    }
}
impl<I: Ord + Clone + 'static, J: Ord + Clone + 'static, T: Clone + 'static> SharedConcat<I, J, T> {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn apply<S: AdditiveScore + 'static>(
        &self,
        b: &mut StreamGraphBuilder<'_, S>,
        left: &StreamPort<I, T>,
        right: &StreamPort<J, T>,
    ) -> Result<ConcatenatedStreamPort<I, J, T>, GraphError> {
        b.validate(left)?;
        b.validate(right)?;
        let parents = [left.node, right.node];
        if let Some(out) = self.registrations.find(&b.owner, parents) {
            return Ok(out);
        }
        let out = b.concat(left, right)?;
        self.registrations.remember(parents, &out);
        Ok(out)
    }
}

/// Reusable concatenation with explicit conversion/padding to one output type.
/// Each side has a shared mapping, so pairing one parent with several opposite
/// parents still maps each incoming occurrence once. Both mappings must be pure
/// and deterministic. Retraction uses captured outputs without calling mappings.
/// Definitions/clones share identity; fresh definitions retain distinct callbacks.
/// Both parents validate before any topology is added. Built nodes retain their
/// callbacks after definitions are dropped; graph checkpoints cover all branches.
///
/// ```
/// use timetuck::*;
/// let padded = SharedPaddedConcat::new(
///     |v: &i32| (*v, None::<i32>), |v: &(i32, i32)| (v.0, Some(v.1)));
/// let mut b = StreamGraphBuilder::<SimpleScore>::new();
/// let singles = b.source::<usize, i32>();
/// let pairs = b.source::<usize, (i32, i32)>();
/// let out = padded.apply(&mut b, singles.port(), pairs.port())?;
/// assert!(out.same_node(&padded.clone().apply(&mut b, singles.port(), pairs.port())?));
/// # Ok::<(), GraphError>(())
/// ```
pub struct SharedPaddedConcat<'a, I, J, L, R, T> {
    left: SharedMap<'a, I, L, T>,
    right: SharedMap<'a, J, R, T>,
    concat: SharedConcat<I, J, T>,
}
impl<I, J, L, R, T> Clone for SharedPaddedConcat<'_, I, J, L, R, T> {
    fn clone(&self) -> Self {
        Self {
            left: self.left.clone(),
            right: self.right.clone(),
            concat: self.concat.clone(),
        }
    }
}
impl<
    'a,
    I: Ord + Clone + 'static,
    J: Ord + Clone + 'static,
    L: Clone + 'static,
    R: Clone + 'static,
    T: Clone + 'static,
> SharedPaddedConcat<'a, I, J, L, R, T>
{
    pub fn new(left: impl Fn(&L) -> T + 'a, right: impl Fn(&R) -> T + 'a) -> Self {
        Self {
            left: SharedMap::new(left),
            right: SharedMap::new(right),
            concat: SharedConcat::new(),
        }
    }
    pub fn apply<S: AdditiveScore + 'static>(
        &self,
        b: &mut StreamGraphBuilder<'a, S>,
        left: &StreamPort<I, L>,
        right: &StreamPort<J, R>,
    ) -> Result<ConcatenatedStreamPort<I, J, T>, GraphError> {
        b.validate(left)?;
        b.validate(right)?;
        let left = self.left.apply(b, left)?;
        let right = self.right.apply(b, right)?;
        self.concat.apply(b, &left, &right)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shared_set_definitions_validate_before_mutation_and_release_graph_owners() {
        let distinct = SharedDistinct::<usize, i32>::new();
        let concat = SharedConcat::<usize, usize, i32>::new();
        let padded = SharedPaddedConcat::<usize, usize, i32, i32, i32>::new(|v| *v, |v| *v);
        let mut foreign = StreamGraphBuilder::<crate::SimpleScore>::new();
        let foreign_source = foreign.source::<usize, i32>();
        for _ in 0..100 {
            let mut b = StreamGraphBuilder::<crate::SimpleScore>::new();
            let s = b.source::<usize, i32>();
            assert!(matches!(
                distinct.apply(&mut b, foreign_source.port()),
                Err(GraphError::ForeignPort)
            ));
            assert!(matches!(
                concat.apply(&mut b, s.port(), foreign_source.port()),
                Err(GraphError::ForeignPort)
            ));
            assert!(matches!(
                padded.apply(&mut b, s.port(), foreign_source.port()),
                Err(GraphError::ForeignPort)
            ));
            assert!(matches!(
                padded.apply(&mut b, foreign_source.port(), s.port()),
                Err(GraphError::ForeignPort)
            ));
            assert_eq!(b.nodes.len(), 1);
            distinct.apply(&mut b, s.port()).unwrap();
            concat.apply(&mut b, s.port(), s.port()).unwrap();
            padded.apply(&mut b, s.port(), s.port()).unwrap();
            let weak = Rc::downgrade(&b.owner);
            drop(s);
            drop(b);
            assert!(weak.upgrade().is_none());
        }
    }
}
