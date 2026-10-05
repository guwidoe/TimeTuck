//! Explicit reusable callback identity for node sharing during graph construction.
use super::*;
use std::{cell::RefCell, rc::Weak};

struct Registration {
    owner: Weak<()>,
    parent: usize,
    output: usize,
}
#[derive(Clone, Default)]
struct Registrations(Rc<RefCell<Vec<Registration>>>);
impl Registrations {
    fn find<I, T>(&self, owner: &Rc<()>, parent: usize) -> Option<StreamPort<I, T>> {
        let mut entries = self.0.borrow_mut();
        entries.retain(|entry| entry.owner.strong_count() != 0);
        let key = Rc::downgrade(owner);
        entries
            .iter()
            .find(|entry| entry.parent == parent && entry.owner.ptr_eq(&key))
            .map(|entry| StreamPort {
                owner: owner.clone(),
                node: entry.output,
                marker: PhantomData,
            })
    }
    fn remember<I, T>(&self, parent: usize, out: &StreamPort<I, T>) {
        self.0.borrow_mut().push(Registration {
            owner: Rc::downgrade(&out.owner),
            parent,
            output: out.node,
        });
    }
}

/// A reusable filter definition. Applying this definition (or a clone) twice to
/// the same parent in the same builder returns the same node. Different parents,
/// builders or freshly created definitions remain independent, even if callbacks
/// have identical bodies. Ordinary builder.filter calls retain their existing
/// independent-node behavior. Callback construction does not run the predicate.
///
/// Shared nodes evaluate each incoming change once and fan out to all consumers;
/// normal graph scoring, rollback and rebuild rules apply. Callbacks must remain
/// pure/deterministic. The definition may be dropped after graph construction.
/// Registrations retain only weak graph identity, prune dead identities on apply,
/// and do not retain graph state. Handles are local to one thread, like the graph.
///
/// ```
/// use timetuck::*;
/// let shared = SharedFilter::new(|v: &i32| *v > 0);
/// let mut b = StreamGraphBuilder::<SimpleScore>::new();
/// let source = b.source::<usize, i32>();
/// let first = shared.apply(&mut b, source.port())?;
/// let second = shared.clone().apply(&mut b, source.port())?;
/// assert_eq!(b.dataset(&first)?, b.dataset(&second)?);
/// # Ok::<(), GraphError>(())
/// ```
pub struct SharedFilter<'a, I, T> {
    callback: Rc<dyn Fn(&T) -> bool + 'a>,
    registrations: Registrations,
    ids: PhantomData<fn() -> I>,
}
impl<I, T> Clone for SharedFilter<'_, I, T> {
    fn clone(&self) -> Self {
        Self {
            callback: self.callback.clone(),
            registrations: self.registrations.clone(),
            ids: PhantomData,
        }
    }
}
impl<'a, I: Ord + Clone + 'static, T: Clone + 'static> SharedFilter<'a, I, T> {
    pub fn new(predicate: impl Fn(&T) -> bool + 'a) -> Self {
        Self {
            callback: Rc::new(predicate),
            registrations: Registrations::default(),
            ids: PhantomData,
        }
    }
    pub fn apply<S: AdditiveScore + 'static>(
        &self,
        b: &mut StreamGraphBuilder<'a, S>,
        input: &StreamPort<I, T>,
    ) -> Result<StreamPort<I, T>, GraphError> {
        b.validate(input)?;
        if let Some(port) = self.registrations.find(&b.owner, input.node) {
            return Ok(port);
        }
        let callback = self.callback.clone();
        let out = b.filter(input, move |v| callback(v))?;
        self.registrations.remember(input.node, &out);
        Ok(out)
    }
}

