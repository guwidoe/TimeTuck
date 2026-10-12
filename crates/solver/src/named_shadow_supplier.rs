//! Typed named supplier resolution, without reflective method discovery.
use crate::{PlanningSchema, PlanningType, PlanningVariableRef, SolutionSourceSupplier};
use std::{any::Any, collections::HashMap, sync::Arc};

type SupplierFn<E, P, V> = dyn Fn(&E, &P) -> Option<V> + Send + Sync;
struct Callable<E, P, V> {
    call: Box<SupplierFn<E, P, V>>,
}
struct Entry {
    value: PlanningType,
    callable: Arc<dyn Any + Send + Sync>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NamedSupplierError {
    SolutionType {
        expected: PlanningType,
        actual: PlanningType,
    },
    InvalidOwner(PlanningType),
    EmptyName,
    Duplicate {
        owner: PlanningType,
        name: String,
    },
    InvalidTarget {
        owner: PlanningType,
        target: String,
    },
    Missing {
        owner: PlanningType,
        target: String,
        name: String,
        solution: PlanningType,
    },
    ReturnType {
        target: String,
        expected: PlanningType,
        actual: PlanningType,
    },
}
impl std::fmt::Display for NamedSupplierError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Self::Missing {
            owner,
            target,
            name,
            solution,
        } = self
        {
            write!(
                f,
                "shadow variable ({target}) has supplierName ({name}) that does not exist inside its declaring class ({}). Maybe you included a parameter which is not a planning solution ({})? Maybe you misspelled the supplierName name?",
                owner.name(),
                solution.name()
            )
        } else {
            write!(f, "named supplier: {self:?}")
        }
    }
}
impl std::error::Error for NamedSupplierError {}
/// Registration and resolution invoke no callbacks. Resolved suppliers retain
/// actual typed callables independently of the registry and reuse the existing
/// solution-aware shadow-session supplier API. Owner/name keys are exact native
/// types; inherited methods require explicit registration on the concrete owner.
pub struct NamedShadowSuppliers<'s, P> {
    schema: &'s PlanningSchema,
    entries: HashMap<(PlanningType, String), Entry>,
    solution: std::marker::PhantomData<fn(&P)>,
}
impl<'s, P: 'static> NamedShadowSuppliers<'s, P> {
    pub fn new(schema: &'s PlanningSchema) -> Result<Self, NamedSupplierError> {
        let expected = schema.solution_type();
        let actual = PlanningType::of::<P>();
        if expected != actual {
            return Err(NamedSupplierError::SolutionType { expected, actual });
        }
        Ok(Self {
            schema,
            entries: HashMap::new(),
            solution: std::marker::PhantomData,
        })
    }
    pub fn register<E: 'static, V: 'static>(
        &mut self,
        name: &str,
        call: impl Fn(&E, &P) -> Option<V> + Send + Sync + 'static,
    ) -> Result<(), NamedSupplierError> {
        let owner = PlanningType::of::<E>();
        if self.schema.entity::<E>().is_err() {
            return Err(NamedSupplierError::InvalidOwner(owner));
        }
        if name.is_empty() {
            return Err(NamedSupplierError::EmptyName);
        }
        let key = (owner, name.into());
        if self.entries.contains_key(&key) {
            return Err(NamedSupplierError::Duplicate {
                owner,
                name: name.into(),
            });
        }
        self.entries.insert(
            key,
            Entry {
                value: PlanningType::of::<V>(),
                callable: Arc::new(Callable {
                    call: Box::new(call),
                }),
            },
        );
        Ok(())
    }
    fn resolve_entry(
        &self,
        owner: PlanningType,
        target: &str,
        name: &str,
    ) -> Result<(PlanningVariableRef<'s>, &Entry), NamedSupplierError> {
        let variable = self
            .schema
            .entity_type(owner)
            .and_then(|e| e.shadow_variable(target))
            .map_err(|_| NamedSupplierError::InvalidTarget {
                owner,
                target: target.into(),
            })?;
        let entry =
            self.entries
                .get(&(owner, name.into()))
                .ok_or_else(|| NamedSupplierError::Missing {
                    owner,
                    target: target.into(),
                    name: name.into(),
                    solution: self.schema.solution_type(),
                })?;
        if variable.value_type() != entry.value {
            return Err(NamedSupplierError::ReturnType {
                target: target.into(),
                expected: variable.value_type(),
                actual: entry.value,
            });
        }
        Ok((variable, entry))
    }
    /// Validate every declared link before constructing a session or reading data.
    pub fn validate(&self, links: &[(PlanningType, &str, &str)]) -> Result<(), NamedSupplierError> {
        for &(owner, target, name) in links {
            self.resolve_entry(owner, target, name)?;
        }
        Ok(())
    }
    pub fn resolve<E: 'static, V: 'static>(
        &self,
        target: &str,
        name: &str,
    ) -> Result<SolutionSourceSupplier<'s, E, P, V>, NamedSupplierError> {
        let (target, entry) = self.resolve_entry(PlanningType::of::<E>(), target, name)?;
        let actual = PlanningType::of::<V>();
        if actual != entry.value {
            return Err(NamedSupplierError::ReturnType {
                target: target.name().into(),
                expected: entry.value,
                actual,
            });
        }
        let callable = entry
            .callable
            .clone()
            .downcast::<Callable<E, P, V>>()
            .expect("registered typed supplier");
        Ok(SolutionSourceSupplier {
            target,
            call: Box::new(move |e, p| (callable.call)(e, p)),
        })
    }
}
