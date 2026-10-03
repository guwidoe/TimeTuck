//! Incremental single-valued references with reverse dependency indexes.
use crate::{SnapshotMap, TupleChange};
use std::any::Any;

#[derive(Clone)]
struct State<I, J, L, R> {
    left: SnapshotMap<I, (Option<J>, L)>,
    right: SnapshotMap<J, R>,
    inverse: SnapshotMap<J, SnapshotMap<I, ()>>,
}
impl<I, J, L, R> State<I, J, L, R> {
    fn new() -> Self {
        Self {
            left: SnapshotMap::new(),
            right: SnapshotMap::new(),
            inverse: SnapshotMap::new(),
        }
    }
}
type ReferenceMapping<'a, L, J> = dyn Fn(&L) -> Option<J> + 'a;

/// Follow an optional target ID, preserving every left row and its identity.
/// Missing targets and absent references produce `None`; an existing target with
/// an optional payload produces `Some(None)`. Target updates visit only current
/// dependents, in left-ID order. Source values/keys must be immutable snapshots.
/// Changes stage new persistent roots before installation, including on unwind.
/// Use `StreamGraphBuilder::follow_reference` to compose transactional chains.
pub struct ReferenceNode<'a, I, J, L, R> {
    state: State<I, J, L, R>,
    reference: Box<ReferenceMapping<'a, L, J>>,
}
impl<'a, I: Ord + Clone, J: Ord + Clone, L: Clone, R: Clone> ReferenceNode<'a, I, J, L, R> {
    pub fn new(reference: impl Fn(&L) -> Option<J> + 'a) -> Self {
        Self {
            state: State::new(),
            reference: Box::new(reference),
        }
    }
    pub fn rows(&self) -> impl Iterator<Item = (&I, (&L, Option<&R>))> {
        self.state.left.iter().map(|(id, (key, row))| {
            (
                id,
                (row, key.as_ref().and_then(|k| self.state.right.get(k))),
            )
        })
    }
    pub fn apply_left(
        &mut self,
        change: TupleChange<I, L>,
    ) -> Option<TupleChange<I, (L, Option<R>)>> {
        let old = self.state.left.get(&change.id);
        let new = change.value.map(|row| ((self.reference)(&row), row));
        let output = new
            .as_ref()
            .map(|(key, row)| {
                TupleChange::put(
                    change.id.clone(),
                    (
                        row.clone(),
                        key.as_ref().and_then(|k| self.state.right.get(k)).cloned(),
                    ),
                )
            })
            .or_else(|| old.map(|_| TupleChange::retract(change.id.clone())));
        let mut next = self.state.clone();
        if let Some((Some(key), _)) = old {
            let mut members = next.inverse.get(key).expect("reference membership").clone();
            members.remove(&change.id);
            if members.is_empty() {
                next.inverse.remove(key);
            } else {
                next.inverse.insert(key.clone(), members);
            }
        }
        if let Some((Some(key), _)) = &new {
            let mut members = next.inverse.get(key).cloned().unwrap_or_default();
            members.insert(change.id.clone(), ());
            next.inverse.insert(key.clone(), members);
        }
        if let Some(row) = new {
            next.left.insert(change.id, row);
        } else {
            next.left.remove(&change.id);
        }
        self.state = next;
        output
    }
    pub fn apply_right(
        &mut self,
        change: TupleChange<J, R>,
    ) -> Vec<TupleChange<I, (L, Option<R>)>> {
        if change.value.is_none() && !self.state.right.contains_key(&change.id) {
            return vec![];
        }
        let output = self
            .state
            .inverse
            .get(&change.id)
            .into_iter()
            .flat_map(|members| members.iter())
            .map(|(id, ())| {
                TupleChange::put(
                    id.clone(),
                    (
                        self.state.left.get(id).expect("dependent row").1.clone(),
                        change.value.clone(),
                    ),
                )
            })
            .collect();
        let mut next = self.state.clone();
        if let Some(value) = change.value {
            next.right.insert(change.id, value);
        } else {
            next.right.remove(&change.id);
        }
        self.state = next;
        output
    }
    pub(crate) fn clear(&mut self) {
        self.state = State::new();
    }
    pub(crate) fn checkpoint(&self) -> Box<dyn Any>
    where
        I: 'static,
        J: 'static,
        L: 'static,
        R: 'static,
    {
        Box::new(self.state.clone())
    }
    pub(crate) fn restore(&mut self, saved: Box<dyn Any>)
    where
        I: 'static,
        J: 'static,
        L: 'static,
        R: 'static,
    {
        self.state = *saved.downcast().expect("reference checkpoint type");
    }
}
