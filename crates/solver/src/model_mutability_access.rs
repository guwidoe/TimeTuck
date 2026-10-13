//! Optional live model capabilities validated against composed planning metadata.
use crate::{ComposedPlanningSolution, PlanningType};
use std::{error::Error, fmt, sync::Arc};
/// Caller-declared foreign shape, not Java class discovery or an ABI contract.
/// Record/enum declarations expose reads only; ordinary native read-only inputs
/// use `Native`. Immutable planning values never require write capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReadOnlyModelShape {
    Native,
    Record,
    Enum,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlanningModelRole {
    Solution,
    Entity,
    Value,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ModelMutabilityAccessError {
    WrongRole {
        model: PlanningType,
        role: PlanningModelRole,
    },
    RequiresMutable {
        model: PlanningType,
        shape: ReadOnlyModelShape,
    },
    ReadOnly(PlanningType),
}
impl fmt::Display for ModelMutabilityAccessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RequiresMutable { model, shape } => {
                let kind = match shape {
                    ReadOnlyModelShape::Record => "a record",
                    ReadOnlyModelShape::Enum => "an enum",
                    ReadOnlyModelShape::Native => "read-only",
                };
                write!(
                    f,
                    "{} cannot be {kind} as it needs to be mutable.",
                    model.name()
                )
            }
            _ => write!(f, "planning model access: {self:?}"),
        }
    }
}
impl Error for ModelMutabilityAccessError {}
type ModelRead<'a, T, W> = dyn Fn(&T) -> W + 'a;
type ModelWrite<'a, T, W> = dyn Fn(&mut T, W) + 'a;
/// Executable access to a caller model's working representation. The same
/// capability validated during binding performs every subsequent read/write.
/// Callbacks must access the declared model; their effects/panics are caller-owned.
/// No callback executes during validation. Setter registration is an explicit
/// capability contract; Rust interior mutability is not detected automatically.
pub struct ModelMutabilityAccess<'a, T, W> {
    read: Box<ModelRead<'a, T, W>>,
    write: Option<Box<ModelWrite<'a, T, W>>>,
    shape: ReadOnlyModelShape,
}
impl<'a, T: 'static, W> ModelMutabilityAccess<'a, T, W> {
    pub fn read_only(shape: ReadOnlyModelShape, read: impl Fn(&T) -> W + 'a) -> Self {
        Self {
            read: Box::new(read),
            write: None,
            shape,
        }
    }
    pub fn mutable(read: impl Fn(&T) -> W + 'a, write: impl Fn(&mut T, W) + 'a) -> Self {
        Self {
            read: Box::new(read),
            write: Some(Box::new(write)),
            shape: ReadOnlyModelShape::Native,
        }
    }
    pub fn bind(
        self,
        layout: Arc<ComposedPlanningSolution>,
        role: PlanningModelRole,
    ) -> Result<BoundModelMutabilityAccess<'a, T, W>, ModelMutabilityAccessError> {
        let model = PlanningType::of::<T>();
        let declared = match role {
            PlanningModelRole::Solution => layout.schema().solution_type() == model,
            PlanningModelRole::Entity => layout.schema().entity::<T>().is_ok(),
            PlanningModelRole::Value => layout
                .schema()
                .entities()
                .any(|e| e.variables().any(|v| v.value_type() == model)),
        };
        if !declared {
            return Err(ModelMutabilityAccessError::WrongRole { model, role });
        }
        if role != PlanningModelRole::Value && self.write.is_none() {
            return Err(ModelMutabilityAccessError::RequiresMutable {
                model,
                shape: self.shape,
            });
        }
        Ok(BoundModelMutabilityAccess {
            access: self,
            layout,
            role,
        })
    }
}
/// Retains the exact validated schema and live capabilities. Use `read` to build
/// the native problem/working assignment, execute ordinary director moves, and
/// `write` to publish the completed working representation into the caller model.
pub struct BoundModelMutabilityAccess<'a, T, W> {
    access: ModelMutabilityAccess<'a, T, W>,
    layout: Arc<ComposedPlanningSolution>,
    role: PlanningModelRole,
}
impl<T: 'static, W> BoundModelMutabilityAccess<'_, T, W> {
    pub fn layout(&self) -> &Arc<ComposedPlanningSolution> {
        &self.layout
    }
    pub fn role(&self) -> PlanningModelRole {
        self.role
    }
    pub fn read(&self, model: &T) -> W {
        (self.access.read)(model)
    }
    pub fn write(&self, model: &mut T, working: W) -> Result<(), ModelMutabilityAccessError> {
        let write = self
            .access
            .write
            .as_ref()
            .ok_or(ModelMutabilityAccessError::ReadOnly(PlanningType::of::<T>()))?;
        write(model, working);
        Ok(())
    }
}
