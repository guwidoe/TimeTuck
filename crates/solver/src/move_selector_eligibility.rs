//! Borrowed selector queries, independent of executable move validation.
use crate::{Move, MoveRebaseError, rebase_basic_move};
use std::borrow::Cow;

impl<V> Move<V> {
    /// Query primitive children in execution order against the caller's opening
    /// context. A composite is eligible if any child is eligible; an empty one
    /// is ineligible without consulting that context. Stop at the first true
    /// result or error. This does not validate or simulate the composite's final
    /// assignment: use the problem's move checker before execution.
    pub fn try_selector_is_doable<E>(
        &self,
        mut leaf_is_doable: impl FnMut(&Self) -> Result<bool, E>,
    ) -> Result<bool, E> {
        let children = match self {
            Self::Composite(children) if children.is_empty() => return Ok(false),
            Self::Composite(children) => children,
            leaf => return leaf_is_doable(leaf),
        };
        let mut pending: Vec<_> = children.iter().rev().collect();
        while let Some(movement) = pending.pop() {
            match movement {
                Self::Composite(children) => pending.extend(children.iter().rev()),
                leaf => {
                    if leaf_is_doable(leaf)? {
                        return Ok(true);
                    }
                }
            }
        }
        Ok(false)
    }
}

impl<V: Eq + Clone> Move<V> {
    /// Keep the identity of a no-change move when rebasing. Empty compositions
    /// (including nested empty compositions) borrow the original move and never
    /// call either lookup. Other moves use the ordinary identity/collision rules
    /// and own their rebased result. This does not introduce a global singleton.
    pub fn rebase_preserving_no_change<E>(
        &self,
        variable_lookup: impl FnMut(usize) -> Result<usize, E>,
        value_lookup: impl FnMut(&V) -> Result<V, E>,
    ) -> Result<Cow<'_, Self>, MoveRebaseError<E>> {
        let has_leaf = self
            .try_selector_is_doable(|_| Ok::<_, std::convert::Infallible>(true))
            .unwrap_or_else(|never| match never {});
        if has_leaf {
            rebase_basic_move(self, variable_lookup, value_lookup).map(Cow::Owned)
        } else {
            Ok(Cow::Borrowed(self))
        }
    }
}
