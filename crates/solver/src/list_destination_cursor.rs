//! Stateful list destinations with replay-aware lookahead and bounded child work.
use crate::{
    ListAssignment, ListError, ListPosition, ListProblem, ListSelectionView, SampleRandom,
    SelectionLifecycleEvent, SelectionSource,
};
use std::{error::Error, fmt, num::NonZeroUsize};

/// Epoch identifies the caller's replay selection; value is a registered domain
/// index, or None for already-filtered child streams. A changed selection discards
/// old lookahead, including exhausted state, without rewinding either raw child.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DestinationReplay {
    pub epoch: u64,
    pub value: Option<usize>,
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DestinationAvailability {
    pub unassigned: bool,
    pub entity_start: bool,
    pub anchor: bool,
    pub suffix_slots: usize,
}
/// Inject semantic choices instead of depending on a Java random call trace.
/// skip consumes that many *eligible* entity occurrences before taking one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DestinationDraw {
    Unassigned,
    EntityStart { skip: usize },
    AfterValue { fallback_rank: usize },
    EntitySuffix { skip: usize, rank: usize },
}
#[derive(Debug, Eq, PartialEq)]
pub enum DestinationCursorError {
    Model(ListError),
    InvalidChoice,
    Exhausted,
    Poisoned,
}
impl fmt::Display for DestinationCursorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "list destination cursor: {self:?}")
    }
}
impl Error for DestinationCursorError {}
impl From<ListError> for DestinationCursorError {
    fn from(e: ListError) -> Self {
        Self::Model(e)
    }
}
#[derive(Default)]
enum Upcoming<T> {
    #[default]
    Unknown,
    End,
    Item(T),
}

/// A cursor over actual entity IDs and value anchors on one immutable layout.
/// Entity starts use pinned-prefix boundaries. Anchors must be assigned and
/// movable; selected values exclude their own anchor and filter receiving ranges.
/// Empty anchor branches can fall back to a movable entity suffix. Optional
/// unassignment remains available when no entity can receive the selected value.
///
/// Lookahead is cached across has_next/next_required. Replaying the same selection
/// does not poll exhausted children again; changing it clears lookahead and permits
/// a non-fused raw child to resume. Raw streams are not rewound. Reopen after layout
/// changes. Borrowing prevents stale positions from following a mutable assignment.
/// Per-child reads are bounded, including skips, for each preparation. Budget
/// exhaustion is observable and is not proof that later child items are invalid.
/// A caught callback panic poisons this cursor; create a new one to retry.
/// Choices describe receiver selection, not move doability: validate source pins,
/// dependencies and PRE-to-POST removal indices through native move construction.
///
/// ```
/// use timetuck::*;
/// use std::num::NonZeroUsize;
/// let model = ListProblem::new(vec![1], vec![ListEntity::new(vec![1]), ListEntity::new(vec![])])?;
/// let layout = model.initial_assignment();
/// let mut cursor = ListDestinationCursor::new(&model, &layout,
///     [0, 1].into_iter(), [1].into_iter(),
///     |_| DestinationDraw::EntityStart { skip: 1 }, false,
///     NonZeroUsize::new(8).unwrap())?;
/// cursor.select_replay(DestinationReplay { epoch: 1, value: Some(0) })?;
/// assert_eq!(cursor.next_required()?, Some(ListPosition { entity: 1, index: 0 }));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub struct ListDestinationCursor<'p, V, E, A, R> {
    model: &'p ListProblem<V>,
    layout: &'p ListAssignment<V>,
    entities: E,
    anchors: A,
    draw: R,
    entity: Upcoming<ListPosition>,
    anchor: Upcoming<ListPosition>,
    pending: Upcoming<Option<ListPosition>>,
    selection: Option<DestinationReplay>,
    allowed: Vec<bool>,
    locations: Vec<Option<ListPosition>>,
    any_anchor: bool,
    optional: bool,
    reads: NonZeroUsize,
    discarded: u64,
    limited: bool,
    poisoned: bool,
}
impl<
    'p,
    V: Clone + Eq,
    E: Iterator<Item = usize>,
    A: Iterator<Item = V>,
    R: FnMut(DestinationAvailability) -> DestinationDraw,
