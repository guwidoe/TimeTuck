//! Typed working-object identity lookup with explicit planning-ID metadata.
use std::{any::type_name, collections::BTreeMap, error::Error, fmt, sync::Arc};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkingLookupStrategy {
    /// Require declared planning IDs except for explicitly immutable objects.
    IdOrFail,
    /// Ignore objects with no declared ID when adding/removing; lookup still fails.
    IdOrNone,
    /// Skip all registration/ID validation and reject non-null lookup.
    Disabled,
}
/// One explicitly declared ID field/accessor. Key extraction must be deterministic;
/// registered IDs must not change through interior mutability.
pub struct PlanningIdMember<T: ?Sized, K> {
    pub name: &'static str,
    pub read: fn(&T) -> Option<K>,
}
impl<T: ?Sized, K> PlanningIdMember<T, K> {
    pub fn new(name: &'static str, read: fn(&T) -> Option<K>) -> Self {
        Self { name, read }
    }
}
/// Native metadata replaces annotation discovery. ID-bearing schemas must declare
/// exactly one non-null ID; validation occurs when an operation needs the schema.
/// Immutable objects pass through lookup without registration (unless disabled).
pub enum WorkingLookupSchema<T: ?Sized, K> {
    PlanningIds(Vec<PlanningIdMember<T, K>>),
    Immutable,
}
impl<T: ?Sized, K> WorkingLookupSchema<T, K> {
    pub fn id(name: &'static str, read: fn(&T) -> Option<K>) -> Self {
        Self::PlanningIds(vec![PlanningIdMember::new(name, read)])
    }
    pub fn without_id() -> Self {
        Self::PlanningIds(vec![])
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkingLookupError<K> {
    MissingIdMetadata {
        object_type: &'static str,
    },
    MultipleIdMembers {
        object_type: &'static str,
        members: Vec<&'static str>,
    },
    NullId {
        object_type: &'static str,
        member: &'static str,
    },
    DuplicateId(K),
    UnknownId(K),
    DifferentWorkingObject(K),
    Unsupported {
        object_type: &'static str,
    },
}
impl<K: fmt::Debug> fmt::Display for WorkingLookupError<K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingIdMetadata { object_type } => {
                write!(f, "{object_type} does not declare a planning ID")
            }
            Self::MultipleIdMembers {
                object_type,
                members,
            } => write!(
                f,
                "{object_type} declares {} planning ID members: {members:?}",
                members.len()
            ),
            Self::NullId {
                object_type,
                member,
            } => write!(f, "{object_type}.{member} planning ID must not be null"),
            Self::DuplicateId(k) => {
                write!(f, "two working objects have the same planning ID {k:?}")
            }
            Self::UnknownId(k) => write!(
                f,
                "external object with planning ID {k:?} has no known working object"
            ),
            Self::DifferentWorkingObject(k) => write!(
                f,
                "supplied and registered working objects differ for planning ID {k:?}"
            ),
            Self::Unsupported { object_type } => write!(
                f,
                "{object_type} cannot be looked up under this strategy/schema"
            ),
        }
    }
}
impl<K: fmt::Debug> Error for WorkingLookupError<K> {}

/// Optional concrete object diagnostics around the ordinary typed lookup error.
/// Captured only on failure; canonical registration and ID validation are unchanged.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkingLookupObjectError<K> {
    pub cause: WorkingLookupError<K>,
    pub object: String,
}
impl<K: fmt::Debug> fmt::Display for WorkingLookupObjectError<K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}; object {}", self.cause, self.object)
    }
}
impl<K: fmt::Debug + 'static> Error for WorkingLookupObjectError<K> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.cause)
    }
}

