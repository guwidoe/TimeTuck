//! Typed executable property and borrowed-parameter accessors with explicit metadata.
use crate::{
    MemberAccessError, MemberAccessRole, MemberExposure, PlanningType, RegisteredMember,
    RegisteredMemberBody,
};
use std::sync::Arc;
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MemberAnnotation {
    PlanningVariable { range_ids: Vec<String> },
    ValueRangeProvider { id: String },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TypedAccessorKind {
    Field,
    BeanProperty,
    ParameterizedMethod,
}
/// Metadata is declared by the application. Type identities derive from actual
/// callback types. No Java annotations/reflection are inspected or synthesized.
pub struct TypedPropertyAccessor<E, V> {
    name: String,
    kind: TypedAccessorKind,
    annotations: Vec<MemberAnnotation>,
    member: RegisteredMember<E, Arc<V>, ()>,
}
impl<E: 'static, V: 'static> TypedPropertyAccessor<E, V> {
    pub fn field(
        name: impl Into<String>,
        annotations: Vec<MemberAnnotation>,
        read: impl Fn(&E) -> Arc<V> + 'static,
        write: impl Fn(&mut E, Arc<V>) + 'static,
    ) -> Self {
        let name = name.into();
        let member = RegisteredMember::new(
            std::any::type_name::<E>(),
            MemberExposure::Public,
            &name,
            MemberExposure::Public,
            std::any::type_name::<V>(),
            RegisteredMemberBody::Field {
                read: Box::new(read),
                write: Some(Box::new(write)),
                getter: None,
                setter: None,
            },
        );
        Self {
            name,
            kind: TypedAccessorKind::Field,
            annotations,
            member,
        }
    }
    pub fn bean(
        name: impl Into<String>,
        getter_name: impl Into<String>,
        annotations: Vec<MemberAnnotation>,
        read: impl Fn(&E) -> Arc<V> + 'static,
        write: impl Fn(&mut E, Arc<V>) + 'static,
    ) -> Result<Self, MemberAccessError> {
        let member = RegisteredMember::new(
            std::any::type_name::<E>(),
            MemberExposure::Public,
            getter_name,
            MemberExposure::Public,
            std::any::type_name::<V>(),
            RegisteredMemberBody::Read(Box::new(read)),
        )
        .with_setter(MemberExposure::Public, write);
        member.bind(MemberAccessRole::FieldOrGetterWithSetter, None)?;
        Ok(Self {
            name: name.into(),
            kind: TypedAccessorKind::BeanProperty,
            annotations,
            member,
        })
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn kind(&self) -> TypedAccessorKind {
        self.kind
    }
    /// Logical payload type of the Arc returned and accepted by callbacks.
    pub fn value_type(&self) -> PlanningType {
        PlanningType::of::<V>()
    }
    pub fn annotations(&self) -> &[MemberAnnotation] {
        &self.annotations
    }
    pub fn get(&self, entity: &E) -> Result<Arc<V>, MemberAccessError> {
        self.member
            .bind(MemberAccessRole::FieldOrGetterWithSetter, None)?
            .read(entity, None)
    }
    pub fn getter_function(&self) -> impl Fn(&E) -> Result<Arc<V>, MemberAccessError> + '_ {
        move |entity| self.get(entity)
    }
    pub fn set(&self, entity: &mut E, value: Arc<V>) -> Result<(), MemberAccessError> {
        self.member
            .bind(MemberAccessRole::FieldOrGetterWithSetter, None)?
            .write(entity, value)
    }
}
type BorrowedRead<E, S, R> = dyn Fn(&E, &S) -> R;
/// Exactly one borrowed typed parameter; source/base projections can pass a
/// reference to an actual composed parent without allocating/cloning that parent.
/// Getter calls always observe current data. Callbacks may have external effects;
/// they run only after the required parameter is present. No automatic overload or
/// inheritance discovery is promised. Getter-function adaptation without the
/// mandatory parameter is intentionally unsupported.
pub struct TypedParameterAccessor<E, S, R> {
    name: String,
    annotations: Vec<MemberAnnotation>,
    read: Box<BorrowedRead<E, S, R>>,
}
impl<E, S: 'static, R: 'static> TypedParameterAccessor<E, S, R> {
    pub fn new(
        name: impl Into<String>,
        annotations: Vec<MemberAnnotation>,
        read: impl Fn(&E, &S) -> R + 'static,
    ) -> Self {
        Self {
            name: name.into(),
            annotations,
            read: Box::new(read),
        }
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn kind(&self) -> TypedAccessorKind {
        TypedAccessorKind::ParameterizedMethod
    }
    pub fn value_type(&self) -> PlanningType {
        PlanningType::of::<R>()
    }
    pub fn parameter_type(&self) -> PlanningType {
        PlanningType::of::<S>()
    }
    pub fn annotations(&self) -> &[MemberAnnotation] {
        &self.annotations
    }
    pub fn get(&self, entity: &E, solution: Option<&S>) -> Result<R, MemberAccessError> {
        let solution=solution.ok_or_else(||MemberAccessError("Impossible state: the method executeGetter(Object) without parameter is not supported.".into()))?;
        Ok((self.read)(entity, solution))
    }
    pub fn getter_function(&self) -> Result<fn(&E) -> R, MemberAccessError> {
        Err(MemberAccessError(
            "Impossible state: the method getGetterFunction() is not supported.".into(),
        ))
    }
}
