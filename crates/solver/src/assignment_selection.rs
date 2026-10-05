//! Assignment-aware native candidate sources and whole-source gating.
use crate::{
    Assignment, FiniteProblem, ListAssignment, ListError, ListProblem, ModelError, MoveError,
    SelectionLifecycleEvent, SelectionSource,
};

/// Evaluate a context predicate before asking the child for size or opening it.
/// Disabled contexts have exact size zero and never open/poll the child. Lifecycle
/// events always reach the child. Each size/open call evaluates the predicate once;
/// use a stable predicate during a traversal. This does not filter individual rows.
pub struct GatedSelectionSource<S, F> {
    source: S,
    enabled: F,
}
impl<S, F> GatedSelectionSource<S, F> {
    pub fn new(source: S, enabled: F) -> Self {
        Self { source, enabled }
    }
    pub fn source(&self) -> &S {
        &self.source
    }
    pub fn source_mut(&mut self) -> &mut S {
        &mut self.source
    }
}
impl<C, T, S: SelectionSource<C, T>, F: FnMut(&C) -> bool> SelectionSource<C, T>
    for GatedSelectionSource<S, F>
{
    fn is_never_ending(&self) -> bool {
        self.source.is_never_ending()
    }
    fn event(&mut self, event: SelectionLifecycleEvent, context: &C) {
        self.source.event(event, context);
    }
    fn size(&mut self, context: &C) -> Option<usize> {
        if (self.enabled)(context) {
            self.source.size(context)
        } else {
            Some(0)
        }
    }
    fn open<'a>(&'a mut self, context: &'a C) -> Box<dyn Iterator<Item = T> + 'a>
    where
        T: 'a,
    {
        if (self.enabled)(context) {
            Box::new(self.source.open(context).fuse())
        } else {
            Box::new(std::iter::empty())
        }
    }
}

/// Borrow a validated current basic assignment. Recreate after committing moves;
/// the borrow prevents mutation while a source cursor is observing it.
pub struct BasicSelectionContext<'p, V> {
    problem: &'p FiniteProblem<V>,
    assignment: &'p Assignment<V>,
}
impl<'p, V: Eq> BasicSelectionContext<'p, V> {
    pub fn new(
        problem: &'p FiniteProblem<V>,
        assignment: &'p Assignment<V>,
    ) -> Result<Self, ModelError> {
        problem.validate(assignment)?;
        Ok(Self {
            problem,
            assignment,
        })
    }
    pub fn variable(&self, variable: usize) -> Result<BasicValueContext<'p, V>, MoveError> {
        if variable >= self.problem.variables().len() {
            return Err(MoveError {
                variable,
                variable_count: self.problem.variables().len(),
            });
        }
        Ok(BasicValueContext {
            problem: self.problem,
            assignment: self.assignment,
            variable,
        })
    }
}
pub struct BasicValueContext<'p, V> {
    problem: &'p FiniteProblem<V>,
    assignment: &'p Assignment<V>,
    variable: usize,
}
impl<V: Eq> BasicValueContext<'_, V> {
    pub fn current(&self) -> Option<&V> {
        self.assignment.values()[self.variable].as_ref()
    }
    /// Required and unassigned, with pinning respected. Optional None is complete.
    pub fn needs_initialization(&self) -> bool {
        let v = &self.problem.variables()[self.variable];
        self.current().is_none() && !v.allows_unassigned() && !v.is_pinned()
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BasicEntitySelection {
    All,
    Movable,
    Uninitialized,
}
pub struct BasicEntitySource(pub BasicEntitySelection);
impl<V: Eq> SelectionSource<BasicSelectionContext<'_, V>, usize> for BasicEntitySource {
    fn size(&mut self, context: &BasicSelectionContext<'_, V>) -> Option<usize> {
        Some(self.open(context).count())
    }
    fn open<'a>(
        &'a mut self,
        context: &'a BasicSelectionContext<'_, V>,
    ) -> Box<dyn Iterator<Item = usize> + 'a>
    where
        usize: 'a,
    {
        let mode = self.0;
        Box::new(
            context
                .problem
                .variables()
                .iter()
                .enumerate()
                .filter_map(move |(i, v)| {
                    let enabled = match mode {
                        BasicEntitySelection::All => true,
                        BasicEntitySelection::Movable => !v.is_pinned(),
                        BasicEntitySelection::Uninitialized => {
                            !v.is_pinned()
                                && !v.allows_unassigned()
                                && context.assignment.values()[i].is_none()
                        }
                    };
                    enabled.then_some(i)
                }),
        )
    }
}
/// Domain-order values followed by None for an optional variable. This describes
/// structural choices; a gate/provider can suppress initialized, pinned or no-op
/// changes without erasing the domain itself.
#[derive(Clone, Copy, Debug, Default)]
pub struct BasicValueSource;
impl<V: Eq + Clone> SelectionSource<BasicValueContext<'_, V>, Option<V>> for BasicValueSource {
    fn size(&mut self, context: &BasicValueContext<'_, V>) -> Option<usize> {
        let v = &context.problem.variables()[context.variable];
        v.domain()
            .len()
            .checked_add(usize::from(v.allows_unassigned()))
    }
    fn open<'a>(
        &'a mut self,
        context: &'a BasicValueContext<'_, V>,
    ) -> Box<dyn Iterator<Item = Option<V>> + 'a>
    where
        Option<V>: 'a,
    {
        let v = &context.problem.variables()[context.variable];
        Box::new(
            v.domain()
                .iter()
                .cloned()
                .map(Some)
                .chain(v.allows_unassigned().then_some(None)),
        )
    }
}

