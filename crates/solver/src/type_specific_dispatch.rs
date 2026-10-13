//! Derived specificity ordering and executable dispatch over typed projections.
use crate::{PlanningType, SourceMemberError, SourceTypeHierarchy};
use std::{any::Any, cmp::Ordering, collections::HashMap, error::Error, fmt};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TypeSpecificityError {
    Duplicate(PlanningType),
    Unknown(PlanningType),
}
impl fmt::Display for TypeSpecificityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "native type specificity: {self:?}")
    }
}
impl Error for TypeSpecificityError {}
/// More derived registered types precede all less-derived candidates, including
/// unrelated roots. Depth is computed within this candidate set using actual
/// transitive projection relationships. Ties use Rust type names and TypeIds.
/// This is an immutable native order, not JVM assignability or portable TypeIds.
pub struct TypeSpecificityOrder {
    depths: HashMap<PlanningType, usize>,
    ordered: Vec<PlanningType>,
}
impl TypeSpecificityOrder {
    pub fn new(
        hierarchy: &SourceTypeHierarchy,
        types: impl IntoIterator<Item = PlanningType>,
    ) -> Result<Self, TypeSpecificityError> {
        let mut depths = HashMap::new();
        for ty in types {
            if depths.insert(ty, 0usize).is_some() {
                return Err(TypeSpecificityError::Duplicate(ty));
            }
        }
        let types: Vec<_> = depths.keys().copied().collect();
        // The projection hierarchy rejects cycles at registration. Iterating
        // at most the candidate count resolves even reversed chain order.
        for _ in 0..types.len() {
            let previous = depths.clone();
            for &child in &types {
                let depth = types
                    .iter()
                    .filter(|&&parent| child != parent && hierarchy.is_assignable(child, parent))
                    .map(|parent| previous[parent] + 1)
                    .max()
                    .unwrap_or(0);
                depths.insert(child, depth);
            }
            if depths == previous {
                break;
            }
        }
        let mut ordered = types;
        ordered.sort_by(|left, right| {
            depths[right]
                .cmp(&depths[left])
                .then_with(|| left.name().cmp(right.name()))
                .then_with(|| left.id().cmp(&right.id()))
        });
        Ok(Self { depths, ordered })
    }
    pub fn compare(
        &self,
        left: PlanningType,
        right: PlanningType,
    ) -> Result<Ordering, TypeSpecificityError> {
        let a = self
            .depths
            .get(&left)
            .ok_or(TypeSpecificityError::Unknown(left))?;
        let b = self
            .depths
            .get(&right)
            .ok_or(TypeSpecificityError::Unknown(right))?;
        Ok(b.cmp(a)
            .then_with(|| left.name().cmp(right.name()))
            .then_with(|| left.id().cmp(&right.id())))
    }
    pub fn ordered_types(&self) -> &[PlanningType] {
        &self.ordered
    }
}
type Handler<R> = dyn Fn(&dyn Any) -> Option<R>;
/// A factory owns real typed parent projections and executable read callbacks.
/// Compilation derives handler order; runtime dispatch borrows the current
/// concrete object and invokes the first most-specific applicable handler.
pub struct TypeSpecificDispatchBuilder<R> {
    hierarchy: SourceTypeHierarchy,
    handlers: HashMap<PlanningType, Box<Handler<R>>>,
}
impl<R> Default for TypeSpecificDispatchBuilder<R> {
    fn default() -> Self {
        Self {
            hierarchy: SourceTypeHierarchy::default(),
            handlers: HashMap::new(),
        }
    }
}
impl<R: 'static> TypeSpecificDispatchBuilder<R> {
    pub fn project<C: 'static, P: 'static>(
        &mut self,
        project: impl for<'v> Fn(&'v C) -> &'v P + Send + Sync + 'static,
    ) -> Result<&mut Self, SourceMemberError> {
        self.hierarchy.register(project)?;
        Ok(self)
    }
    pub fn register<T: 'static>(
        &mut self,
        read: impl Fn(&T) -> R + 'static,
    ) -> Result<&mut Self, TypeSpecificityError> {
        let ty = PlanningType::of::<T>();
        if self.handlers.contains_key(&ty) {
            return Err(TypeSpecificityError::Duplicate(ty));
        }
        self.handlers.insert(
            ty,
            Box::new(move |value| value.downcast_ref::<T>().map(&read)),
        );
        Ok(self)
    }
    pub fn compile(self) -> Result<TypeSpecificDispatcher<R>, TypeSpecificityError> {
        let order = TypeSpecificityOrder::new(&self.hierarchy, self.handlers.keys().copied())?;
        Ok(TypeSpecificDispatcher {
            hierarchy: self.hierarchy,
            handlers: self.handlers,
            order,
        })
    }
}
pub struct TypeSpecificDispatcher<R> {
    hierarchy: SourceTypeHierarchy,
    handlers: HashMap<PlanningType, Box<Handler<R>>>,
    order: TypeSpecificityOrder,
}
impl<R> TypeSpecificDispatcher<R> {
    pub fn order(&self) -> &TypeSpecificityOrder {
        &self.order
    }
    pub fn dispatch<T: 'static>(&self, value: &T) -> Option<R> {
        for &candidate in self.order.ordered_types() {
            if let Some(projected) =
                self.hierarchy
                    .project_value(PlanningType::of::<T>(), candidate, value)
            {
                return self.handlers[&candidate](projected);
            }
        }
        None
    }
}
