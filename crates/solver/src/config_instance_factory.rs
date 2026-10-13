//! Typed, executable construction bindings for configuration-owned objects.
use crate::PlanningType;
use std::{any::Any, error::Error, fmt};

type InputConstructor<T> = dyn Fn(&dyn Any) -> Option<T>;
enum Constructor<T> {
    NoArguments(fn() -> T),
    Argument(PlanningType, Box<InputConstructor<T>>),
    Context(PlanningType, Box<InputConstructor<T>>),
}
/// Native constructors are real function pointers, registered with their input
/// types. A context-dependent binding models an enclosing owner explicitly;
/// this is not JVM discovery of local or inner classes. No reflection occurs.
pub struct ConfigInstanceFactory<T> {
    constructor: Constructor<T>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigInstanceError {
    pub owner: String,
    pub property: String,
    pub output_type: PlanningType,
    pub required_input: Option<PlanningType>,
    pub actual_input: Option<PlanningType>,
    pub requires_context: bool,
}
impl fmt::Display for ConfigInstanceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} property ({}) cannot construct {}: ",
            self.owner,
            self.property,
            self.output_type.name()
        )?;
        match (self.requires_context, self.actual_input) {
            (true, None) => {
                f.write_str("inner class/context-dependent binding requires its enclosing owner")
            }
            (false, None) => f.write_str("no no-arg constructor."),
            (_, Some(actual)) => write!(
                f,
                "input {} does not match the registered constructor input {:?}",
                actual.name(),
                self.required_input
            ),
        }
    }
}
impl Error for ConfigInstanceError {}
impl<T: 'static> ConfigInstanceFactory<T> {
    pub fn no_arguments(constructor: fn() -> T) -> Self {
        Self {
            constructor: Constructor::NoArguments(constructor),
        }
    }
    pub fn with_argument<A: 'static>(constructor: fn(&A) -> T) -> Self {
        Self {
            constructor: Constructor::Argument(
                PlanningType::of::<A>(),
                Box::new(move |input| input.downcast_ref::<A>().map(constructor)),
            ),
        }
    }
    pub fn with_context<C: 'static>(constructor: fn(&C) -> T) -> Self {
        Self {
            constructor: Constructor::Context(
                PlanningType::of::<C>(),
                Box::new(move |input| input.downcast_ref::<C>().map(constructor)),
            ),
        }
    }
    fn error(
        &self,
        owner: impl FnOnce() -> String,
        property: &str,
        actual_input: Option<PlanningType>,
    ) -> Box<ConfigInstanceError> {
        let (required_input, requires_context) = match &self.constructor {
            Constructor::NoArguments(_) => (None, false),
            Constructor::Argument(input, _) => (Some(*input), false),
            Constructor::Context(input, _) => (Some(*input), true),
        };
        Box::new(ConfigInstanceError {
            owner: owner(),
            property: property.into(),
            output_type: PlanningType::of::<T>(),
            required_input,
            actual_input,
            requires_context,
        })
    }
    /// Owner diagnostics are evaluated only on failure. A successful call
    /// constructs a new object through the registered function every time.
    pub fn construct(
        &self,
        owner: impl FnOnce() -> String,
        property: &str,
    ) -> Result<T, Box<ConfigInstanceError>> {
        match self.constructor {
            Constructor::NoArguments(constructor) => Ok(constructor()),
            _ => Err(self.error(owner, property, None)),
        }
    }
    /// Supply the actual constructor argument or enclosing context. Invalid
    /// input types reject before the registered function executes; no casts,
    /// fabricated defaults or silent input dropping occur.
    pub fn construct_with<I: 'static>(
        &self,
        input: &I,
        owner: impl FnOnce() -> String,
        property: &str,
    ) -> Result<T, Box<ConfigInstanceError>> {
        let result = match &self.constructor {
            Constructor::NoArguments(_) => None,
            Constructor::Argument(_, constructor) | Constructor::Context(_, constructor) => {
                constructor(input)
            }
        };
        result.ok_or_else(|| self.error(owner, property, Some(PlanningType::of::<I>())))
    }
}
