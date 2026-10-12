//! Registered list-owner and shadow-value cloning with value rebasing.
use crate::*;
use std::{
    any::Any,
    collections::{HashMap, HashSet},
    sync::Arc,
};
type Object = Arc<dyn Any + Send + Sync>;
#[derive(Debug)]
pub enum ListSolutionCloneError {
    Binding(ListSolutionBindingError),
    Clone(BasicSolutionCloneError),
    LayoutChanged,
    RegistryChanged,
    AssignmentChanged,
}
impl From<BasicSolutionCloneError> for ListSolutionCloneError {
    fn from(e: BasicSolutionCloneError) -> Self {
        Self::Clone(e)
    }
}
impl std::fmt::Display for ListSolutionCloneError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Binding(e) => e.fmt(f),
            Self::Clone(e) => e.fmt(f),
            Self::LayoutChanged => f.write_str("cloning changed list model layout or pins"),
            Self::RegistryChanged => f.write_str("cloning changed the rebased list value registry"),
            Self::AssignmentChanged => f.write_str("cloning changed the rebased planning lists"),
        }
    }
}
impl std::error::Error for ListSolutionCloneError {}
type Bind<'a, D, V> = dyn Fn(&D) -> Result<BoundListSolution<V>, ListSolutionBindingError> + 'a;
type Factory<'a, V> =
    dyn FnMut(&Object, &[(String, Vec<V>)]) -> Result<Object, ListSolutionCloneError> + 'a;
