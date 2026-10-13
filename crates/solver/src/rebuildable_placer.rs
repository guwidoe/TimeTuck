//! Factory-backed ownership of fresh construction queues and provider sessions.
use std::{error::Error, fmt, rc::Rc};
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlacerCopyError<E> {
    Unavailable,
    Build(E),
}
impl<E: fmt::Display> fmt::Display for PlacerCopyError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable => f.write_str("The entity placer cannot be copied."),
            Self::Build(e) => write!(f, "construction placer factory: {e}"),
        }
    }
}
impl<E: Error + 'static> Error for PlacerCopyError<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Build(e) => Some(e),
            Self::Unavailable => None,
        }
    }
}
type PlacerBuilder<P, C, E> = dyn Fn(&C) -> Result<P, E>;
type PlacerRecipe<P, C, E> = (Rc<C>, Rc<PlacerBuilder<P, C, E>>);
/// An owned construction session plus its immutable rebuilding recipe/policy.
/// `copy` calls the retained factory exactly once with the same configuration,
/// returning a newly built queue/provider rather than cloning live caches, mimic
/// keys, cursors, random state, snapshots or lifecycle scopes. The factory must
/// construct fresh owned session state; shared application facts may be referenced.
/// Native queues/providers remain directly accessible for normal solving APIs.
/// This optional wrapper adds no dispatch to candidate scanning or scoring.
/// Session lifecycle and configuration validation remain the factory/provider's
/// responsibilities. A manual session has no rebuilding recipe and rejects copy.
/// Factories run synchronously; their external side effects are not rolled back
/// after errors/panics. Failed copies leave the original session intact.
pub struct RebuildablePlacer<P, C, E> {
    session: P,
    recipe: Option<PlacerRecipe<P, C, E>>,
}
impl<P, C, E> RebuildablePlacer<P, C, E> {
    pub fn from_session(session: P) -> Self {
        Self {
            session,
            recipe: None,
        }
    }
    pub fn build(
        configuration: C,
        factory: impl Fn(&C) -> Result<P, E> + 'static,
    ) -> Result<Self, PlacerCopyError<E>> {
        let session = factory(&configuration).map_err(PlacerCopyError::Build)?;
        Ok(Self {
            session,
            recipe: Some((Rc::new(configuration), Rc::new(factory))),
        })
    }
    pub fn session(&self) -> &P {
        &self.session
    }
    pub fn session_mut(&mut self) -> &mut P {
        &mut self.session
    }
    pub fn configuration(&self) -> Option<&C> {
        self.recipe.as_ref().map(|(c, _)| c.as_ref())
    }
    pub fn copy(&self) -> Result<Self, PlacerCopyError<E>> {
        let (configuration, factory) = self.recipe.as_ref().ok_or(PlacerCopyError::Unavailable)?;
        let session = factory(configuration).map_err(PlacerCopyError::Build)?;
        Ok(Self {
            session,
            recipe: Some((configuration.clone(), factory.clone())),
        })
    }
}
