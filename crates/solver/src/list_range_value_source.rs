//! Raw distinct values from effective list-entity ranges in registry order.
use crate::{
    Lcg48Random, ListSelectionView, SampleRandom, SelectionLifecycleEvent, SelectionSource,
};
use std::num::{NonZeroU32, NonZeroUsize};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ListRangeValueOrder {
    Original,
    NativeRandom { seed: u64 },
    Lcg48Random { seed: u64 },
}
/// Domain-union selection without multiplying the weight of overlapping ranges.
/// Registry order supplies original traversal and seeded draw ranks. A global
/// entity range uses the registry directly; explicit ranges build one bitmap.
/// Zero entities yield an empty union. Pins/assignment status are intentionally
/// checked by consuming move selectors, after raw draws, not by this source.
///
/// Original traversal is finite. Random traversal samples with replacement,
/// including one-element draws; reopening uses seed+step-start count. No values
/// are cloned until returned, and opening itself visits no domain. Storage is
/// O(registry size), never entities times values. Eq-only registries are supported.
/// Unknown draw indices or caught draw/Clone panics fuse the cursor. Lifecycle
/// advances subsequent opening seeds and does not mutate existing cursors.
pub struct ListRangeValueSource {
    order: ListRangeValueOrder,
    step: u64,
}
impl ListRangeValueSource {
    pub fn new(order: ListRangeValueOrder) -> Self {
        Self { order, step: 0 }
    }
    fn pool<V: Clone + Eq, C: ListSelectionView<V>>(c: &C) -> Vec<usize> {
        let p = c.problem();
        if p.entities().is_empty() {
            return Vec::new();
        }
        if p.entities().iter().any(|e| e.value_range().is_none()) {
            return (0..p.values().len()).collect();
        }
        let mut present = vec![false; p.values().len()];
        for e in p.entities() {
            for v in e.value_range().expect("explicit entity range") {
                present[p.value_index(v).expect("validated entity range")] = true;
            }
        }
        present
            .into_iter()
            .enumerate()
            .filter_map(|(i, yes)| yes.then_some(i))
            .collect()
    }
    /// Inject an owned/borrowed draw stream, including a caller's shared generator.
    /// Each successful pull calls it exactly once with the distinct union size.
    /// Empty unions never draw. This supports cooperating selectors without
    /// inventing implicit global RNG state or promising Timefold call traces.
    pub fn select_with<
        'a,
        V: Clone + Eq + 'a,
        C: ListSelectionView<V>,
        F: FnMut(NonZeroUsize) -> usize + 'a,
    >(
        c: &'a C,
        mut draw: F,
    ) -> Box<dyn Iterator<Item = V> + 'a> {
        Box::new(
            std::iter::once_with(move || {
                let ids = Self::pool(c);
                let mut ended = false;
                std::iter::from_fn(move || {
                    if ended {
                        return None;
                    }
                    ended = true;
                    let n = NonZeroUsize::new(ids.len())?;
                    let rank = draw(n);
                    let id = *ids.get(rank).expect("list range draw is below union size");
                    let value = c.problem().values()[id].clone();
                    ended = false;
                    Some(value)
                })
                .fuse()
            })
            .flatten()
            .fuse(),
        )
    }
}
impl<V: Clone + Eq, C: ListSelectionView<V>> SelectionSource<C, V> for ListRangeValueSource {
    fn event(&mut self, event: SelectionLifecycleEvent, _: &C) {
        if event == SelectionLifecycleEvent::SolveStarted {
            self.step = 0;
        }
        if event == SelectionLifecycleEvent::StepStarted {
            self.step = self.step.wrapping_add(1);
        }
    }
    fn size(&mut self, c: &C) -> Option<usize> {
        Some(Self::pool(c).len())
    }
    fn is_never_ending(&self) -> bool {
        self.order != ListRangeValueOrder::Original
    }
    fn open<'a>(&'a mut self, c: &'a C) -> Box<dyn Iterator<Item = V> + 'a>
    where
        V: 'a,
    {
        match self.order {
            ListRangeValueOrder::Original => Box::new(
                std::iter::once_with(move || {
                    let mut ids = Self::pool(c).into_iter();
                    let mut ended = false;
                    std::iter::from_fn(move || {
                        if ended {
                            return None;
                        }
                        ended = true;
                        let id = ids.next()?;
                        let value = c.problem().values()[id].clone();
                        ended = false;
                        Some(value)
                    })
                    .fuse()
                })
                .flatten()
                .fuse(),
            ),
            ListRangeValueOrder::NativeRandom { seed } => {
                let mut random = SampleRandom::new(seed.wrapping_add(self.step));
                Self::select_with(c, move |n| random.index(n))
            }
            ListRangeValueOrder::Lcg48Random { seed } => {
                let mut random = Lcg48Random::new(seed.wrapping_add(self.step));
                Self::select_with(c, move |n| {
                    random
                        .below(
                            NonZeroU32::new(u32::try_from(n.get()).expect("LCG48 union fits u32"))
                                .unwrap(),
                        )
                        .expect("LCG48 union fits i32") as usize
                })
            }
        }
    }
}
