//! Compile executable graph factories into a dependency-closed root contract.
use crate::*;
use std::{
    any::Any,
    collections::{HashMap, HashSet},
    error::Error,
    fmt,
    sync::Arc,
};
#[derive(Debug)]
pub enum ObjectClonePlanError {
    Graph(ObjectGraphCloneError),
    MissingDependency(PlanningType),
    UnsupportedRoot {
        encountered: PlanningType,
        solution: PlanningType,
        known: Vec<PlanningType>,
    },
}
impl fmt::Display for ObjectClonePlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Graph(e) => e.fmt(f),
            Self::MissingDependency(t) => write!(
                f,
                "No executable clone factory registered for {}.",
                t.name()
            ),
            Self::UnsupportedRoot {
                encountered,
                solution,
                known,
            } => write!(
                f,
                "Failed to create clone: encountered ({}) which is not a known subclass of the solution class ({}). The known subclasses are: [{}]. Maybe register this root type explicitly?",
                encountered.name(),
                solution.name(),
                known
                    .iter()
                    .map(|t| t.name())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    }
}
impl Error for ObjectClonePlanError {}
impl From<ObjectGraphCloneError> for ObjectClonePlanError {
    fn from(e: ObjectGraphCloneError) -> Self {
        Self::Graph(e)
    }
}
/// Every class dependency has an actual allocate/discover/populate factory.
/// Dependencies explicitly describe required owned/embedded model types; they
/// are not discovered via Java annotations or Rust reflection. Container support
/// factories belong to the same executable registry but are not model classes.
/// Callback traversal must truthfully follow its declared dependency contract.
#[derive(Default)]
pub struct ObjectClonePlanBuilder<'a> {
    registry: ObjectGraphCloneRegistry<'a>,
    dependencies: HashMap<PlanningType, Vec<PlanningType>>,
}
impl<'a> ObjectClonePlanBuilder<'a> {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn containers(&mut self) -> &mut ObjectGraphCloneRegistry<'a> {
        &mut self.registry
    }
    pub fn factory<T: Any + Send + Sync>(
        &mut self,
        dependencies: Vec<PlanningType>,
        allocate: impl Fn(&T) -> Result<T, ObjectGraphCloneError> + 'a,
        discover: impl Fn(&T, &mut ObjectGraphDiscovery<'_, 'a>) -> Result<(), ObjectGraphCloneError>
        + 'a,
        populate: impl Fn(&T, &T, &ObjectGraphCloneView) -> Result<(), ObjectGraphCloneError> + 'a,
    ) -> Result<&mut Self, ObjectClonePlanError> {
        self.registry.register(allocate, discover, populate)?;
        self.dependencies
            .insert(PlanningType::of::<T>(), dependencies);
        Ok(self)
    }
    /// Roots share the same compiled factory dependency closure. `extra` adds
    /// explicit known model types (e.g. another planning entity). Compilation
    /// rejects missing factories before allocating or invoking any callback.
    pub fn compile<S: Any + Send + Sync>(
        self,
        extra: Vec<PlanningType>,
        allowed_roots: Vec<PlanningType>,
    ) -> Result<ObjectClonePlan<'a>, ObjectClonePlanError> {
        let solution = PlanningType::of::<S>();
        let mut known = vec![solution];
        for t in allowed_roots {
            if !known.contains(&t) {
                known.push(t);
            }
        }
        let mut pending = known.clone();
        pending.extend(extra);
        let mut seen = HashSet::new();
        while let Some(t) = pending.pop() {
            if !seen.insert(t) {
                continue;
            }
            let deps = self
                .dependencies
                .get(&t)
                .ok_or(ObjectClonePlanError::MissingDependency(t))?;
            pending.extend(deps.iter().copied());
        }
        let mut classes: Vec<_> = seen.into_iter().collect();
        classes.sort_by_key(|t| t.name());
        Ok(ObjectClonePlan {
            registry: self.registry,
            solution,
            known,
            classes,
        })
    }
}
/// Only dependency-reachable factories are declared model classes. Registered
/// support containers remain available to actual traversal. Unknown roots fail
/// before allocation even if an executable factory was separately registered.
/// Native concrete root identities replace JVM subclass discovery; the diagnostic
/// retains source meaning with an explicit registration remedy instead of GIZMO
/// versus REFLECTION. This does not implement JVM bytecode or Java inheritance.
pub struct ObjectClonePlan<'a> {
    registry: ObjectGraphCloneRegistry<'a>,
    solution: PlanningType,
    known: Vec<PlanningType>,
    classes: Vec<PlanningType>,
}
impl ObjectClonePlan<'_> {
    pub fn deep_cloned_classes(&self) -> &[PlanningType] {
        &self.classes
    }
    pub fn known_roots(&self) -> &[PlanningType] {
        &self.known
    }
    pub fn copy<T: Any + Send + Sync>(
        &self,
        source: &Arc<T>,
    ) -> Result<CopiedObjectGraph<T>, ObjectClonePlanError> {
        let encountered = PlanningType::of::<T>();
        if !self.known.contains(&encountered) {
            return Err(ObjectClonePlanError::UnsupportedRoot {
                encountered,
                solution: self.solution,
                known: self.known.clone(),
            });
        }
        Ok(self.registry.copy(source)?)
    }
}