type Root<'a, D> = dyn FnMut(&D, &BasicEntityClones) -> Result<D, ListSolutionCloneError> + 'a;
type Rebase<'a, V> = dyn FnMut(&V, &BasicEntityClones) -> Result<V, ListSolutionCloneError> + 'a;
type Score<'a, D, S> = dyn Fn(&D) -> Option<S> + 'a;
pub struct CopiedListSolution<D, V> {
    pub solution: D,
    pub binding: BoundListSolution<V>,
    pub entities: BasicEntityClones,
}
/// Allocate shadow-only entities before list owners, rebase registry values once,
/// and supply rebased current lists to owner factories. Root reconstruction can
/// rebuild application inverse/index fields using the completed allocation map.
/// Shared facts can use an identity rebasing callback; registered mutable values
/// can resolve their freshly allocated counterparts through the map.
///
/// Binding, score, allocations, collection aliases, registry order, lists, entity
/// ranges, optionality and pins validate before return. Factories/root callbacks
/// remain responsible for independent mutable storage and application backlinks;
/// native assignment shadows are derived by rebinding. No scoring/search occurs.
/// Errors publish no copy; callback side effects are not undone. Panics propagate.
/// This handles one genuine list field, not mixed/basic or recursive deep fields.
pub struct ListSolutionClonePlan<'a, D, V, S> {
    bind: Box<Bind<'a, D, V>>,
    factories: HashMap<PlanningType, Box<Factory<'a, V>>>,
    root: Box<Root<'a, D>>,
    rebase: Box<Rebase<'a, V>>,
    score: Box<Score<'a, D, S>>,
}
impl<'a, D: 'static, V: Eq + Clone + 'static, S: AdditiveScore + 'static>
    ListSolutionClonePlan<'a, D, V, S>
{
    pub fn new(
        bind: impl Fn(&D) -> Result<BoundListSolution<V>, ListSolutionBindingError> + 'a,
        root: impl FnMut(&D, &BasicEntityClones) -> Result<D, ListSolutionCloneError> + 'a,
        rebase: impl FnMut(&V, &BasicEntityClones) -> Result<V, ListSolutionCloneError> + 'a,
        score: impl Fn(&D) -> Option<S> + 'a,
    ) -> Self {
        Self {
            bind: Box::new(bind),
            factories: HashMap::new(),
            root: Box::new(root),
            rebase: Box::new(rebase),
            score: Box::new(score),
        }
    }
    pub fn entity<E: Any + Send + Sync>(
        &mut self,
        mut factory: impl FnMut(&E, &[(String, Vec<V>)]) -> Result<E, ListSolutionCloneError> + 'a,
    ) -> Result<&mut Self, ListSolutionCloneError> {
        let t = PlanningType::of::<E>();
        if self.factories.contains_key(&t) {
            return Err(BasicSolutionCloneError::DuplicateCloneFactory(t).into());
        }
        self.factories.insert(
            t,
            Box::new(move |object, fields| {
                Ok(Arc::new(factory(
                    object
                        .downcast_ref::<E>()
                        .ok_or(BasicSolutionCloneError::CloneTypeMismatch)?,
                    fields,
                )?) as Object)
            }),
        );
        Ok(self)
    }
    pub fn copy(&mut self, source: &D) -> Result<CopiedListSolution<D, V>, ListSolutionCloneError> {
        let original = (self.bind)(source).map_err(ListSolutionCloneError::Binding)?;
        let layout = original.facts().descriptor();
        if layout.schema().solution_type() != PlanningType::of::<D>()
            || !layout.members().iter().any(
                |m| matches!(m.kind(),SolutionMemberKind::Score(t) if *t==PlanningType::of::<S>()),
            )
        {
            return Err(BasicSolutionCloneError::ScoreTypeMismatch.into());
        }
        for e in original.entities() {
            if !self.factories.contains_key(&e.entity_type()) {
                return Err(BasicSolutionCloneError::MissingCloneFactory(e.entity_type()).into());
            }
        }
        let mut clones = BasicEntityClones::empty();
        for e in original
            .entities()
            .iter()
            .filter(|e| e.list_index().is_none())
        {
            let target = self
                .factories
                .get_mut(&e.entity_type())
                .expect("validated factories")(e.object(), &[])?;
            clones.insert(e.object().clone(), target);
        }
        let p = original.problem();
        let values: Vec<V> = p
            .values()
            .iter()
            .map(|v| (self.rebase)(v, &clones))
            .collect::<Result<_, _>>()?;
        let row = |input: &[V]| -> Result<Vec<V>, ListSolutionCloneError> {
            input
                .iter()
                .map(|v| {
                    p.value_index(v)
                        .map(|i| values[i].clone())
                        .ok_or(ListSolutionCloneError::RegistryChanged)
                })
                .collect()
        };
        let expected: Vec<Vec<V>> = p
            .entities()
            .iter()
            .map(|e| row(e.initial()))
            .collect::<Result<_, _>>()?;
        for e in original
            .entities()
            .iter()
            .filter(|e| e.list_index().is_some())
        {
            let fields = [(
                original.list_variable().to_owned(),
                expected[e.list_index().expect("owner")].clone(),
            )];
            let target = self
                .factories
                .get_mut(&e.entity_type())
                .expect("validated factories")(e.object(), &fields)?;
            clones.insert(e.object().clone(), target);
        }
        let original_score = (self.score)(source);
        let solution = (self.root)(source, &clones)?;
        let score = (self.score)(&solution);
        if score != original_score {
            return Err(BasicSolutionCloneError::ScoreCorruption(format!("Cloning corruption: the original's score {original_score:?} differs from the clone's score {score:?}")).into());
        }
        let binding = (self.bind)(&solution).map_err(ListSolutionCloneError::Binding)?;
        let q = binding.problem();
        if !Arc::ptr_eq(layout, binding.facts().descriptor())
            || original.list_owner() != binding.list_owner()
            || original.list_variable() != binding.list_variable()
            || original.entities().len() != binding.entities().len()
            || original
                .entities()
                .iter()
                .zip(binding.entities())
                .any(|(a, b)| {
                    a.entity_type() != b.entity_type() || a.list_index() != b.list_index()
                })
            || layout.members().iter().any(|m| {
                matches!(m.kind(), SolutionMemberKind::Entities { .. })
                    && original.collection(m.name()) != binding.collection(m.name())
            })
            || p.allows_unassigned() != q.allows_unassigned()
            || p.entities().len() != q.entities().len()
        {
            return Err(ListSolutionCloneError::LayoutChanged);
        }
        let origins: HashSet<_> = original
            .entities()
            .iter()
            .map(|e| Arc::as_ptr(e.object()) as *const () as usize)
            .collect();
        let targets = clones.targets();
        for (clone_index, e) in binding.entities().iter().enumerate() {
            let key = Arc::as_ptr(e.object()) as *const () as usize;
            if origins.contains(&key) {
                return Err(BasicSolutionCloneError::SharedEntity { clone_index }.into());
            }
            if !targets.contains(&key) {
                return Err(BasicSolutionCloneError::UnknownCloneEntity.into());
            }
        }
        if q.values() != values {
            return Err(ListSolutionCloneError::RegistryChanged);
        }
        if q.initial_assignment().lists() != expected {
            return Err(ListSolutionCloneError::AssignmentChanged);
        }
        for (a, b) in p.entities().iter().zip(q.entities()) {
            if a.is_pinned() != b.is_pinned() || a.pinned_prefix() != b.pinned_prefix() {
                return Err(ListSolutionCloneError::LayoutChanged);
            }
            let range = a.value_range().map(row).transpose()?;
            if range.as_deref() != b.value_range() {
                return Err(ListSolutionCloneError::RegistryChanged);
            }
        }
        Ok(CopiedListSolution {
            solution,
            binding,
            entities: clones,
        })
    }
}