> ListDestinationCursor<'p, V, E, A, R>
{
    pub fn new(
        model: &'p ListProblem<V>,
        layout: &'p ListAssignment<V>,
        entities: E,
        anchors: A,
        draw: R,
        optional: bool,
        reads: NonZeroUsize,
    ) -> Result<Self, DestinationCursorError> {
        model.validate(layout)?;
        if optional && !model.allows_unassigned() {
            return Err(ListError::UnassignmentNotAllowed.into());
        }
        let locations = model.value_locations(layout);
        let any_anchor = locations.iter().flatten().any(|p| {
            !model.entities()[p.entity].is_pinned()
                && p.index >= model.entities()[p.entity].pinned_prefix()
        });
        Ok(Self {
            model,
            layout,
            entities,
            anchors,
            draw,
            entity: Upcoming::Unknown,
            anchor: Upcoming::Unknown,
            pending: Upcoming::Unknown,
            selection: None,
            allowed: vec![true; model.entities().len()],
            locations,
            any_anchor,
            optional,
            reads,
            discarded: 0,
            limited: false,
            poisoned: false,
        })
    }
    pub fn select_replay(
        &mut self,
        selection: DestinationReplay,
    ) -> Result<(), DestinationCursorError> {
        if self.poisoned {
            return Err(DestinationCursorError::Poisoned);
        }
        if selection
            .value
            .is_some_and(|i| i >= self.model.values().len())
        {
            return Err(ListError::ValueOutsideDomain.into());
        }
        if self.selection == Some(selection) {
            return Ok(());
        }
        if self.selection.is_some()
            && (!matches!(self.entity, Upcoming::Unknown)
                || matches!(self.pending, Upcoming::Item(Some(_))))
        {
            self.discarded = self.discarded.saturating_add(1);
        }
        for (entity, allowed) in self.allowed.iter_mut().enumerate() {
            *allowed = selection
                .value
                .is_none_or(|id| self.model.accepts_registered(entity, id));
        }
        self.any_anchor = self.locations.iter().enumerate().any(|(id, at)| {
            selection.value != Some(id)
                && at.is_some_and(|p| {
                    self.allowed[p.entity]
                        && !self.model.entities()[p.entity].is_pinned()
                        && p.index >= self.model.entities()[p.entity].pinned_prefix()
                })
        });
        self.selection = Some(selection);
        self.entity = Upcoming::Unknown;
        self.anchor = Upcoming::Unknown;
        self.pending = Upcoming::Unknown;
        self.limited = false;
        Ok(())
    }
    pub fn discarded_replay_lookahead(&self) -> u64 {
        self.discarded
    }
    pub fn reached_read_limit(&self) -> bool {
        self.limited
    }
    fn pull_entity(&mut self, remaining: &mut usize) -> Result<(), DestinationCursorError> {
        if !matches!(self.entity, Upcoming::Unknown) {
            return Ok(());
        }
        if !self
            .allowed
            .iter()
            .enumerate()
            .any(|(i, &yes)| yes && !self.model.entities()[i].is_pinned())
        {
            self.entity = Upcoming::End;
            return Ok(());
        }
        while *remaining > 0 {
            *remaining -= 1;
            let Some(id) = self.entities.next() else {
                self.entity = Upcoming::End;
                return Ok(());
            };
            let e = self
                .model
                .entities()
                .get(id)
                .ok_or(ListError::EntityIndex {
                    entity: id,
                    count: self.model.entities().len(),
                })?;
            if self.allowed[id] && !e.is_pinned() {
                self.entity = Upcoming::Item(ListPosition {
                    entity: id,
                    index: e.pinned_prefix(),
                });
                return Ok(());
            }
        }
        self.limited = true;
        self.entity = Upcoming::End;
        Ok(())
    }
    fn pull_anchor(&mut self, remaining: &mut usize) -> Result<(), DestinationCursorError> {
        if !matches!(self.anchor, Upcoming::Unknown) {
            return Ok(());
        }
        if !self.any_anchor || !self.allowed.iter().any(|&yes| yes) {
            self.anchor = Upcoming::End;
            return Ok(());
        }
        while *remaining > 0 {
            *remaining -= 1;
            let Some(value) = self.anchors.next() else {
                self.anchor = Upcoming::End;
                return Ok(());
            };
            let id = self
                .model
                .value_index(&value)
                .ok_or(ListError::ValueOutsideDomain)?;
            if self.selection.and_then(|s| s.value) == Some(id) {
                continue;
            }
            if let Some(p) = self.locations[id] {
                let e = &self.model.entities()[p.entity];
                if self.allowed[p.entity] && !e.is_pinned() && p.index >= e.pinned_prefix() {
                    self.anchor = Upcoming::Item(ListPosition {
                        entity: p.entity,
                        index: p.index + 1,
                    });
                    return Ok(());
                }
            }
        }
        self.limited = true;
        self.anchor = Upcoming::End;
        Ok(())
    }
    fn take_entity(
        &mut self,
        skip: usize,
        rank: usize,
        remaining: &mut usize,
    ) -> Result<Option<ListPosition>, DestinationCursorError> {
        // Cached first occurrence costs no new child read. Every additional skip
        // shares this preparation's entity budget; an enormous skip cannot spin.
        for _ in 0..skip {
            if !matches!(self.entity, Upcoming::Item(_)) {
                return Ok(None);
            }
            self.entity = Upcoming::Unknown;
            self.pull_entity(remaining)?;
        }
        let Upcoming::Item(p) = self.entity else {
            return Ok(None);
        };
        let index = p.index.saturating_add(rank);
        if index > self.layout.list(p.entity).expect("validated entity").len() {
            return Err(ListError::InsertionIndex(ListPosition {
                entity: p.entity,
                index,
            })
            .into());
        }
        self.entity = Upcoming::Unknown;
        Ok(Some(ListPosition {
            entity: p.entity,
            index,
        }))
    }
    fn prepare_inner(&mut self) -> Result<(), DestinationCursorError> {
        let mut er = self.reads.get();
        let mut ar = self.reads.get();
        self.pull_entity(&mut er)?;
        self.pull_anchor(&mut ar)?;
        let entity = matches!(self.entity, Upcoming::Item(_));
        let anchor = matches!(self.anchor, Upcoming::Item(_));
        if !self.optional && !entity && !anchor {
            self.pending = Upcoming::End;
            return Ok(());
        }
        let suffix_slots = match self.entity {
            Upcoming::Item(p) => self.layout.list(p.entity).unwrap().len() - p.index + 1,
            _ => 0,
        };
        let choice = (self.draw)(DestinationAvailability {
            unassigned: self.optional,
            entity_start: entity,
            anchor,
            suffix_slots,
        });
        let position = match choice {
            DestinationDraw::Unassigned if self.optional => Some(None),
            DestinationDraw::Unassigned => return Err(DestinationCursorError::InvalidChoice),
            DestinationDraw::EntityStart { skip } => self.take_entity(skip, 0, &mut er)?.map(Some),
            DestinationDraw::EntitySuffix { skip, rank } => {
                self.take_entity(skip, rank, &mut er)?.map(Some)
            }
            DestinationDraw::AfterValue { fallback_rank } => match std::mem::take(&mut self.anchor)
            {
                Upcoming::Item(p) => Some(Some(p)),
                end => {
                    self.anchor = end;
                    self.take_entity(0, fallback_rank, &mut er)?.map(Some)
                }
            },
        };
        self.pending = position
            .or_else(|| self.optional.then_some(None))
            .map_or(Upcoming::End, Upcoming::Item);
        Ok(())
    }
    fn prepare(&mut self) -> Result<(), DestinationCursorError> {
        if self.poisoned {
            return Err(DestinationCursorError::Poisoned);
        }
        if !matches!(self.pending, Upcoming::Unknown) {
            return Ok(());
        }
        self.poisoned = true;
        // Initialize the default selection before arming user callbacks.
        if self.selection.is_none() {
            self.poisoned = false;
            self.select_replay(DestinationReplay::default())?;
            self.poisoned = true;
        }
        let result = self.prepare_inner();
        self.poisoned = false;
        result
    }
    pub fn has_next(&mut self) -> Result<bool, DestinationCursorError> {
        self.prepare()?;
        Ok(matches!(self.pending, Upcoming::Item(_)))
    }
    pub fn try_next(&mut self) -> Result<Option<Option<ListPosition>>, DestinationCursorError> {
        self.prepare()?;
        match std::mem::take(&mut self.pending) {
            Upcoming::Item(p) => Ok(Some(p)),
            end => {
                self.pending = end;
                Ok(None)
            }
        }
    }
    pub fn next_required(&mut self) -> Result<Option<ListPosition>, DestinationCursorError> {
        self.try_next()?.ok_or(DestinationCursorError::Exhausted)
    }
}
impl<
    V: Clone + Eq,
    E: Iterator<Item = usize>,
    A: Iterator<Item = V>,
    R: FnMut(DestinationAvailability) -> DestinationDraw,
