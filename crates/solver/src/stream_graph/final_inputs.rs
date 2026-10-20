//! Reusable final-input positions for exact ordinal identities only.
use super::{Event, typed};
use std::any::{Any, TypeId};

#[derive(Default)]
pub(super) struct FinalInputs {
    positions: Vec<(usize, usize)>,
}
impl FinalInputs {
    pub(super) fn prepare<I: 'static, T: 'static>(&mut self, inputs: &[(bool, Event)]) -> bool {
        // Also clear after a prior mapping panic. Positions never own events or
        // payloads, and rebuilding requires no knowledge of the identity universe.
        self.positions.clear();
        if TypeId::of::<I>() != TypeId::of::<usize>() {
            return false;
        }
        self.positions
            .extend(inputs.iter().enumerate().map(|(position, (_, event))| {
                let id = &typed::<I, T>(event).id as &dyn Any;
                (
                    *id.downcast_ref::<usize>().expect("exact usize identity"),
                    position,
                )
            }));
        // Position breaks ties, so swapping the later element into the retained
        // slot gives the final write and ascending ID order, including usize::MAX.
        self.positions.sort_unstable();
        self.positions.dedup_by(|later, earlier| {
            if later.0 == earlier.0 {
                std::mem::swap(later, earlier);
                true
            } else {
                false
            }
        });
        true
    }
    pub(super) fn positions(&self) -> &[(usize, usize)] {
        &self.positions
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TupleChange;
    use std::{cell::Cell, rc::Rc};

    fn event<I: 'static, T: 'static>(id: I, value: Option<T>) -> (bool, Event) {
        (false, Event::new(TupleChange { id, value }))
    }

    #[test]
    fn sparse_ordinals_select_final_positions_without_a_universe() {
        let mut scratch = FinalInputs::default();
        let inputs = vec![
            event(usize::MAX, Some(1)),
            event(0usize, Some(2)),
            event(usize::MAX, None::<i32>),
            event(19usize, Some(3)),
            event(0usize, Some(4)),
        ];
        assert!(scratch.prepare::<usize, i32>(&inputs));
        assert_eq!(scratch.positions(), &[(0, 4), (19, 3), (usize::MAX, 2)]);
        let capacity = scratch.positions.capacity();
        assert!(scratch.prepare::<usize, i32>(&[]));
        assert!(scratch.positions().is_empty());
        assert_eq!(scratch.positions.capacity(), capacity);
        assert!(scratch.prepare::<usize, i32>(&inputs[..1]));
        assert_eq!(scratch.positions(), &[(usize::MAX, 0)]);
    }

    #[test]
    fn generic_ids_are_not_observed_or_cloned_and_clear_old_positions() {
        struct Id;
        let mut scratch = FinalInputs::default();
        assert!(scratch.prepare::<usize, i32>(&[event(42usize, Some(1))]));
        // Wrong event shape is intentionally opaque on the generic path.
        assert!(!scratch.prepare::<Id, i32>(&[event(9usize, Some(1))]));
        assert!(scratch.positions().is_empty());
    }

    #[test]
    fn scratch_retains_no_payload_after_unwind_and_retry() {
        struct Payload(Rc<Cell<usize>>);
        impl Drop for Payload {
            fn drop(&mut self) {
                self.0.set(self.0.get() + 1);
            }
        }
        let drops = Rc::new(Cell::new(0));
        let mut scratch = FinalInputs::default();
        let inputs = vec![
            event(usize::MAX, Some(Payload(drops.clone()))),
            event(7usize, Some(1i32)),
        ];
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            scratch.prepare::<usize, Payload>(&inputs);
        }));
        assert!(result.is_err());
        drop(inputs);
        assert_eq!(drops.get(), 1);
        assert!(scratch.prepare::<usize, Payload>(&[]));
        assert!(scratch.positions().is_empty());
    }
}