/// Registry for a Rust concrete type or explicit trait/interface view. IDs are scoped to this registry;
/// separate registries isolate model/entity types even when their key types match.
/// Lookup returns the original stored Arc, not an equal clone of its payload.
/// Removing by external same-ID clone is rejected. Reset-with is atomic on errors
/// and supplier panics. External mutations/side effects are outside its rollback.
///
/// ```
/// use std::sync::Arc;
/// use timetuck::*;
/// #[derive(Debug)] struct Job { id: u64, label: &'static str }
/// let mut lookup=WorkingObjectLookup::new(WorkingLookupStrategy::IdOrFail,
///     WorkingLookupSchema::id("id",|job:&Job|Some(job.id)));
/// let working=Arc::new(Job{id:7,label:"working"});
/// lookup.add(working.clone())?;
/// let external=Arc::new(Job{id:7,label:"external"});
/// let found=lookup.lookup(&external)?;
/// assert!(Arc::ptr_eq(&found,&working));
/// assert_eq!(found.label,"working");
/// # Ok::<(),WorkingLookupError<u64>>(())
/// ```
pub struct WorkingObjectLookup<T: ?Sized, K> {
    strategy: WorkingLookupStrategy,
    schema: WorkingLookupSchema<T, K>,
    objects: BTreeMap<K, Arc<T>>,
}
impl<T: ?Sized, K: Ord> WorkingObjectLookup<T, K> {
    pub fn new(strategy: WorkingLookupStrategy, schema: WorkingLookupSchema<T, K>) -> Self {
        Self {
            strategy,
            schema,
            objects: BTreeMap::new(),
        }
    }
    pub fn len(&self) -> usize {
        self.objects.len()
    }
    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }
    fn key(&self, object: &T) -> Result<Option<K>, WorkingLookupError<K>> {
        if self.strategy == WorkingLookupStrategy::Disabled {
            return Ok(None);
        }
        let WorkingLookupSchema::PlanningIds(members) = &self.schema else {
            return Ok(None);
        };
        let object_type = type_name::<T>();
        match members.as_slice() {
            [] if self.strategy == WorkingLookupStrategy::IdOrNone => Ok(None),
            [] => Err(WorkingLookupError::MissingIdMetadata { object_type }),
            [member] => (member.read)(object)
                .map(Some)
                .ok_or(WorkingLookupError::NullId {
                    object_type,
                    member: member.name,
                }),
            _ => Err(WorkingLookupError::MultipleIdMembers {
                object_type,
                members: members.iter().map(|m| m.name).collect(),
            }),
        }
    }
    pub fn add(&mut self, object: Arc<T>) -> Result<(), WorkingLookupError<K>> {
        let Some(key) = self.key(&object)? else {
            return Ok(());
        };
        if self.objects.contains_key(&key) {
            return Err(WorkingLookupError::DuplicateId(key));
        }
        self.objects.insert(key, object);
        Ok(())
    }
    /// Add with the actual object's Debug representation attached on error.
    /// Concrete typed bindings retain their concrete Rust type in the cause,
    /// even when their ID getter explicitly projects into a parent field/view.
    /// Trait-object schemas call the actual supplied trait getter, and lookup
    /// still returns the stored canonical Arc allocation. No reflection or
    /// automatic trait/subclass discovery is performed.
    pub fn add_debug(&mut self, object: Arc<T>) -> Result<(), WorkingLookupObjectError<K>>
    where
        T: fmt::Debug,
    {
        self.add(object.clone())
            .map_err(|cause| WorkingLookupObjectError {
                cause,
                object: format!("{object:?}"),
            })
    }

    pub fn remove(&mut self, object: &Arc<T>) -> Result<(), WorkingLookupError<K>> {
        let Some(key) = self.key(object)? else {
            return Ok(());
        };
        if !self
            .objects
            .get(&key)
            .is_some_and(|stored| Arc::ptr_eq(stored, object))
        {
            return Err(WorkingLookupError::DifferentWorkingObject(key));
        }
        self.objects.remove(&key);
        Ok(())
    }
    pub fn lookup(&self, external: &Arc<T>) -> Result<Arc<T>, WorkingLookupError<K>> {
        if self.strategy == WorkingLookupStrategy::Disabled {
            return Err(WorkingLookupError::Unsupported {
                object_type: type_name::<T>(),
            });
        }
        if matches!(self.schema, WorkingLookupSchema::Immutable) {
            return Ok(external.clone());
        }
        let key = self.key(external)?.ok_or(WorkingLookupError::Unsupported {
            object_type: type_name::<T>(),
        })?;
        self.objects
            .get(&key)
            .cloned()
            .ok_or(WorkingLookupError::UnknownId(key))
    }
    pub fn lookup_optional(
        &self,
        external: Option<&Arc<T>>,
    ) -> Result<Option<Arc<T>>, WorkingLookupError<K>> {
        external.map(|object| self.lookup(object)).transpose()
    }
    pub fn reset(&mut self) {
        self.objects.clear();
    }
    /// Validate a new working set before replacing the old one. Ignored/immutable
    /// entries need no storage; duplicate IDs reject the entire replacement.
    pub fn reset_with(
        &mut self,
        objects: impl IntoIterator<Item = Arc<T>>,
    ) -> Result<(), WorkingLookupError<K>> {
        let mut next = BTreeMap::new();
        for object in objects {
            if let Some(key) = self.key(&object)? {
                if next.contains_key(&key) {
                    return Err(WorkingLookupError::DuplicateId(key));
                }
                next.insert(key, object);
            }
        }
        self.objects = next;
        Ok(())
    }
}
