//! Executable typed Rust function bindings, with schema validation and reuse.
//! Rust compilation supplies callable code; this factory does not generate JVM
//! bytecode or promise reflection, class-loading or measured access speed.
use crate::{MemberAccessError, PlanningType};
use std::{
    any::{Any, TypeId},
    collections::HashMap,
    rc::Rc,
};
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum CompiledMemberAnnotation {
    PlanningVariable { range_ids: Vec<String> },
    PlanningId,
    PlanningPin,
    ValueRangeProvider { id: String },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CompiledMemberKind {
    Field,
    Method,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CompiledMemberRole {
    Read,
    ReadWithParameter,
    Unit,
}
/// Signatures derive from actual compiled function types, including two-argument
/// methods accepted as declarations but rejected by the single-parameter policy.
pub enum CompiledMemberBody<E, V, P> {
    Read(fn(&E) -> V),
    ReadOne(fn(&E, &P) -> V),
    ReadTwo(fn(&E, &P, &P) -> V),
    Unit(fn(&mut E) -> V),
}
impl<E, V, P> CompiledMemberBody<E, V, P> {
    fn signature(&self) -> (usize, usize, bool) {
        match self {
            Self::Read(f) => (0, *f as usize, false),
            Self::ReadOne(f) => (1, *f as usize, false),
            Self::ReadTwo(f) => (2, *f as usize, false),
            Self::Unit(f) => (0, *f as usize, true),
        }
    }
}
/// Own declarations plus compiled callables; no caller-supplied parameter counts,
/// return-type strings or backend flags exist. Names/annotations remain explicit.
pub struct CompiledMemberRegistration<E, V, P = ()> {
    name: String,
    kind: CompiledMemberKind,
    annotations: Vec<CompiledMemberAnnotation>,
    body: CompiledMemberBody<E, V, P>,
    setter: Option<fn(&mut E, V)>,
}
impl<E, V, P> CompiledMemberRegistration<E, V, P> {
    pub fn new(
        name: impl Into<String>,
        kind: CompiledMemberKind,
        annotations: Vec<CompiledMemberAnnotation>,
        body: CompiledMemberBody<E, V, P>,
    ) -> Self {
        Self {
            name: name.into(),
            kind,
            annotations,
            body,
            setter: None,
        }
    }
    pub fn with_setter(mut self, setter: fn(&mut E, V)) -> Self {
        self.setter = Some(setter);
        self
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CompiledMemberCodeType {
    owner: TypeId,
    value: TypeId,
    parameter: TypeId,
    name: String,
    kind: CompiledMemberKind,
    role: CompiledMemberRole,
    read_address: usize,
    setter_address: Option<usize>,
    annotations: Vec<CompiledMemberAnnotation>,
}
impl CompiledMemberCodeType {
    pub fn owner_type(&self) -> TypeId {
        self.owner
    }
    pub fn value_type(&self) -> TypeId {
        self.value
    }
    pub fn kind(&self) -> CompiledMemberKind {
        self.kind
    }
    pub fn role(&self) -> CompiledMemberRole {
        self.role
    }
}
pub struct CompiledMemberAccessor<E, V, P = ()> {
    code: Rc<CompiledMemberCodeType>,
    name: String,
    registration: CompiledMemberRegistration<E, V, P>,
}
impl<E: 'static, V: 'static, P: 'static> CompiledMemberAccessor<E, V, P> {
    pub fn code_type(&self) -> &Rc<CompiledMemberCodeType> {
        &self.code
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn owner_type(&self) -> PlanningType {
        PlanningType::of::<E>()
    }
    pub fn value_type(&self) -> PlanningType {
        PlanningType::of::<V>()
    }
    pub fn generic_type(&self) -> Option<PlanningType> {
        (self.code.role != CompiledMemberRole::Unit).then(PlanningType::of::<V>)
    }
    pub fn parameter_type(&self) -> Option<PlanningType> {
        (self.code.role == CompiledMemberRole::ReadWithParameter).then(PlanningType::of::<P>)
    }
    pub fn implementation_note(&self) -> &'static str {
        "Compiled Rust function binding"
    }
    pub fn supports_setter(&self) -> bool {
        self.registration.setter.is_some()
    }
    pub fn annotations(&self) -> &[CompiledMemberAnnotation] {
        &self.registration.annotations
    }
    pub fn read(&self, entity: &E, parameter: Option<&P>) -> Result<V, MemberAccessError> {
        match self.registration.body{
        CompiledMemberBody::Read(f)=>Ok(f(entity)),
        CompiledMemberBody::ReadOne(f)=>Ok(f(entity,parameter.ok_or_else(||MemberAccessError("Impossible state: the method executeGetter(Object) without parameter is not supported.".into()))?)),
        CompiledMemberBody::ReadTwo(_)=>Err(MemberAccessError("invalid unbound signature".into())),
        CompiledMemberBody::Unit(_)=>Err(MemberAccessError("unit member requires mutable invocation".into())),
    }
    }
    pub fn invoke(&self, entity: &mut E) -> Result<V, MemberAccessError> {
        match self.registration.body {
            CompiledMemberBody::Unit(f) => Ok(f(entity)),
            _ => self.read(entity, None),
        }
    }
    pub fn write(&self, entity: &mut E, value: V) -> Result<(), MemberAccessError> {
        let setter = self
            .registration
            .setter
            .ok_or_else(|| MemberAccessError("member has no setter".into()))?;
        setter(entity, value);
        Ok(())
    }
    pub fn getter_function(&self) -> Result<fn(&E) -> V, MemberAccessError> {
        match self.registration.body {
            CompiledMemberBody::Read(f) => Ok(f),
            _ => Err(MemberAccessError(
                "Impossible state: the method getGetterFunction() is not supported.".into(),
            )),
        }
    }
}
/// Heterogeneous factory cache: each key covers typed signature, semantic schema
/// and actual function addresses. Same signature with different callables never
/// silently reuses earlier behavior. Addresses are process-local binding keys,
/// not portable code identifiers or proofs of machine-code equality.
#[derive(Default)]
pub struct CompiledMemberFactory {
    cache: HashMap<CompiledMemberCodeType, Rc<dyn Any>>,
}
impl CompiledMemberFactory {
    pub fn build<E: 'static, V: 'static, P: 'static>(
        &mut self,
        registration: CompiledMemberRegistration<E, V, P>,
        role: CompiledMemberRole,
    ) -> Result<Rc<CompiledMemberAccessor<E, V, P>>, MemberAccessError> {
        let (count, address, unit) = registration.body.signature();
        if registration.name.is_empty() {
            return Err(MemberAccessError(
                "member registration name is empty".into(),
            ));
        }
        if registration.kind == CompiledMemberKind::Field && (count != 0 || unit) {
            return Err(MemberAccessError(
                "field binding requires a zero-argument value getter".into(),
            ));
        }
        if count > 1 {
            return Err(MemberAccessError(format!(
                "The getterMethod ({}) must have only one parameter",
                registration.name
            )));
        }
        if unit && TypeId::of::<V>() != TypeId::of::<()>() {
            return Err(MemberAccessError("unit invocation must return ()".into()));
        }
        if (role == CompiledMemberRole::ReadWithParameter) != (count == 1)
            || (role == CompiledMemberRole::Unit) != unit
        {
            return Err(MemberAccessError(
                "member role disagrees with callable signature".into(),
            ));
        }
        if (count != 0 || unit) && registration.setter.is_some() {
            return Err(MemberAccessError(
                "setter requires a zero-argument value getter".into(),
            ));
        }
        let name = if registration.kind == CompiledMemberKind::Method
            && role == CompiledMemberRole::Read
        {
            let prefix = if registration.name.starts_with("get") {
                Some("get")
            } else if registration.name.starts_with("is")
                && TypeId::of::<V>() == TypeId::of::<bool>()
            {
                Some("is")
            } else {
                None
            };
            if let Some(prefix) = prefix {
                let tail = registration.name.strip_prefix(prefix).unwrap();
                let mut chars = tail.chars();
                let first = chars
                    .next()
                    .ok_or_else(|| MemberAccessError("empty property name".into()))?;
                first.to_lowercase().to_string() + chars.as_str()
            } else {
                registration.name.clone()
            }
        } else {
            registration.name.clone()
        };
        let key = CompiledMemberCodeType {
            owner: TypeId::of::<E>(),
            value: TypeId::of::<V>(),
            parameter: TypeId::of::<P>(),
            name: registration.name.clone(),
            kind: registration.kind,
            role,
            read_address: address,
            setter_address: registration.setter.map(|f| f as usize),
            annotations: registration.annotations.clone(),
        };
        if let Some(found) = self.cache.get(&key) {
            return Rc::clone(found)
                .downcast::<CompiledMemberAccessor<E, V, P>>()
                .map_err(|_| MemberAccessError("factory cache type mismatch".into()));
        }
        let accessor = Rc::new(CompiledMemberAccessor {
            code: Rc::new(key.clone()),
            name,
            registration,
        });
        self.cache.insert(key, accessor.clone());
        Ok(accessor)
    }
    pub fn binding_count(&self) -> usize {
        self.cache.len()
    }
}
