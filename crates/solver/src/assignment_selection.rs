//! Assignment-aware native candidate sources and whole-source gating.
use crate::{
    Assignment, FiniteProblem, ListAssignment, ListError, ListProblem, ListSelectionView,
    ModelError, MoveError, SelectionLifecycleEvent, SelectionSource,
};
use std::sync::Arc;
mod value_lookup;
use value_lookup::ValueOwnership;

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
    pub fn problem(&self) -> &'p ListProblem<V> {
        self.problem
    }
    pub fn assignment(&self) -> &'p ListAssignment<V> {
        self.assignment
    }

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
/// Supports validated borrowed contexts and owned (Arc<ListProblem<V>>,
/// ListAssignment<V>) snapshots. Owned size/open calls validate the snapshot;
/// borrowed contexts retain validation at construction.
/// All mode reads only registry order. For large indexed models, filtered size
/// uses one location index, while opening starts with direct ownership searches
/// and promotes after their row/value visits reach twice the registry length
/// plus the row count (a conservative traversal-cost bound). Promotion costs
/// O(registry size) storage;
/// short cursors avoid that allocation. Hash collisions retain model lookup costs.
/// Small and Eq-only models keep direct searches. No index survives an opening,
/// and owned-context validation/capture costs are unchanged.
pub struct ListValueSource(pub ListValueSelection);
/// Optional-list assignment filtering. Required lists bypass these filters so
/// construction can still select values that have not yet been placed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OptionalListValueFilter {
    All,
    Assigned,
    Unassigned,
}
impl ListValueSource {
    pub fn configured<V: Eq + Clone>(
        problem: &ListProblem<V>,
        filter: OptionalListValueFilter,
    ) -> Self {
        Self(if !problem.allows_unassigned() {
            ListValueSelection::All
        } else {
            match filter {
                OptionalListValueFilter::All => ListValueSelection::All,
                OptionalListValueFilter::Assigned => ListValueSelection::Assigned,
                OptionalListValueFilter::Unassigned => ListValueSelection::Unassigned,
            }
        })
    }
}

fn selected_list_position<V: Eq + Clone>(
    problem: &ListProblem<V>,
    mode: ListValueSelection,
    position: Option<crate::ListPosition>,
) -> bool {
    match mode {
        ListValueSelection::All => true,
        ListValueSelection::Assigned => position.is_some(),
        ListValueSelection::Unassigned => position.is_none(),
        ListValueSelection::Movable => position.is_some_and(|p| {
            !problem.entities()[p.entity].is_pinned()
                && p.index >= problem.entities()[p.entity].pinned_prefix()
        }),
    }
}
impl ListValueSource {
    fn size_view<V: Eq + Clone, C: ListSelectionView<V>>(&mut self, context: &C) -> Option<usize> {
        let problem = context.problem();
        if self.0 == ListValueSelection::All {
            return Some(problem.values().len());
        }
        if ValueOwnership::useful(problem) {
            return Some(
                problem
                    .value_locations(context.assignment())
                    .into_iter()
                    .filter(|&position| selected_list_position(problem, self.0, position))
                    .count(),
            );
        }
        Some(
            problem
                .values()
                .iter()
                .filter(|value| {
                    selected_list_position(problem, self.0, context.assignment().location(value))
                })
                .count(),
        )
    }
    fn open_view<'a, V: Eq + Clone + 'a, C: ListSelectionView<V>>(
        &'a mut self,
        context: &'a C,
    ) -> Box<dyn Iterator<Item = V> + 'a> {
        let mode = self.0;
        let problem = context.problem();
        if mode == ListValueSelection::All {
            return Box::new(problem.values().iter().cloned());
        }
        if ValueOwnership::useful(problem) {
            let mut lookup = ValueOwnership::new(problem, context.assignment());
            return Box::new(
                problem
                    .values()
                    .iter()
                    .enumerate()
                    .filter(move |(id, _)| {
                        selected_list_position(
                            problem,
                            mode,
                            lookup.location(problem, context.assignment(), *id),
                        )
                    })
                    .map(|(_, value)| value.clone()),
            );
        }
        Box::new(
            problem
                .values()
                .iter()
                .filter(move |value| {
                    selected_list_position(problem, mode, context.assignment().location(value))
                })
                .cloned(),
        )
    }
}