> Iterator for ListDestinationCursor<'_, V, E, A, R>
{
    type Item = Option<ListPosition>;
    fn next(&mut self) -> Option<Self::Item> {
        self.try_next()
            .expect("valid list destination child streams and choices")
    }
}

/// Contexts can change replay selection through interior state between pulls.
/// Domain indices belong to this context's immutable ListProblem.
pub trait DestinationSelectionView<V>: ListSelectionView<V> {
    fn list_destination_replay(&self) -> DestinationReplay;
}
impl<V> DestinationSelectionView<V> for (std::sync::Arc<ListProblem<V>>, ListAssignment<V>) {
    fn list_destination_replay(&self) -> DestinationReplay {
        DestinationReplay::default()
    }
}
/// A composable destination child for ListValueChangeSource or a move provider.
/// Events forward once to entities then anchors. Each opening borrows one layout,
/// prepares its location index once, and observes replay selection on every pull.
/// Sources may be finite or unbounded; child reads are bounded per preparation.
/// Original mode prefers entity starts then anchors. Random mode samples available
/// categories (unassignment, entity suffix, anchor) equally and suffix slots
/// uniformly. Child occurrence order/distribution remains the child's policy;
/// this is not a claim of uniform sampling over all global insertion positions.
/// After edits, reopen with a new layout. Output remains potentially unbounded
/// when a child is unbounded or optional unassignment is enabled in random mode.
pub struct ListDestinationSelectionSource<V, E, A> {
    entities: E,
    anchors: A,
    random: bool,
    optional: bool,
    seed: u64,
    step: u64,
    reads: NonZeroUsize,
    marker: std::marker::PhantomData<V>,
}
impl<V, E, A> ListDestinationSelectionSource<V, E, A> {
    pub fn new(
        entities: E,
        anchors: A,
        random: bool,
        optional: bool,
        seed: u64,
        reads: NonZeroUsize,
    ) -> Self {
        Self {
            entities,
            anchors,
            random,
            optional,
            seed,
            step: 0,
            reads,
            marker: std::marker::PhantomData,
        }
    }
    pub fn entity_source(&self) -> &E {
        &self.entities
    }
    pub fn anchor_source(&self) -> &A {
        &self.anchors
    }
}
impl<
    V: Clone + Eq,
    C: DestinationSelectionView<V>,
    E: SelectionSource<C, usize>,
    A: SelectionSource<C, V>,
