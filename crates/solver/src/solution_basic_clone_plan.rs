//! Registered entity allocation and alias preservation for typed basic clones.
use crate::*;
use std::{
    any::Any,
    collections::{HashMap, HashSet},
    sync::Arc,
};
type Object = Arc<dyn Any + Send + Sync>;
struct CloneEntry {
    source: Object,
    target: Object,
}
/// A clone map owns both allocations, so dropping the original root cannot make
/// a recycled address look like a known entity. Equal foreign objects are rejected.
pub struct BasicEntityClones {
    entries: HashMap<usize, CloneEntry>,
}
impl BasicEntityClones {
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub fn entity<E: Any + Send + Sync>(
        &self,
        source: &Arc<E>,
    ) -> Result<Arc<E>, BasicSolutionCloneError> {
        let key = Arc::as_ptr(source) as *const () as usize;
        let entry = self
            .entries
            .get(&key)
            .ok_or(BasicSolutionCloneError::UnknownCloneEntity)?;
        let erased: Object = source.clone();
        if !Arc::ptr_eq(&erased, &entry.source) {
            return Err(BasicSolutionCloneError::UnknownCloneEntity);
        }
        entry
            .target
            .clone()
            .downcast::<E>()
            .map_err(|_| BasicSolutionCloneError::CloneTypeMismatch)
    }
    pub fn collection<E: Any + Send + Sync>(
        &self,
        sources: &[Arc<E>],
    ) -> Result<Vec<Arc<E>>, BasicSolutionCloneError> {
        sources.iter().map(|s| self.entity(s)).collect()
    }
}
type Factory<'a, V> =
    dyn FnMut(&Object, &[(String, Option<V>)]) -> Result<Object, BasicSolutionCloneError> + 'a;
type RootFactory<'a, D> =
    dyn FnMut(&D, &BasicEntityClones) -> Result<D, BasicSolutionCloneError> + 'a;
type ScoreRead<'a, D, S> = dyn Fn(&D) -> Option<S> + 'a;
pub struct CopiedBasicSolution<D, V> {
    pub solution: D,
    pub binding: BoundBasicSolution<V>,
    pub entities: BasicEntityClones,
}
/// Allocate every bound entity once, then rebuild the application root using a
/// memoized map. Typed entity factories receive the registered planning values;
/// root factories can preserve aliases across any number of collections/arrays.
/// No scoring or search is performed, and absent scores remain absent.
///
/// Fact sharing and unregistered/deep fields/backlinks are application policies.
/// Factories must provide independent mutable storage. This does not infer Rust
/// object fields or resolve recursive entity references. Errors publish no copy;
/// callback side effects are not rolled back, and panics propagate.
pub struct BasicSolutionClonePlan<'a, D, V, S> {
    factories: HashMap<PlanningType, Box<Factory<'a, V>>>,
    root: Box<RootFactory<'a, D>>,
    score: Box<ScoreRead<'a, D, S>>,
}
impl<'a, D: 'static, V: Eq + Clone + 'static, S: AdditiveScore + 'static>
    BasicSolutionClonePlan<'a, D, V, S>
{
    pub fn new(
        root: impl FnMut(&D, &BasicEntityClones) -> Result<D, BasicSolutionCloneError> + 'a,
        score: impl Fn(&D) -> Option<S> + 'a,
    ) -> Self {
        Self {
            factories: HashMap::new(),
            root: Box::new(root),
            score: Box::new(score),
        }
    }
    pub fn entity<E: Any + Send + Sync>(
        &mut self,
        mut factory: impl FnMut(&E, &[(String, Option<V>)]) -> Result<E, BasicSolutionCloneError> + 'a,
    ) -> Result<&mut Self, BasicSolutionCloneError> {
        let kind = PlanningType::of::<E>();
        if self.factories.contains_key(&kind) {
            return Err(BasicSolutionCloneError::DuplicateCloneFactory(kind));
        }
        self.factories.insert(
            kind,
            Box::new(move |object, fields| {
                let source = object
                    .downcast_ref::<E>()
                    .ok_or(BasicSolutionCloneError::CloneTypeMismatch)?;
                Ok(Arc::new(factory(source, fields)?) as Object)
            }),
        );
        Ok(self)
    }
    pub fn copy(
        &mut self,
        accessors: &SolutionBasicAccessors<'_, D, V>,
        source: &D,
    ) -> Result<CopiedBasicSolution<D, V>, BasicSolutionCloneError> {
        if !accessors
            .layout()
            .members()
            .iter()
            .any(|m| matches!(m.kind(),SolutionMemberKind::Score(t) if *t==PlanningType::of::<S>()))
        {
            return Err(BasicSolutionCloneError::ScoreTypeMismatch);
        }
        let original = accessors
            .bind(source)
            .map_err(BasicSolutionCloneError::Binding)?;
        for entity in original.entities() {
            if !self.factories.contains_key(&entity.entity_type()) {
                return Err(BasicSolutionCloneError::MissingCloneFactory(
                    entity.entity_type(),
                ));
            }
        }
        let assignment = original.problem().initial_assignment();
        let mut clones = BasicEntityClones {
            entries: HashMap::with_capacity(original.entities().len()),
        };
        for entity in original.entities() {
            let fields: Vec<_> = entity
                .slots()
                .iter()
                .map(|(name, index)| (name.clone(), assignment.values()[*index].clone()))
                .collect();
            let target = self
                .factories
                .get_mut(&entity.entity_type())
                .expect("validated factories")(entity.object(), &fields)?;
            clones.entries.insert(
                Arc::as_ptr(entity.object()) as *const () as usize,
                CloneEntry {
                    source: entity.object().clone(),
                    target,
                },
            );
        }
        let score = (self.score)(source);
        let solution = (self.root)(source, &clones)?;
        let copied_score = (self.score)(&solution);
        if score != copied_score {
            return Err(BasicSolutionCloneError::ScoreCorruption(format!(
                "Cloning corruption: the original's score {score:?} differs from the clone's score {copied_score:?}"
            )));
        }
        let binding = crate::solution_basic_clone::validate_basic_clone(
            accessors,
            &original,
            &solution,
            &assignment,
        )?;
        // A root that bypasses the allocated map must not silently discard it.
        let targets: HashSet<_> = clones
            .entries
            .values()
            .map(|entry| Arc::as_ptr(&entry.target) as *const ())
            .collect();
        for entity in binding.entities() {
            if !targets.contains(&(Arc::as_ptr(entity.object()) as *const ())) {
                return Err(BasicSolutionCloneError::UnknownCloneEntity);
            }
        }
        Ok(CopiedBasicSolution {
            solution,
            binding,
            entities: clones,
        })
    }
}