/// Reusable mapping definition, with the same parent/definition sharing contract
/// as SharedFilter. Output payloads may be tuples or Option values. Reusing a
/// definition never equates distinct callbacks by their code or observed results.
pub struct SharedMap<'a, I, T, U> {
    callback: Rc<dyn Fn(&T) -> U + 'a>,
    registrations: Registrations,
    ids: PhantomData<fn() -> I>,
}
impl<I, T, U> Clone for SharedMap<'_, I, T, U> {
    fn clone(&self) -> Self {
        Self {
            callback: self.callback.clone(),
            registrations: self.registrations.clone(),
            ids: PhantomData,
        }
    }
}
impl<'a, I: Ord + Clone + 'static, T: Clone + 'static, U: Clone + 'static> SharedMap<'a, I, T, U> {
    pub fn new(mapping: impl Fn(&T) -> U + 'a) -> Self {
        Self {
            callback: Rc::new(mapping),
            registrations: Registrations::default(),
            ids: PhantomData,
        }
    }
    pub fn apply<S: AdditiveScore + 'static>(
        &self,
        b: &mut StreamGraphBuilder<'a, S>,
        input: &StreamPort<I, T>,
    ) -> Result<StreamPort<I, U>, GraphError> {
        b.validate(input)?;
        if let Some(port) = self.registrations.find(&b.owner, input.node) {
            return Ok(port);
        }
        let callback = self.callback.clone();
        let out = b.map(input, move |v| callback(v))?;
        self.registrations.remember(input.node, &out);
        Ok(out)
    }
}

type Expansion<'a, T, U> = dyn Fn(&T) -> Vec<U> + 'a;
/// Reusable finite expansion. Each changed input produces positional occurrences
/// `(input_id, position)` including duplicates. Retraction uses captured lengths.
/// Sharing performs expansion once per change even when several constraints use
/// it. IntoIterator results are collected into an owned finite vector; callbacks
/// must return promptly. Tuple mappings can retain prefixes while flattening a
/// final field. Definition identity, not iterator/container type, controls sharing.
pub struct SharedFlatMap<'a, I, T, U> {
    callback: Rc<Expansion<'a, T, U>>,
    registrations: Registrations,
    ids: PhantomData<fn() -> I>,
}
impl<I, T, U> Clone for SharedFlatMap<'_, I, T, U> {
    fn clone(&self) -> Self {
        Self {
            callback: self.callback.clone(),
            registrations: self.registrations.clone(),
            ids: PhantomData,
        }
    }
}
impl<'a, I: Ord + Clone + 'static, T: Clone + 'static, U: Clone + 'static>
    SharedFlatMap<'a, I, T, U>
{
    pub fn new<R: IntoIterator<Item = U>>(mapping: impl Fn(&T) -> R + 'a) -> Self {
        Self {
            callback: Rc::new(move |v| mapping(v).into_iter().collect()),
            registrations: Registrations::default(),
            ids: PhantomData,
        }
    }
    pub fn apply<S: AdditiveScore + 'static>(
        &self,
        b: &mut StreamGraphBuilder<'a, S>,
        input: &StreamPort<I, T>,
    ) -> Result<StreamPort<(I, usize), U>, GraphError> {
        b.validate(input)?;
        if let Some(port) = self.registrations.find(&b.owner, input.node) {
            return Ok(port);
        }
        let callback = self.callback.clone();
        let out = b.flat_map(input, move |v| callback(v))?;
        self.registrations.remember(input.node, &out);
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SimpleScore;
    #[test]
    fn dead_builder_registrations_are_weak_and_pruned_on_next_application() {
        let definition = SharedFilter::<usize, i32>::new(|_| true);
        for _ in 0..100 {
            let mut b = StreamGraphBuilder::<SimpleScore>::new();
            let s = b.source::<usize, i32>();
            definition.apply(&mut b, s.port()).unwrap();
            assert_eq!(definition.registrations.0.borrow().len(), 1);
            let identity = Rc::downgrade(&b.owner);
            drop(s);
            drop(b);
            assert!(identity.upgrade().is_none());
        }
    }
}
