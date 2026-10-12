//! Typed list range, fact-alignment and borrowed entity access without reflection.
use crate::*;
use std::{
    any::{Any, TypeId},
    collections::{BTreeMap, HashMap, HashSet},
    error::Error,
    fmt,
    marker::PhantomData,
    sync::Arc,
};
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanningModelAccessError(pub String);
impl fmt::Display for PlanningModelAccessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl Error for PlanningModelAccessError {}
fn error(text: impl Into<String>) -> PlanningModelAccessError {
    PlanningModelAccessError(text.into())
}
/// Canonical native list metadata checked once against the actual element type.
pub struct ListVariableAccess<'s, V> {
    variable: PlanningVariableRef<'s>,
    element: PhantomData<fn() -> V>,
}
impl<'s, V: 'static> ListVariableAccess<'s, V> {
    pub fn new(variable: PlanningVariableRef<'s>) -> Result<Self, PlanningModelAccessError> {
        if !matches!(variable.kind(), PlanningVariableKind::List { .. }) {
            return Err(error("list access requires a list planning variable"));
        }
        if variable.value_type() != PlanningType::of::<V>() {
            return Err(error("list element type differs from the typed binding"));
        }
        Ok(Self {
            variable,
            element: PhantomData,
        })
    }
    pub fn variable(&self) -> PlanningVariableRef<'s> {
        self.variable
    }
    pub fn element_type(&self) -> PlanningType {
        self.variable.value_type()
    }
    pub fn accepts_type(&self, candidate: PlanningType) -> bool {
        self.element_type() == candidate
    }
    pub fn array_range<P, const N: usize>(
        self,
        read: impl for<'a> Fn(&'a P) -> &'a [V; N] + 'static,
    ) -> ListArrayRange<'s, P, V, N> {
        ListArrayRange {
            access: self,
            read: Box::new(read),
        }
    }
}
type ArrayGetter<P, V, const N: usize> = dyn for<'a> Fn(&'a P) -> &'a [V; N];
pub struct ListArrayRange<'s, P, V, const N: usize> {
    access: ListVariableAccess<'s, V>,
    read: Box<ArrayGetter<P, V, N>>,
}
impl<P, V: Eq + Clone + 'static, const N: usize> ListArrayRange<'_, P, V, N> {
    pub fn read(
        &self,
        solution: &P,
        entity_count: usize,
    ) -> Result<ManagedValueRanges<V>, PlanningSchemaError> {
        self.access
            .variable
            .shared_ranges(vec![(self.read)(solution).to_vec()], entity_count)
    }
}
type KeyGetter<E, K> = dyn Fn(&E) -> K + Send + Sync;
struct FactKey<E, K> {
    get: Arc<KeyGetter<E, K>>,
}
struct KeyEntry {
    key_type: PlanningType,
    reader: Arc<dyn Any + Send + Sync>,
}
/// Named typed readers. Resolution rejects any field which the canonical schema
/// declares as basic, list or shadow; a planning field is never a stable fact key.
pub struct AlignmentKeyRegistry<'s> {
    schema: &'s PlanningSchema,
    readers: HashMap<(PlanningType, String), KeyEntry>,
}
impl<'s> AlignmentKeyRegistry<'s> {
    pub fn new(schema: &'s PlanningSchema) -> Self {
        Self {
            schema,
            readers: HashMap::new(),
        }
    }
    pub fn register<E: 'static, K: 'static>(
        &mut self,
        name: &str,
        get: impl Fn(&E) -> K + Send + Sync + 'static,
    ) -> Result<(), PlanningModelAccessError> {
        let owner = PlanningType::of::<E>();
        self.schema
            .entity::<E>()
            .map_err(|e| error(e.to_string()))?;
        let key = (owner, name.into());
        if name.is_empty() || self.readers.contains_key(&key) {
            return Err(error("duplicate/empty alignment member"));
        }
        self.readers.insert(
            key,
            KeyEntry {
                key_type: PlanningType::of::<K>(),
                reader: Arc::new(FactKey { get: Arc::new(get) }),
            },
        );
        Ok(())
    }
}
/// The typed reader is owned and can outlive its registry/schema; returned groups
/// borrow only the supplied entities. Rust native type identity replaces classes.
pub struct AlignmentKeyMember<E, K> {
    name: String,
    get: Arc<KeyGetter<E, K>>,
}
impl<E: 'static, K> AlignmentKeyMember<E, K> {
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn declaring_type(&self) -> PlanningType {
        PlanningType::of::<E>()
    }
    pub fn key(&self, entity: &E) -> K {
        (self.get)(entity)
    }
    pub fn groups<'a>(&self, entities: impl IntoIterator<Item = &'a E>) -> BTreeMap<K, Vec<&'a E>>
    where
        K: Ord,
    {
        let mut groups = BTreeMap::<K, Vec<&E>>::new();
        for entity in entities {
            groups.entry(self.key(entity)).or_default().push(entity);
        }
        groups
    }
}
pub fn resolve_alignment_key<E: 'static, K: 'static>(
    registry: Option<&AlignmentKeyRegistry<'_>>,
    supplier: Option<&str>,
    shadow: &str,
    key: Option<&str>,
) -> Result<Option<AlignmentKeyMember<E, K>>, PlanningModelAccessError> {
    let Some(key) = key else { return Ok(None) };
    let registry = registry.ok_or_else(|| error("alignment key requires a member registry"))?;
    let entity = registry
        .schema
        .entity::<E>()
        .map_err(|e| error(e.to_string()))?;
    if entity.variable(key).is_ok() {
        return Err(error(format!(
            "@ShadowSources annotated supplier method ({}) for variable ({shadow}) on class ({}) uses a alignmentKey ({key}) that is a variable",
            supplier.unwrap_or("<unspecified>"),
            PlanningType::of::<E>().name()
        )));
    }
    let entry = registry
        .readers
        .get(&(PlanningType::of::<E>(), key.into()))
        .ok_or_else(|| error(format!("unregistered alignment member ({key})")))?;
    if entry.key_type != PlanningType::of::<K>() {
        return Err(error("alignment key binding has the wrong value type"));
    }
    let reader = entry
        .reader
        .downcast_ref::<FactKey<E, K>>()
        .ok_or_else(|| error("alignment callable type differs"))?;
    Ok(Some(AlignmentKeyMember {
        name: key.into(),
        get: reader.get.clone(),
    }))
}
type EntityGetter<P> = dyn for<'a> Fn(&'a P) -> Vec<&'a dyn Any>;
/// Read actual solution members and safely project recognized concrete types to
/// an embedded base entity. Unrelated heterogeneous elements are ignored. Aliased
/// projected entities are retained once by address; equal distinct objects remain.
/// Nonzero-sized target types and at least one declared variable are required.
pub struct BorrowedEntityExtractor<P, E> {
    members: Vec<(String, Box<EntityGetter<P>>)>,
    projections: HashMap<TypeId, Arc<crate::source_member::Read>>,
    target: PhantomData<fn() -> E>,
}
impl<P: 'static, E: 'static> BorrowedEntityExtractor<P, E> {
    pub fn new(schema: &PlanningSchema) -> Result<Self, PlanningModelAccessError> {
        if schema.solution_type() != PlanningType::of::<P>() {
            return Err(error("entity extractor solution type differs from schema"));
        }
        let entity = schema.entity::<E>().map_err(|e| error(e.to_string()))?;
        if entity.definition().variables().is_empty() {
            return Err(error(
                "A planning entity must have at least 1 member with a planning or shadow variable declaration",
            ));
        }
        if std::mem::size_of::<E>() == 0 {
            return Err(error(
                "borrowed entity identity requires a nonzero-sized target type",
            ));
        }
        let mut projections = HashMap::<TypeId, Arc<crate::source_member::Read>>::new();
        projections.insert(
            TypeId::of::<E>(),
            Arc::new(|value| value.downcast_ref::<E>().map(|e| e as &dyn Any)),
        );
        Ok(Self {
            members: vec![],
            projections,
            target: PhantomData,
        })
    }
    pub fn project<C: 'static>(
        &mut self,
        hierarchy: &SourceTypeHierarchy,
    ) -> Result<(), PlanningModelAccessError> {
        if self.projections.contains_key(&TypeId::of::<C>()) {
            return Err(error("duplicate entity projection"));
        }
        let selected = SourceAccessorOptions::<C>::default()
            .field(|entity: &C| Some(entity))
            .select::<P>("entity", "entity", hierarchy)
            .map_err(|e| error(e.to_string()))?;
        let read = selected
            .projected(PlanningType::of::<E>(), hierarchy)
            .map_err(|e| error(e.to_string()))?;
        self.projections.insert(TypeId::of::<C>(), read);
        Ok(())
    }
    pub fn member(
        &mut self,
        name: &str,
        read: impl for<'a> Fn(&'a P) -> Vec<&'a dyn Any> + 'static,
    ) -> Result<(), PlanningModelAccessError> {
        if name.is_empty() || self.members.iter().any(|(n, _)| n == name) {
            return Err(error("duplicate/empty entity member"));
        }
        self.members.push((name.into(), Box::new(read)));
        Ok(())
    }
    pub fn extract<'a>(&self, solution: &'a P) -> Result<Vec<&'a E>, PlanningModelAccessError> {
        let mut out = vec![];
        let mut seen = HashSet::new();
        for (_, read) in &self.members {
            for value in read(solution) {
                if let Some(project) = self.projections.get(&value.type_id()) {
                    let entity = project(value)
                        .and_then(|v| v.downcast_ref::<E>())
                        .ok_or_else(|| {
                            error("registered entity projection returned the wrong type")
                        })?;
                    if seen.insert(entity as *const E) {
                        out.push(entity);
                    }
                }
            }
        }
        Ok(out)
    }
}
