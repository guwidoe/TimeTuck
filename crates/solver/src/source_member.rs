//! Executable field/getter choices and safe borrowed value-type projections.
use crate::PlanningType;
use std::{any::Any, collections::BTreeSet, error::Error, fmt, marker::PhantomData, sync::Arc};
pub(crate) type Read = dyn for<'v> Fn(&'v dyn Any) -> Option<&'v dyn Any> + Send + Sync;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceAccessorKind {
    Field,
    Getter,
}
struct Accessor {
    value: PlanningType,
    read: Arc<Read>,
}
struct Relation {
    child: PlanningType,
    parent: PlanningType,
    project: Arc<Read>,
}
#[derive(Default)]
pub struct SourceTypeHierarchy {
    relations: Vec<Relation>,
}
impl SourceTypeHierarchy {
    /// A genuine borrowed projection replaces JVM assignability. Rust child
    /// values must contain/refer to their base; no pointer casts or clones occur.
    pub fn register<C: 'static, P: 'static>(
        &mut self,
        project: impl for<'v> Fn(&'v C) -> &'v P + Send + Sync + 'static,
    ) -> Result<(), SourceMemberError> {
        let child = PlanningType::of::<C>();
        let parent = PlanningType::of::<P>();
        if child == parent || self.path(parent, child).is_some() {
            return Err(SourceMemberError::TypeCycle);
        }
        if self
            .relations
            .iter()
            .any(|r| r.child == child && r.parent == parent)
        {
            return Err(SourceMemberError::DuplicateProjection);
        }
        self.relations.push(Relation {
            child,
            parent,
            project: Arc::new(move |value| {
                value
                    .downcast_ref::<C>()
                    .map(|value| project(value) as &dyn Any)
            }),
        });
        Ok(())
    }
    fn path(&self, from: PlanningType, to: PlanningType) -> Option<Vec<Arc<Read>>> {
        let mut queue = std::collections::VecDeque::from([(from, vec![])]);
        let mut seen = BTreeSet::new();
        while let Some((node, path)) = queue.pop_front() {
            if node == to {
                return Some(path);
            }
            if !seen.insert(node.id()) {
                continue;
            }
            for edge in self.relations.iter().filter(|r| r.child == node) {
                let mut next = path.clone();
                next.push(edge.project.clone());
                queue.push_back((edge.parent, next));
            }
        }
        None
    }
    pub fn is_assignable(&self, value: PlanningType, expected: PlanningType) -> bool {
        self.path(value, expected).is_some()
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceMemberError {
    Missing {
        root: PlanningType,
        path: String,
        owner: PlanningType,
        member: String,
    },
    TypeCycle,
    DuplicateProjection,
    ValueType {
        actual: PlanningType,
        expected: PlanningType,
    },
}
fn simple(ty: PlanningType) -> &'static str {
    ty.name().rsplit("::").next().unwrap_or(ty.name())
}
impl fmt::Display for SourceMemberError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing {
                root,
                path,
                owner,
                member,
            } => write!(
                f,
                "The source path ({path}) starting from root class ({}) references a member ({member}) on class ({}) that does not exist.",
                simple(*root),
                simple(*owner)
            ),
            Self::TypeCycle => {
                f.write_str("source value projections must form an acyclic strict type hierarchy")
            }
            Self::DuplicateProjection => f.write_str("source value projection already registered"),
            Self::ValueType { actual, expected } => write!(
                f,
                "source accessor returns {}, which cannot be projected to {}",
                actual.name(),
                expected.name()
            ),
        }
    }
}
impl Error for SourceMemberError {}
pub struct SourceAccessorOptions<E> {
    field: Option<Accessor>,
    getter: Option<Accessor>,
    owner: PhantomData<fn(&E)>,
}
impl<E> Default for SourceAccessorOptions<E> {
    fn default() -> Self {
        Self {
            field: None,
            getter: None,
            owner: PhantomData,
        }
    }
}
fn accessor<E: 'static, V: 'static>(
    read: impl for<'v> Fn(&'v E) -> Option<&'v V> + Send + Sync + 'static,
) -> Accessor {
    Accessor {
        value: PlanningType::of::<V>(),
        read: Arc::new(move |owner| {
            read(owner.downcast_ref::<E>()?).map(|value| value as &dyn Any)
        }),
    }
}
impl<E: 'static> SourceAccessorOptions<E> {
    pub fn field<V: 'static>(
        mut self,
        read: impl for<'v> Fn(&'v E) -> Option<&'v V> + Send + Sync + 'static,
    ) -> Self {
        self.field = Some(accessor(read));
        self
    }
    pub fn getter<V: 'static>(
        mut self,
        read: impl for<'v> Fn(&'v E) -> Option<&'v V> + Send + Sync + 'static,
    ) -> Self {
        self.getter = Some(accessor(read));
        self
    }
    pub fn select<R: 'static>(
        self,
        path: &str,
        member: &str,
        hierarchy: &SourceTypeHierarchy,
    ) -> Result<SelectedSourceAccessor<E>, SourceMemberError> {
        let (kind, chosen) = match (self.field, self.getter) {
            (Some(field), Some(getter))
                if field.value != getter.value
                    && hierarchy.is_assignable(field.value, getter.value) =>
            {
                (SourceAccessorKind::Field, field)
            }
            (_, Some(getter)) => (SourceAccessorKind::Getter, getter),
            (Some(field), None) => (SourceAccessorKind::Field, field),
            (None, None) => {
                return Err(SourceMemberError::Missing {
                    root: PlanningType::of::<R>(),
                    path: path.into(),
                    owner: PlanningType::of::<E>(),
                    member: member.into(),
                });
            }
        };
        Ok(SelectedSourceAccessor {
            kind,
            value: chosen.value,
            read: chosen.read,
            owner: PhantomData,
        })
    }
}
pub struct SelectedSourceAccessor<E> {
    pub(crate) kind: SourceAccessorKind,
    pub(crate) value: PlanningType,
    pub(crate) read: Arc<Read>,
    owner: PhantomData<fn(&E)>,
}
impl<E: 'static> SelectedSourceAccessor<E> {
    pub fn kind(&self) -> SourceAccessorKind {
        self.kind
    }
    pub fn value_type(&self) -> PlanningType {
        self.value
    }
    pub fn read<'v>(&self, owner: &'v E) -> Option<&'v dyn Any> {
        (self.read)(owner)
    }
    pub(crate) fn projected(
        self,
        expected: PlanningType,
        hierarchy: &SourceTypeHierarchy,
    ) -> Result<Arc<Read>, SourceMemberError> {
        let path = hierarchy
            .path(self.value, expected)
            .ok_or(SourceMemberError::ValueType {
                actual: self.value,
                expected,
            })?;
        Ok(Arc::new(move |owner| {
            let mut value = (self.read)(owner)?;
            for project in &path {
                value = project(value)?;
            }
            Some(value)
        }))
    }
}

type SolutionSupplierFn<E, P, V> = dyn Fn(&E, &P) -> Option<V> + Send + Sync;
/// A validated, executable solution-aware shadow supplier. The owner and actual
/// planning solution are borrowed for evaluation; the callback supplies the value.
pub struct SolutionSourceSupplier<'s, E, P, V> {
    pub(crate) target: crate::PlanningVariableRef<'s>,
    pub(crate) call: Box<SolutionSupplierFn<E, P, V>>,
}
impl<'s, E, P, V> SolutionSourceSupplier<'s, E, P, V> {
    pub fn target(&self) -> crate::PlanningVariableRef<'s> {
        self.target
    }
    pub fn evaluate(&self, entity: &E, solution: &P) -> Option<V> {
        (self.call)(entity, solution)
    }
}