> SelectionSource<C, Option<ListPosition>> for ListDestinationSelectionSource<V, E, A>
{
    fn event(&mut self, event: SelectionLifecycleEvent, context: &C) {
        if event == SelectionLifecycleEvent::SolveStarted {
            self.step = 0;
        }
        if event == SelectionLifecycleEvent::StepStarted {
            self.step = self.step.wrapping_add(1);
        }
        self.entities.event(event, context);
        self.anchors.event(event, context);
    }
    fn size(&mut self, context: &C) -> Option<usize> {
        (!self.optional
            && self.entities.size(context) == Some(0)
            && self.anchors.size(context) == Some(0))
        .then_some(0)
    }
    fn is_never_ending(&self) -> bool {
        self.entities.is_never_ending()
            || self.anchors.is_never_ending()
            || (self.random && self.optional)
    }
    fn open<'a>(&'a mut self, context: &'a C) -> Box<dyn Iterator<Item = Option<ListPosition>> + 'a>
    where
        Option<ListPosition>: 'a,
    {
        let mut random = SampleRandom::new(self.seed.wrapping_add(self.step));
        let shuffled = self.random;
        let draw = move |h: DestinationAvailability| {
            if !shuffled {
                return if h.entity_start {
                    DestinationDraw::EntityStart { skip: 0 }
                } else if h.anchor {
                    DestinationDraw::AfterValue { fallback_rank: 0 }
                } else {
                    DestinationDraw::Unassigned
                };
            }
            let mut kinds = [0; 3];
            let mut n = 0;
            if h.unassigned {
                kinds[n] = 0;
                n += 1;
            }
            if h.entity_start {
                kinds[n] = 1;
                n += 1;
            }
            if h.anchor {
                kinds[n] = 2;
                n += 1;
            }
            let which = if n == 1 {
                0
            } else {
                random.index(NonZeroUsize::new(n).expect("available destination"))
            };
            match kinds[which] {
                0 => DestinationDraw::Unassigned,
                1 => DestinationDraw::EntitySuffix {
                    skip: 0,
                    rank: if h.suffix_slots == 1 {
                        0
                    } else {
                        random.index(NonZeroUsize::new(h.suffix_slots).unwrap())
                    },
                },
                _ => DestinationDraw::AfterValue { fallback_rank: 0 },
            }
        };
        let entities = self.entities.open(context);
        let anchors = self.anchors.open(context);
        let mut cursor = ListDestinationCursor::new(
            context.problem(),
            context.assignment(),
            entities,
            anchors,
            draw,
            self.optional,
            self.reads,
        )
        .expect("valid list destination layout");
        let mut ended_optional = None;
        Box::new(std::iter::from_fn(move || {
            let replay = context.list_destination_replay();
            if !shuffled && ended_optional == Some(replay) {
                return None;
            }
            cursor
                .select_replay(replay)
                .expect("valid destination replay selection");
            let next = cursor.next();
            if !shuffled && next == Some(None) {
                ended_optional = Some(replay);
            }
            next
        }))
    }
}
