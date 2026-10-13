//! Typed executable member metadata; no bytecode generation or reflection.
use crate::{
    MemberAccessError, MemberAccessRole, MemberExposure, PlanningType, RegisteredMember,
    RegisteredMemberBody,
};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutableMemberKind {
    Field,
    Method,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutableMemberBackend {
    RegisteredRustCallback,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutableMethodIdentity {
    pub owner: PlanningType,
    pub name: String,
    pub parameter: Option<PlanningType>,
    pub result: PlanningType,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutableFieldIdentity {
    pub owner: PlanningType,
    pub name: String,
    pub value: PlanningType,
}
/// Names/visibility are explicit registrations. Owner/value identities come from
/// actual native types, retaining Rust ownership and generic parameters.
pub struct ExecutableMemberDescriptor<E, V> {
    owner: PlanningType,
    value: PlanningType,
    name: String,
    method_name: String,
    kind: ExecutableMemberKind,
    setter: Option<ExecutableMethodIdentity>,
    member: RegisteredMember<E, V, ()>,
}
fn property_name(name: &str) -> String {
    if let Some(tail) = name.strip_prefix("get") {
        let mut chars = tail.chars();
        if let Some(first) = chars.next() {
            return first.to_lowercase().to_string() + chars.as_str();
        }
    }
    name.to_owned()
}
fn valid_name(name: &str) -> Result<(), MemberAccessError> {
    if name.is_empty() {
        Err(MemberAccessError(
            "member registration name is empty".into(),
        ))
    } else {
        Ok(())
    }
}
impl<E: 'static, V: 'static> ExecutableMemberDescriptor<E, V> {
    fn finish(
        name: String,
        kind: ExecutableMemberKind,
        member: RegisteredMember<E, V, ()>,
    ) -> Result<Self, MemberAccessError> {
        member.bind(MemberAccessRole::FieldOrRead, None)?;
        Ok(Self {
            owner: PlanningType::of::<E>(),
            value: PlanningType::of::<V>(),
            name: if kind == ExecutableMemberKind::Method {
                property_name(&name)
            } else {
                name.clone()
            },
            method_name: name,
            kind,
            setter: None,
            member,
        })
    }
    pub fn field(
        name: impl Into<String>,
        read: impl Fn(&E) -> V + 'static,
        write: impl Fn(&mut E, V) + 'static,
    ) -> Result<Self, MemberAccessError> {
        let name = name.into();
        valid_name(&name)?;
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
        Self::finish(name, ExecutableMemberKind::Field, member)
    }
    pub fn private_field_with_accessors(
        name: impl Into<String>,
        direct_read: impl Fn(&E) -> V + 'static,
        direct_write: impl Fn(&mut E, V) + 'static,
        getter: impl Fn(&E) -> V + 'static,
        setter: impl Fn(&mut E, V) + 'static,
    ) -> Result<Self, MemberAccessError> {
        let name = name.into();
        valid_name(&name)?;
        let member = RegisteredMember::new(
            std::any::type_name::<E>(),
            MemberExposure::Public,
            &name,
            MemberExposure::Internal,
            std::any::type_name::<V>(),
            RegisteredMemberBody::Field {
                read: Box::new(direct_read),
                write: Some(Box::new(direct_write)),
                getter: Some((MemberExposure::Public, Box::new(getter))),
                setter: Some((MemberExposure::Public, Box::new(setter))),
            },
        );
        Self::finish(name, ExecutableMemberKind::Field, member)
    }
    pub fn method(
        name: impl Into<String>,
        exposure: MemberExposure,
        read: impl Fn(&E) -> V + 'static,
    ) -> Result<Self, MemberAccessError> {
        let name = name.into();
        valid_name(&name)?;
        if exposure != MemberExposure::Public {
            return Err(MemberAccessError(format!(
                "Member ({name}) of class ({}) is not public.",
                std::any::type_name::<E>()
            )));
        }
        let member = RegisteredMember::new(
            std::any::type_name::<E>(),
            MemberExposure::Public,
            &name,
            exposure,
            std::any::type_name::<V>(),
            RegisteredMemberBody::Read(Box::new(read)),
        );
        Self::finish(name, ExecutableMemberKind::Method, member)
    }
    /// Setter metadata is installed together with its executable typed callback.
    pub fn with_method_setter(
        mut self,
        name: impl Into<String>,
        write: impl Fn(&mut E, V) + 'static,
    ) -> Result<Self, MemberAccessError> {
        if self.kind != ExecutableMemberKind::Method {
            return Err(MemberAccessError(
                "method setter requires a method descriptor".into(),
            ));
        }
        let name = name.into();
        valid_name(&name)?;
        self.setter = Some(ExecutableMethodIdentity {
            owner: self.owner,
            name,
            parameter: Some(self.value),
            result: PlanningType::of::<()>(),
        });
        self.member = self.member.with_setter(MemberExposure::Public, write);
        Ok(self)
    }
    pub fn owner_type(&self) -> PlanningType {
        self.owner
    }
    pub fn value_type(&self) -> PlanningType {
        self.value
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn kind(&self) -> ExecutableMemberKind {
        self.kind
    }
    pub fn setter(&self) -> Option<&ExecutableMethodIdentity> {
        self.setter.as_ref()
    }
    pub fn when_method(&self, consume: impl FnOnce(ExecutableMethodIdentity)) {
        if self.kind == ExecutableMemberKind::Method {
            consume(ExecutableMethodIdentity {
                owner: self.owner,
                name: self.method_name.clone(),
                parameter: None,
                result: self.value,
            });
        }
    }
    pub fn when_field(&self, consume: impl FnOnce(ExecutableFieldIdentity)) {
        if self.kind == ExecutableMemberKind::Field {
            consume(ExecutableFieldIdentity {
                owner: self.owner,
                name: self.name.clone(),
                value: self.value,
            });
        }
    }
    pub fn read(&self, entity: &E) -> Result<V, MemberAccessError> {
        self.member
            .bind(MemberAccessRole::FieldOrRead, None)?
            .read(entity, None)
    }
    pub fn write(&self, entity: &mut E, value: V) -> Result<(), MemberAccessError> {
        self.member
            .bind(MemberAccessRole::FieldOrGetterWithSetter, None)?
            .write(entity, value)
    }
}
/// Typed action registration factory. A mutable callback determines the actual
/// void operation; its unit return and owner identities cannot be supplied as labels.
pub struct ExecutableVoidMember<E> {
    name: String,
    member: RegisteredMember<E, (), ()>,
}
impl<E: 'static> ExecutableVoidMember<E> {
    pub fn build(
        name: impl Into<String>,
        action: impl Fn(&mut E) + 'static,
    ) -> Result<Self, MemberAccessError> {
        let name = name.into();
        valid_name(&name)?;
        let member = RegisteredMember::new(
            std::any::type_name::<E>(),
            MemberExposure::Public,
            &name,
            MemberExposure::Public,
            std::any::type_name::<()>(),
            RegisteredMemberBody::Void(Box::new(action)),
        );
        member.bind(MemberAccessRole::Void, None)?;
        Ok(Self { name, member })
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn kind(&self) -> ExecutableMemberKind {
        ExecutableMemberKind::Method
    }
    pub fn value_type(&self) -> PlanningType {
        PlanningType::of::<()>()
    }
    pub fn generic_type(&self) -> PlanningType {
        self.value_type()
    }
    pub fn backend(&self) -> ExecutableMemberBackend {
        ExecutableMemberBackend::RegisteredRustCallback
    }
    pub fn invoke(&self, entity: &mut E) -> Result<(), MemberAccessError> {
        self.member
            .bind(MemberAccessRole::Void, None)?
            .invoke_void(entity)
    }
}