impl<V: Eq + Clone> SelectionSource<ListSelectionContext<'_, V>, V> for ListValueSource {
    fn size(&mut self, context: &ListSelectionContext<'_, V>) -> Option<usize> {
        self.size_view(context)
    }
    fn open<'a>(
        &'a mut self,
        context: &'a ListSelectionContext<'_, V>,
    ) -> Box<dyn Iterator<Item = V> + 'a>
    where
        V: 'a,
    {
        self.open_view(context)
    }
}
impl<V: Eq + Clone> SelectionSource<(Arc<ListProblem<V>>, ListAssignment<V>), V>
    for ListValueSource
{
    fn size(&mut self, context: &(Arc<ListProblem<V>>, ListAssignment<V>)) -> Option<usize> {
        context
            .problem()
            .validate(context.assignment())
            .expect("valid owned list selection context");
        self.size_view(context)
    }
    fn open<'a>(
        &'a mut self,
        context: &'a (Arc<ListProblem<V>>, ListAssignment<V>),
    ) -> Box<dyn Iterator<Item = V> + 'a>
    where
        V: 'a,
    {
        context
            .problem()
            .validate(context.assignment())
            .expect("valid owned list selection context");
        self.open_view(context)
    }
}

/// A native list-span source for configured selectors and named recording.
/// Stores size bounds and random state; indexes the current validated context on
/// each opening without retaining/cloning list values or materializing all spans.
/// Borrowed contexts and owned model/assignment snapshots share this same index
/// preparation and random-state behavior; owned contexts enable configured providers.
/// Uniform random ranks sample spans with replacement; native draws do not match
/// Java entity-first traces. SourceRequest::random selects that mode only for
/// uncached random pipelines; cache builders receive finite original enumeration.
pub struct SubListSource {
    bounds: crate::SubListSize,
    random: bool,
    seed: u64,
}
impl SubListSource {
    pub fn configured(
        request: crate::SelectionSourceRequest,
        bounds: crate::SubListSize,
        seed: u64,
    ) -> Self {
        Self {
            bounds,
            random: request.random,
            seed,
        }
    }
    pub fn bounds(&self) -> crate::SubListSize {
        self.bounds
    }
}
impl SubListSource {
    fn size_view<V: Eq + Clone, C: ListSelectionView<V>>(&mut self, c: &C) -> Option<usize> {
        crate::SubListSelection::new(c.problem(), c.assignment(), self.bounds)
            .expect("validated list context")
            .len()
            .try_into()
            .ok()
    }
    fn open_view<'a, V: Eq + Clone, C: ListSelectionView<V>>(
        &'a mut self,
        c: &'a C,
    ) -> Box<dyn Iterator<Item = crate::ListRange> + 'a>
    where
        crate::ListRange: 'a,
    {
        let index = crate::SubListSelection::new(c.problem(), c.assignment(), self.bounds)
            .expect("validated list context");
        if self.random {
            let mut random = crate::random::SplitMix64::new(self.seed);
            self.seed = self.seed.wrapping_add(1);
            Box::new(std::iter::from_fn(move || {
                if index.is_empty() {
                    None
                } else {
                    index.at(random.below_wide(index.len()))
                }
            }))
        } else {
            Box::new((0..index.len()).map(move |rank| index.at(rank).unwrap()))
        }
    }
}
impl<V: Eq + Clone> SelectionSource<ListSelectionContext<'_, V>, crate::ListRange>
    for SubListSource
{
    fn is_never_ending(&self) -> bool {
        self.random
    }
    fn size(&mut self, c: &ListSelectionContext<'_, V>) -> Option<usize> {
        self.size_view(c)
    }
    fn open<'a>(
        &'a mut self,
        c: &'a ListSelectionContext<'_, V>,
    ) -> Box<dyn Iterator<Item = crate::ListRange> + 'a>
    where
        crate::ListRange: 'a,
    {
        self.open_view(c)
    }
}
impl<V: Eq + Clone> SelectionSource<(Arc<ListProblem<V>>, ListAssignment<V>), crate::ListRange>
    for SubListSource
{
    fn is_never_ending(&self) -> bool {
        self.random
    }
    fn size(&mut self, c: &(Arc<ListProblem<V>>, ListAssignment<V>)) -> Option<usize> {
        self.size_view(c)
    }
    fn open<'a>(
        &'a mut self,
        c: &'a (Arc<ListProblem<V>>, ListAssignment<V>),
    ) -> Box<dyn Iterator<Item = crate::ListRange> + 'a>
    where
        crate::ListRange: 'a,
    {
        self.open_view(c)
    }
}
