//! Executable registered member contracts; no Rust or Java reflection is used.
use std::{error::Error, fmt};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemberExposure {
    Public,
    Internal,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemberAccessRole {
    FieldOrGetter,
    FieldOrGetterWithSetter,
    FieldOrRead,
    FieldOrReadWithOptionalParameter,
    Void,
}
type Read<E, V> = dyn Fn(&E) -> V;
type Write<E, V> = dyn Fn(&mut E, V);
type ReadOne<E, V, P> = dyn Fn(&E, P) -> V;
type ReadTwo<E, V, P> = dyn Fn(&E, P, P) -> V;
/// Variant signatures determine return kind and parameter count. Visibility and
/// diagnostic names are explicit application declarations, not reflection facts.
pub enum RegisteredMemberBody<E, V, P> {
    Field {
        read: Box<Read<E, V>>,
        write: Option<Box<Write<E, V>>>,
        getter: Option<(MemberExposure, Box<Read<E, V>>)>,
        setter: Option<(MemberExposure, Box<Write<E, V>>)>,
    },
    Read(Box<Read<E, V>>),
    ReadOne(Box<ReadOne<E, V, P>>),
    ReadTwo(Box<ReadTwo<E, V, P>>),
    Void(Box<dyn Fn(&mut E)>),
}
pub struct RegisteredMember<E, V, P> {
    class: String,
    class_exposure: MemberExposure,
    name: String,
    exposure: MemberExposure,
    return_name: String,
    body: RegisteredMemberBody<E, V, P>,
    setter: Option<(MemberExposure, Box<Write<E, V>>)>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemberAccessError(pub String);
impl fmt::Display for MemberAccessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl Error for MemberAccessError {}
impl<E, V, P> RegisteredMember<E, V, P> {
    pub fn new(
        class: impl Into<String>,
        class_exposure: MemberExposure,
        name: impl Into<String>,
        exposure: MemberExposure,
        return_name: impl Into<String>,
        body: RegisteredMemberBody<E, V, P>,
    ) -> Self {
        Self {
            class: class.into(),
            class_exposure,
            name: name.into(),
            exposure,
            return_name: return_name.into(),
            body,
            setter: None,
        }
    }
    pub fn with_setter(
        mut self,
        exposure: MemberExposure,
        setter: impl Fn(&mut E, V) + 'static,
    ) -> Self {
        self.setter = Some((exposure, Box::new(setter)));
        self
    }
    pub fn bind(
        &self,
        role: MemberAccessRole,
        annotation: Option<&str>,
    ) -> Result<BoundRegisteredMember<'_, E, V, P>, MemberAccessError> {
        use MemberAccessRole::*;
        use MemberExposure::Public;
        let field = matches!(self.body, RegisteredMemberBody::Field { .. });
        let kind = if field { "field" } else { "method" };
        let prefix = match annotation {
            Some(a) => format!(
                "The @{a} annotated {kind} ({}) in class ({})",
                self.name, self.class
            ),
            None => format!("The {kind} ({}) in class ({})", self.name, self.class),
        };
        let fail = |reason: &str| Err(MemberAccessError(format!("{prefix} {reason}")));
        if self.class_exposure != Public {
            return fail(&format!(
                "its declaring class ({}) is not public",
                self.class
            ));
        }
        if field {
            if role == Void {
                return fail("must be a void method, but is a field instead");
            }
            if let RegisteredMemberBody::Field {
                getter,
                setter,
                write,
                ..
            } = &self.body
            {
                if self.exposure != Public && !getter.as_ref().is_some_and(|(v, _)| *v == Public) {
                    return fail("is not public and does not have a public getter method");
                }
                if role == FieldOrGetterWithSetter
                    && !(self.exposure == Public && write.is_some()
                        || setter.as_ref().is_some_and(|(v, _)| *v == Public))
                {
                    return fail("does not have a setter method and is not public");
                }
            }
        } else if role == Void {
            if !matches!(self.body, RegisteredMemberBody::Void(_)) {
                return fail(&format!(
                    "must be a void method, but it returns ({})",
                    self.return_name
                ));
            }
            if self.exposure != Public {
                return fail("is a void method, but it is not public");
            }
        } else {
            let getter = matches!(role, FieldOrGetter | FieldOrGetterWithSetter);
            if getter && !self.name.starts_with("get") {
                return fail(&format!(
                    "is suppose to be a public getter method, but its name ({}) does not start with \"get\"",
                    self.name
                ));
            }
            if self.exposure != Public {
                return fail(if getter {
                    "is a getter method, but it is not public"
                } else {
                    "is a read method, but it is not public."
                });
            }
            if matches!(self.body, RegisteredMemberBody::Void(_)) {
                return fail(if getter {
                    "has a public getter method that returns void"
                } else {
                    "is a public read method, but it returns void"
                });
            }
            let count = match self.body {
                RegisteredMemberBody::ReadOne(_) => 1,
                RegisteredMemberBody::ReadTwo(_) => 2,
                _ => 0,
            };
            let max = usize::from(role == FieldOrReadWithOptionalParameter);
            if count > max {
                return fail(&format!(
                    "is a public read method, but takes ({count}) parameters instead of {}",
                    if max == 0 { "none" } else { "zero or one" }
                ));
            }
            if role == FieldOrGetterWithSetter
                && !self.setter.as_ref().is_some_and(|(v, _)| *v == Public)
            {
                return fail(
                    "requires both a public getter and a public setter but only have a public getter",
                );
            }
        }
        Ok(BoundRegisteredMember { member: self, role })
    }
}
pub struct BoundRegisteredMember<'a, E, V, P> {
    member: &'a RegisteredMember<E, V, P>,
    role: MemberAccessRole,
}
impl<E, V, P> BoundRegisteredMember<'_, E, V, P> {
    pub fn read(&self, entity: &E, parameter: Option<P>) -> Result<V, MemberAccessError> {
        if self.role == MemberAccessRole::Void {
            return Err(MemberAccessError("void binding cannot read".into()));
        }
        match &self.member.body {
            RegisteredMemberBody::Field { read, getter, .. } => {
                Ok(if self.member.exposure == MemberExposure::Public {
                    read(entity)
                } else {
                    (getter.as_ref().expect("validated getter").1)(entity)
                })
            }
            RegisteredMemberBody::Read(read) => Ok(read(entity)),
            RegisteredMemberBody::ReadOne(read) => {
                parameter.map(|p| read(entity, p)).ok_or_else(|| {
                    MemberAccessError("parameter is required by the registered callback".into())
                })
            }
            _ => Err(MemberAccessError(
                "binding cannot read this signature".into(),
            )),
        }
    }
    pub fn write(&self, entity: &mut E, value: V) -> Result<(), MemberAccessError> {
        if self.role != MemberAccessRole::FieldOrGetterWithSetter {
            return Err(MemberAccessError(
                "binding does not authorize setter execution".into(),
            ));
        }
        let setter = match &self.member.body {
            RegisteredMemberBody::Field { write, setter, .. } => {
                if self.member.exposure == MemberExposure::Public {
                    write.as_ref().or_else(|| {
                        setter
                            .as_ref()
                            .filter(|(exposure, _)| *exposure == MemberExposure::Public)
                            .map(|(_, callback)| callback)
                    })
                } else {
                    setter.as_ref().map(|(_, f)| f)
                }
            }
            _ => self.member.setter.as_ref().map(|(_, f)| f),
        };
        setter.ok_or_else(|| MemberAccessError("setter is missing".into()))?(entity, value);
        Ok(())
    }
    pub fn invoke_void(&self, entity: &mut E) -> Result<(), MemberAccessError> {
        if self.role != MemberAccessRole::Void {
            return Err(MemberAccessError(
                "binding does not authorize void execution".into(),
            ));
        }
        if let RegisteredMemberBody::Void(callback) = &self.member.body {
            callback(entity);
            Ok(())
        } else {
            Err(MemberAccessError("void callback is missing".into()))
        }
    }
}