/// Validated list assignment; owner/index data is derived from this current view.
pub struct ListSelectionContext<'p, V> {
    problem: &'p ListProblem<V>,
    assignment: &'p ListAssignment<V>,
}
impl<'p, V: Eq + Clone> ListSelectionContext<'p, V> {
    pub fn new(
        problem: &'p ListProblem<V>,
        assignment: &'p ListAssignment<V>,
    ) -> Result<Self, ListError> {
        problem.validate(assignment)?;
        Ok(Self {
            problem,
            assignment,
        })
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ListValueSelection {
    All,
    Assigned,
    Unassigned,
    Movable,
}
/// Finite registry-order list values. Movable includes assigned values outside
/// full/prefix pins; unassigned values have their own selection mode. Eligibility
/// for a particular destination must still be checked by the move provider.
pub struct ListValueSource(pub ListValueSelection);
fn selected_list_value<V: Eq + Clone>(
    context: &ListSelectionContext<'_, V>,
    mode: ListValueSelection,
    value: &V,
) -> bool {
    let position = context.assignment.location(value);
    match mode {
        ListValueSelection::All => true,
        ListValueSelection::Assigned => position.is_some(),
        ListValueSelection::Unassigned => position.is_none(),
        ListValueSelection::Movable => position.is_some_and(|p| {
            !context.problem.entities()[p.entity].is_pinned()
                && p.index >= context.problem.entities()[p.entity].pinned_prefix()
        }),
    }
}
impl<V: Eq + Clone> SelectionSource<ListSelectionContext<'_, V>, V> for ListValueSource {
    fn size(&mut self, context: &ListSelectionContext<'_, V>) -> Option<usize> {
        Some(
            context
                .problem
                .values()
                .iter()
                .filter(|v| selected_list_value(context, self.0, v))
                .count(),
        )
    }
    fn open<'a>(
        &'a mut self,
        context: &'a ListSelectionContext<'_, V>,
    ) -> Box<dyn Iterator<Item = V> + 'a>
    where
        V: 'a,
    {
        let mode = self.0;
        Box::new(
            context
                .problem
                .values()
                .iter()
                .filter(move |v| selected_list_value(context, mode, v))
                .cloned(),
        )
    }
}
