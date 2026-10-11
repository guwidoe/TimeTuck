// Adapted from Community ListChangeMoveSelectorTest and invoked test helpers.
// Apache-2.0; complete provenance recorded separately for parent integration.
use std::num::NonZeroUsize;
use timetuck::*;
struct Cycle<T>(Vec<T>);
impl<C, T: Clone> SelectionSource<C, T> for Cycle<T> {
    fn is_never_ending(&self) -> bool {
        true
    }
    fn size(&mut self, _: &C) -> Option<usize> {
        Some(self.0.len())
    }
    fn open<'a>(&'a mut self, _: &'a C) -> Box<dyn Iterator<Item = T> + 'a>
    where
        T: 'a,
    {
        Box::new(self.0.iter().cloned().cycle())
    }
}
fn at(entity: usize, index: usize) -> Option<ListPosition> {
    Some(ListPosition { entity, index })
}
fn render(c: ListValueChange<i32>) -> String {
    let position = |p: Option<ListPosition>| {
        p.map_or_else(
            || "null".to_owned(),
            |p| format!("{}[{}]", char::from(b'A' + p.entity as u8), p.index),
        )
    };
    format!(
        "{} {{{}->{}}}",
        c.value(),
        position(c.source()),
        position(c.destination())
    )
}
#[test]
fn random_with_pinning() {
    // Preserve B's actual [4] fixture; the upstream explanatory B[] comment is stale.
    let p = ListProblem::new(
        vec![1, 2, 3, 4],
        vec![
            ListEntity::new(vec![1, 2]).with_pinned_prefix(1),
            ListEntity::new(vec![4]).with_pinned(true),
            ListEntity::new(vec![3]),
        ],
    )
    .unwrap();
    let a = p.initial_assignment();
    let c = ListSelectionContext::new(&p, &a).unwrap();
    let mut s = ListValueChangeSource::new(
        Cycle(vec![2, 1, 4, 3, 3]),
        Cycle(vec![at(2, 0), at(0, 2), at(0, 1), at(2, 0)]),
        ListChangeSourceLimits {
            reads_per_side: NonZeroUsize::new(8).unwrap(),
            pairs_per_item: NonZeroUsize::new(8).unwrap(),
        },
        true,
    );
    s.event(SelectionLifecycleEvent::SolveStarted, &c);
    s.event(SelectionLifecycleEvent::PhaseStarted, &c);
    fn repeats<C, T>(s: &impl SelectionSource<C, T>, _: &C) -> bool {
        s.is_never_ending()
    }
    assert!(repeats(&s, &c));
    let mut selected = s.open(&c);
    assert_eq!(
        selected.by_ref().take(4).map(render).collect::<Vec<_>>(),
        [
            "2 {A[1]->C[0]}",
            "3 {C[0]->A[2]}",
            "3 {C[0]->A[1]}",
            "2 {A[1]->C[0]}"
        ]
    );
    // PlannerAssert's never-ending helper also requires continuation, not size.
    assert!(selected.next().is_some());
}
