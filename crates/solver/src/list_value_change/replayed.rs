//! Automatic value-dependent destination replay for live list-change sources.
use super::{ListSelectionView, ListValueChange, ValueChangeSelection};
use crate::{
    DestinationAvailability, DestinationDraw, DestinationReplay, ListDestinationCursor,
    SampleRandom, SelectionLifecycleEvent, SelectionSource,
};
use std::num::NonZeroUsize;

/// A fresh choice cursor per opening; callbacks may borrow the policy/context.
/// Events arrive after the value, entity and anchor child events.
pub trait ListChangeDrawFactory<C> {
    fn change_draw_event(&mut self, _event: SelectionLifecycleEvent, _context: &C) {}
    fn change_draws<'a>(
        &'a mut self,
        context: &'a C,
        step: u64,
    ) -> Box<dyn FnMut(DestinationAvailability) -> DestinationDraw + 'a>;
}
impl<C, F, D> ListChangeDrawFactory<C> for F
where
    F: FnMut(&C, u64) -> D,
    D: FnMut(DestinationAvailability) -> DestinationDraw + 'static,
{
    fn change_draws<'a>(
        &'a mut self,
        context: &'a C,
        step: u64,
    ) -> Box<dyn FnMut(DestinationAvailability) -> DestinationDraw + 'a> {
        Box::new(self(context, step))
    }
}
/// Native category sampling, not a Java RNG or selector call-trace adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ListChangeDestinationOrder {
    Original,
    Random { seed: u64 },
}
impl<C> ListChangeDrawFactory<C> for ListChangeDestinationOrder {
    fn change_draws<'a>(
        &'a mut self,
        _: &'a C,
        step: u64,
    ) -> Box<dyn FnMut(DestinationAvailability) -> DestinationDraw + 'a> {
        let original = *self == Self::Original;
        let seed = match *self {
            Self::Original => 0,
            Self::Random { seed } => seed,
        };
        let mut random = SampleRandom::new(seed.wrapping_add(step));
        Box::new(move |h| {
            if original {
                return if h.entity_start {
                    DestinationDraw::EntityStart { skip: 0 }
                } else if h.anchor {
                    DestinationDraw::AfterValue { fallback_rank: 0 }
                } else {
                    DestinationDraw::Unassigned
                };
            }
            let mut choices = [0; 3];
            let mut n = 0;
            for (kind, available) in [(0, h.unassigned), (1, h.entity_start), (2, h.anchor)] {
                if available {
                    choices[n] = kind;
                    n += 1;
                }
            }
            let which = if n == 1 {
                0
            } else {
                random.index(NonZeroUsize::new(n).expect("available destination"))
            };
            match choices[which] {
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
        })
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReplayedListChangeLimits {
    pub values_per_item: NonZeroUsize,
    pub destinations_per_value: NonZeroUsize,
    pub reads_per_child: NonZeroUsize,
}
/// Select a movable value, automatically refresh receiver eligibility, then draw
/// a checked destination. One accepted candidate per value occurrence; repeat a
/// value to request further choices. No context-side replay cell is required.
///
/// First pull validates/indexes the immutable assignment and opens children once,
/// values then entities then anchors then policy. Every movable value occurrence
/// has a new replay epoch (including duplicates), discarding stale lookahead but
/// never rewinding child streams. Source pins are checked before destination pulls.
/// Empty/rejected destination branches advance to another value. Successful output
/// resets work limits. Exhaustion, rejected-work limit or caught callback panic
/// fuses the opening; callbacks must return cooperatively.
///
/// Preserve PRE-removal metadata, normalize actual moves to POST-removal positions,
/// and validate ranges/pins/dependencies through the shared native move checker.
/// include_noops retains only valid unchanged proposals. Reopen after edits.
/// Storage scales with model indexes, not the Cartesian move count. Random policy
/// samples available categories equally, then entity suffix slots uniformly; child
/// occurrence distribution remains the child's policy. A bounded empty result is
/// not proof of infeasibility. Never-ending metadata describes the value child's
/// potential, not guaranteed output. Seed+step-start count replays each opening.
pub struct ReplayedListValueChangeSource<S, E, A, P> {
    values: S,
    entities: E,
    anchors: A,
    policy: P,
    limits: ReplayedListChangeLimits,
    optional: bool,
    include_noops: bool,
    step: u64,
}
impl<S, E, A, P> ReplayedListValueChangeSource<S, E, A, P> {
    pub fn new(
        values: S,
        entities: E,
        anchors: A,
        policy: P,
        limits: ReplayedListChangeLimits,
        optional: bool,
        include_noops: bool,
    ) -> Self {
        Self {
            values,
            entities,
            anchors,
            policy,
            limits,
            optional,
            include_noops,
            step: 0,
        }
    }
    pub fn change_values(&self) -> &S {
        &self.values
    }
    pub fn change_entities(&self) -> &E {
        &self.entities
    }
    pub fn change_anchors(&self) -> &A {
        &self.anchors
    }
    pub fn change_policy(&self) -> &P {
        &self.policy
    }
}
impl<V, C, S, E, A, P> SelectionSource<C, ListValueChange<V>>
    for ReplayedListValueChangeSource<S, E, A, P>
where
    V: Clone + Eq,
    C: ListSelectionView<V>,
    S: SelectionSource<C, V>,
    E: SelectionSource<C, usize>,
    A: SelectionSource<C, V>,
    P: ListChangeDrawFactory<C>,
{
    fn event(&mut self, event: SelectionLifecycleEvent, context: &C) {
        if event == SelectionLifecycleEvent::SolveStarted {
            self.step = 0;
        }
        if event == SelectionLifecycleEvent::StepStarted {
            self.step = self.step.wrapping_add(1);
        }
        self.values.event(event, context);
        self.entities.event(event, context);
        self.anchors.event(event, context);
        self.policy.change_draw_event(event, context);
    }
    fn size(&mut self, context: &C) -> Option<usize> {
        (self.values.size(context) == Some(0)).then_some(0)
    }
    fn is_never_ending(&self) -> bool {
        self.values.is_never_ending()
    }
    fn open<'a>(&'a mut self, context: &'a C) -> Box<dyn Iterator<Item = ListValueChange<V>> + 'a>
    where
        ListValueChange<V>: 'a,
    {
        let limits = self.limits;
        let optional = self.optional;
        let include_noops = self.include_noops;
        let step = self.step;
        let values = &mut self.values;
        let entities = &mut self.entities;
        let anchors = &mut self.anchors;
        let policy = &mut self.policy;
        Box::new(
            std::iter::once_with(move || {
                let selection = ValueChangeSelection::new(context.problem(), context.assignment())
                    .expect("valid replayed list-change assignment");
                let mut values = values.open(context);
                let mut destinations = ListDestinationCursor::new(
                    context.problem(),
                    context.assignment(),
                    entities.open(context),
                    anchors.open(context),
                    policy.change_draws(context, step),
                    optional,
                    limits.reads_per_child,
                )
                .expect("valid replayed list-change destinations");
                let mut epoch = 0u64;
                let mut ended = false;
                std::iter::from_fn(move || {
                    if ended {
                        return None;
                    }
                    ended = true;
                    for _ in 0..limits.values_per_item.get() {
                        let value = values.next()?;
                        let id = selection
                            .problem
                            .value_index(&value)
                            .expect("registered selected value");
                        if selection.locations[id].is_some_and(|p| !selection.movable(p)) {
                            continue;
                        }
                        epoch = epoch.wrapping_add(1);
                        destinations
                            .select_replay(DestinationReplay {
                                epoch,
                                value: Some(id),
                            })
                            .expect("registered replay value");
                        for _ in 0..limits.destinations_per_value.get() {
                            let Some(destination) = destinations.next() else {
                                break;
                            };
                            if let Some(candidate) =
                                selection.candidate(id, destination, include_noops)
                            {
                                ended = false;
                                return Some(candidate);
                            }
                        }
                    }
                    None
                })
                .fuse()
            })
            .flatten()
            .fuse(),
        )
    }
}
