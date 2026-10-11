//! Build configured expressions without touching runtime predicate evaluation.
use crate::TerminationExpression;
use std::borrow::Cow;

/// Default OR composition for a list of configured conditions.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TerminationComposition {
    #[default]
    Any,
    All,
}
impl<'a, C: Clone> TerminationExpression<'a, C> {
    /// An empty configuration supplies no condition; a singleton borrows the
    /// original expression. Multiple children borrow their ordered input slice.
    /// No expression tree or child payload is allocated, cloned or flattened.
    /// This differs intentionally from explicit All(&[]), whose identity is true.
    pub fn compose(children: &'a [Self], style: TerminationComposition) -> Option<Cow<'a, Self>> {
        match children {
            [] => None,
            [only] => Some(Cow::Borrowed(only)),
            _ => Some(Cow::Owned(match style {
                TerminationComposition::Any => Self::Any(children),
                TerminationComposition::All => Self::All(children),
            })),
        }
    }
}
