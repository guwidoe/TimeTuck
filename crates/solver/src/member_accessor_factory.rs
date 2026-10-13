//! Native executable member selection and typed-key preexisting accessor reuse.
use crate::{
    MemberAccessError, MemberAccessRole, MemberExposure, PlanningType, RegisteredMember,
    RegisteredMemberBody,
};
use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
    sync::Arc,
};
/// Logical payload type follows actual owned values or their Arc handles.
pub trait NativeAccessorValue: 'static {
    fn payload_type() -> PlanningType;
}
impl NativeAccessorValue for String {
    fn payload_type() -> PlanningType {
        PlanningType::of::<String>()
    }
}
impl<T: 'static> NativeAccessorValue for Arc<T> {
    fn payload_type() -> PlanningType {
        PlanningType::of::<T>()
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FactoryAccessorKind {
    Field,
    BeanProperty,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeMemberKey {
    owner: PlanningType,
    value: PlanningType,
    name: String,
    role: MemberAccessRole,
}
impl Hash for NativeMemberKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.owner.hash(state);
        self.value.hash(state);
        self.name.hash(state);
        std::mem::discriminant(&self.role).hash(state);
    }
}
/// Exposures and class labels are application declarations. Field and method
/// callback signatures are executable typed registrations, never reflective data.
pub struct MemberFactoryRegistration<E, V> {
    member_name: String,
    property_name: String,
    kind: FactoryAccessorKind,
    member: RegisteredMember<E, V, ()>,
}
impl<E: 'static, V: NativeAccessorValue> MemberFactoryRegistration<E, V> {
    pub fn field(
        class: impl Into<String>,
        name: impl Into<String>,
        exposure: MemberExposure,
        read: impl Fn(&E) -> V + 'static,
        write: impl Fn(&mut E, V) + 'static,
    ) -> Self {
        let name = name.into();
        let member = RegisteredMember::new(
            class,
            MemberExposure::Public,
            &name,
            exposure,
            std::any::type_name::<V>(),
            RegisteredMemberBody::Field {
                read: Box::new(read),
                write: Some(Box::new(write)),
                getter: None,
                setter: None,
            },
        );
        Self {
            member_name: name.clone(),
            property_name: name,
            kind: FactoryAccessorKind::Field,
            member,
        }
    }
    /// A real public getter/setter route makes a private field a bean binding.
    pub fn field_with_public_accessors(
        class: impl Into<String>,
        name: impl Into<String>,
        read: impl Fn(&E) -> V + 'static,
        write: impl Fn(&mut E, V) + 'static,
        getter: impl Fn(&E) -> V + 'static,
        setter: impl Fn(&mut E, V) + 'static,
    ) -> Self {
        let name = name.into();
        let member = RegisteredMember::new(
            class,
            MemberExposure::Public,
            &name,
            MemberExposure::Internal,
            std::any::type_name::<V>(),
            RegisteredMemberBody::Field {
                read: Box::new(read),
                write: Some(Box::new(write)),
                getter: Some((MemberExposure::Public, Box::new(getter))),
                setter: Some((MemberExposure::Public, Box::new(setter))),
            },
        );
        Self {
            member_name: name.clone(),
            property_name: name,
            kind: FactoryAccessorKind::BeanProperty,
            member,
        }
    }
    pub fn getter(
        class: impl Into<String>,
        name: impl Into<String>,
        read: impl Fn(&E) -> V + 'static,
        write: impl Fn(&mut E, V) + 'static,
    ) -> Self {
        let name = name.into();
        let property_name = match name.strip_prefix("get") {
            Some(tail) => {
                let mut chars = tail.chars();
                match chars.next() {
                    Some(first) => first.to_lowercase().to_string() + chars.as_str(),
                    None => String::new(),
                }
            }
            None => name.clone(),
        };
        let member = RegisteredMember::new(
            class,
            MemberExposure::Public,
            &name,
            MemberExposure::Public,
            std::any::type_name::<V>(),
            RegisteredMemberBody::Read(Box::new(read)),
        )
        .with_setter(MemberExposure::Public, write);
        Self {
            member_name: name,
            property_name,
            kind: FactoryAccessorKind::BeanProperty,
            member,
        }
    }
    pub fn key(&self, role: MemberAccessRole) -> NativeMemberKey {
        NativeMemberKey {
            owner: PlanningType::of::<E>(),
            value: PlanningType::of::<V>(),
            name: self.member_name.clone(),
            role,
        }
    }
}
pub struct FactoryMemberAccessor<E, V> {
    key: NativeMemberKey,
    name: String,
    kind: FactoryAccessorKind,
    member: RegisteredMember<E, V, ()>,
}
impl<E, V: NativeAccessorValue> FactoryMemberAccessor<E, V> {
    pub fn key(&self) -> &NativeMemberKey {
        &self.key
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn kind(&self) -> FactoryAccessorKind {
        self.kind
    }
    pub fn value_type(&self) -> PlanningType {
        V::payload_type()
    }
    pub fn get(&self, entity: &E) -> Result<V, MemberAccessError> {
        self.member.bind(self.key.role, None)?.read(entity, None)
    }
    pub fn set(&self, entity: &mut E, value: V) -> Result<(), MemberAccessError> {
        self.member.bind(self.key.role, None)?.write(entity, value)
    }
}
/// Cache identity uses actual native owner/value types, member name and access
/// role; it does not derive a generated Java classname. A seeded executable Arc
/// wins before rebuilding a registration, preserving allocation and callbacks.
/// Existing registered-member validation/ordinary solver paths are unchanged.
pub struct NativeMemberAccessorFactory<E, V> {
    cache: HashMap<NativeMemberKey, Arc<FactoryMemberAccessor<E, V>>>,
}
impl<E: 'static, V: NativeAccessorValue> Default for NativeMemberAccessorFactory<E, V> {
    fn default() -> Self {
        Self::new()
    }
}
impl<E: 'static, V: NativeAccessorValue> NativeMemberAccessorFactory<E, V> {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }
    pub fn seed(&mut self, accessor: Arc<FactoryMemberAccessor<E, V>>) {
        self.cache.insert(accessor.key.clone(), accessor);
    }
    pub fn build(
        registration: MemberFactoryRegistration<E, V>,
        role: MemberAccessRole,
        annotation: Option<&str>,
    ) -> Result<Arc<FactoryMemberAccessor<E, V>>, MemberAccessError> {
        registration.member.bind(role, annotation)?;
        Ok(Arc::new(FactoryMemberAccessor {
            key: registration.key(role),
            name: registration.property_name,
            kind: registration.kind,
            member: registration.member,
        }))
    }
    pub fn build_and_cache(
        &mut self,
        registration: MemberFactoryRegistration<E, V>,
        role: MemberAccessRole,
        annotation: Option<&str>,
    ) -> Result<Arc<FactoryMemberAccessor<E, V>>, MemberAccessError> {
        let key = registration.key(role);
        if let Some(found) = self.cache.get(&key) {
            return Ok(found.clone());
        }
        let accessor = Self::build(registration, role, annotation)?;
        self.cache.insert(key, accessor.clone());
        Ok(accessor)
    }
}
