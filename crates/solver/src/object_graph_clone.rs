//! Iterative allocation/discovery/population of explicitly registered object graphs.
use crate::*;
use std::{
    any::Any,
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::{Arc, Weak},
};
type Object = Arc<dyn Any + Send + Sync>;
#[derive(Debug)]
pub enum ObjectGraphCloneError {
    MissingFactory(PlanningType),
    DuplicateFactory(PlanningType),
    UndiscoveredReference,
    TypeMismatch,
    Callback(String),
    Binding(BasicSolutionCloneError),
}
impl std::fmt::Display for ObjectGraphCloneError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingFactory(t) => write!(f, "missing graph clone factory for {}", t.name()),
            Self::DuplicateFactory(t) => {
                write!(f, "duplicate graph clone factory for {}", t.name())
            }
            Self::UndiscoveredReference => f.write_str("clone reference was not discovered"),
            Self::TypeMismatch => f.write_str("graph clone type mismatch"),
            Self::Callback(s) => f.write_str(s),
            Self::Binding(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for ObjectGraphCloneError {}
impl From<BasicSolutionCloneError> for ObjectGraphCloneError {
    fn from(e: BasicSolutionCloneError) -> Self {
        Self::Binding(e)
    }
}
struct Entry {
    source: Object,
    target: Object,
    kind: PlanningType,
}
/// Owns both original and copied allocations; lookups use actual identity, not
/// equality or an address whose allocation might already have been freed.
pub struct ObjectGraphCloneView {
    entries: Vec<Entry>,
    indices: HashMap<usize, usize>,
    registered: HashSet<PlanningType>,
}
impl ObjectGraphCloneView {
    pub(crate) fn has_factory<T: Any + Send + Sync>(&self) -> bool {
        self.registered.contains(&PlanningType::of::<T>())
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub fn required<T: Any + Send + Sync>(
        &self,
        source: &Arc<T>,
    ) -> Result<Arc<T>, ObjectGraphCloneError> {
        let key = Arc::as_ptr(source) as *const () as usize;
        let entry = self
            .indices
            .get(&key)
            .and_then(|i| self.entries.get(*i))
            .ok_or(ObjectGraphCloneError::UndiscoveredReference)?;
        let erased: Object = source.clone();
        if !Arc::ptr_eq(&erased, &entry.source) {
            return Err(ObjectGraphCloneError::UndiscoveredReference);
        }
        entry
            .target
            .clone()
            .downcast::<T>()
            .map_err(|_| ObjectGraphCloneError::TypeMismatch)
    }
    /// Preserve explicitly shared facts; discovered objects resolve to copies.
    pub fn copied_or_shared<T: Any + Send + Sync>(
        &self,
        source: &Arc<T>,
    ) -> Result<Arc<T>, ObjectGraphCloneError> {
        if self
            .indices
            .contains_key(&(Arc::as_ptr(source) as *const () as usize))
        {
            self.required(source)
        } else {
            Ok(source.clone())
        }
    }
    pub fn weak<T: Any + Send + Sync>(
        &self,
        source: &Weak<T>,
    ) -> Result<Weak<T>, ObjectGraphCloneError> {
        match source.upgrade() {
            Some(source) => Ok(Arc::downgrade(&self.required(&source)?)),
            None => Ok(Weak::new()),
        }
    }
    pub fn collection<T: Any + Send + Sync>(
        &self,
        source: &[Arc<T>],
    ) -> Result<Vec<Arc<T>>, ObjectGraphCloneError> {
        source.iter().map(|s| self.required(s)).collect()
    }
}
type Allocate<'a> = dyn Fn(&Object) -> Result<Object, ObjectGraphCloneError> + 'a;
type Discover<'a> =
    dyn Fn(&Object, &mut ObjectGraphDiscovery<'_, 'a>) -> Result<(), ObjectGraphCloneError> + 'a;
type Populate<'a> =
    dyn Fn(&Object, &Object, &ObjectGraphCloneView) -> Result<(), ObjectGraphCloneError> + 'a;
struct Rule<'a> {
    allocate: Box<Allocate<'a>>,
    discover: Box<Discover<'a>>,
    populate: Box<Populate<'a>>,
}
/// Discovery only allocates/enqueues nodes. It does not recurse into their edges
/// or expose destination shells. Repeated references allocate just once.
pub struct ObjectGraphDiscovery<'r, 'a> {
    registry: &'r ObjectGraphCloneRegistry<'a>,
    view: ObjectGraphCloneView,
}
impl<'a> ObjectGraphDiscovery<'_, 'a> {
    pub(crate) fn has_factory<T: Any + Send + Sync>(&self) -> bool {
        self.registry.rules.contains_key(&PlanningType::of::<T>())
    }
    pub fn required<T: Any + Send + Sync>(
        &mut self,
        source: &Arc<T>,
    ) -> Result<(), ObjectGraphCloneError> {
        let key = Arc::as_ptr(source) as *const () as usize;
        if self.view.indices.contains_key(&key) {
            return Ok(());
        }
        let kind = PlanningType::of::<T>();
        let rule = self
            .registry
            .rules
            .get(&kind)
            .ok_or(ObjectGraphCloneError::MissingFactory(kind))?;
        let source: Object = source.clone();
        let target = (rule.allocate)(&source)?;
        self.view.indices.insert(key, self.view.entries.len());
        self.view.entries.push(Entry {
            source,
            target,
            kind,
        });
        Ok(())
    }
    pub fn weak<T: Any + Send + Sync>(
        &mut self,
        source: &Weak<T>,
    ) -> Result<(), ObjectGraphCloneError> {
        if let Some(s) = source.upgrade() {
            self.required(&s)?;
        }
        Ok(())
    }
}
pub struct CopiedObjectGraph<T> {
    pub root: Arc<T>,
    pub objects: ObjectGraphCloneView,
}
pub struct CopiedBasicObjectGraph<T, V> {
    pub graph: CopiedObjectGraph<T>,
    pub binding: BoundBasicSolution<V>,
}
/// Explicit typed shell/edge/fill factories preserve aliases, deep fields and
/// weak backlinks without recursive traversal. All shells are allocated before
/// any fill callback, so cyclic references can resolve to their copied targets.
/// Application payloads use safe mutable/once-initialized storage for filling;
/// immutable facts may be shared explicitly. The registry infers no Rust fields.
///
/// Callbacks must describe all required edges and read stable source objects.
/// Errors return no graph; callback side effects and escaped shells are not undone.
/// Strong ownership cycles and deep Arc destruction retain ordinary Rust semantics;
/// use Weak backlinks and application cleanup where appropriate. Panics propagate.
/// Copy work is O(discovered objects + declared edges), excluding callback work.
pub struct ObjectGraphCloneRegistry<'a> {
    rules: HashMap<PlanningType, Rc<Rule<'a>>>,
}
impl Default for ObjectGraphCloneRegistry<'_> {
    fn default() -> Self {
        Self::new()
    }
}
impl<'a> ObjectGraphCloneRegistry<'a> {
    pub fn new() -> Self {
        Self {
            rules: HashMap::new(),
        }
    }
    pub fn register<T: Any + Send + Sync>(
        &mut self,
        allocate: impl Fn(&T) -> Result<T, ObjectGraphCloneError> + 'a,
        discover: impl Fn(&T, &mut ObjectGraphDiscovery<'_, 'a>) -> Result<(), ObjectGraphCloneError>
        + 'a,
        populate: impl Fn(&T, &T, &ObjectGraphCloneView) -> Result<(), ObjectGraphCloneError> + 'a,
    ) -> Result<&mut Self, ObjectGraphCloneError> {
        let kind = PlanningType::of::<T>();
        if self.rules.contains_key(&kind) {
            return Err(ObjectGraphCloneError::DuplicateFactory(kind));
        }
        self.rules.insert(
            kind,
            Rc::new(Rule {
                allocate: Box::new(move |source| {
                    Ok(Arc::new(allocate(
                        source
                            .downcast_ref::<T>()
                            .ok_or(ObjectGraphCloneError::TypeMismatch)?,
                    )?) as Object)
                }),
                discover: Box::new(move |source, context| {
                    discover(
                        source
                            .downcast_ref::<T>()
                            .ok_or(ObjectGraphCloneError::TypeMismatch)?,
                        context,
                    )
                }),
                populate: Box::new(move |source, target, view| {
                    populate(
                        source
                            .downcast_ref::<T>()
                            .ok_or(ObjectGraphCloneError::TypeMismatch)?,
                        target
                            .downcast_ref::<T>()
                            .ok_or(ObjectGraphCloneError::TypeMismatch)?,
                        view,
                    )
                }),
            }),
        );
        Ok(self)
    }
    pub fn copy<T: Any + Send + Sync>(
        &self,
        source: &Arc<T>,
    ) -> Result<CopiedObjectGraph<T>, ObjectGraphCloneError> {
        let mut context = ObjectGraphDiscovery {
            registry: self,
            view: ObjectGraphCloneView {
                entries: Vec::new(),
                indices: HashMap::new(),
                registered: self.rules.keys().copied().collect(),
            },
        };
        context.required(source)?;
        let mut cursor = 0;
        while cursor < context.view.entries.len() {
            let entry = &context.view.entries[cursor];
            let source = entry.source.clone();
            let rule = self.rules[&entry.kind].clone();
            (rule.discover)(&source, &mut context)?;
            cursor += 1;
        }
        for entry in &context.view.entries {
            (self.rules[&entry.kind].populate)(&entry.source, &entry.target, &context.view)?;
        }
        let root = context.view.required(source)?;
        Ok(CopiedObjectGraph {
            root,
            objects: context.view,
        })
    }
    /// Copy an actual typed basic input and rebind all cloned planning objects.
    /// Preserves score (including absence), registered fields/ranges and aliases.
    pub fn copy_basic<T: Any + Send + Sync, V: Eq + Clone + 'static, S: AdditiveScore + 'static>(
        &self,
        accessors: &SolutionBasicAccessors<'_, Arc<T>, V>,
        source: &Arc<T>,
        score: impl Fn(&T) -> Option<S>,
    ) -> Result<CopiedBasicObjectGraph<T, V>, ObjectGraphCloneError> {
        if !accessors
            .layout()
            .members()
            .iter()
            .any(|m| matches!(m.kind(),SolutionMemberKind::Score(t) if *t==PlanningType::of::<S>()))
        {
            return Err(BasicSolutionCloneError::ScoreTypeMismatch.into());
        }
        let original = accessors
            .bind(source)
            .map_err(BasicSolutionCloneError::Binding)?;
        let before = score(source);
        let graph = self.copy(source)?;
        let after = score(&graph.root);
        if before != after {
            return Err(BasicSolutionCloneError::ScoreCorruption(format!("Cloning corruption: the original's score {before:?} differs from the clone's score {after:?}")).into());
        }
        let binding = crate::solution_basic_clone::validate_basic_clone(
            accessors,
            &original,
            &graph.root,
            &original.problem().initial_assignment(),
        )?;
        Ok(CopiedBasicObjectGraph { graph, binding })
    }
}
