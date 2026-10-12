// Complete adapted pinned Community ListSwapMoveSelectorTest range/pin/optional cases.
// Apache-2.0; source mock streams/helpers retained in compat/provenance.json.
use std::{num::NonZeroUsize, sync::Arc};
use timetuck::*;
struct Recorded(Vec<i32>);
impl<C> SelectionSource<C, i32> for Recorded {
    fn size(&mut self, _: &C) -> Option<usize> {
        Some(self.0.len())
    }
    fn is_never_ending(&self) -> bool {
        true
    }
    fn open<'a>(&'a mut self, _: &'a C) -> Box<dyn Iterator<Item = i32> + 'a>
    where
        i32: 'a,
    {
        Box::new(self.0.iter().copied().cycle())
    }
}
fn at(p: ListPosition) -> String {
    format!("{}[{}]", char::from(b'A' + p.entity as u8), p.index)
}
fn code(p: &ListProblem<i32>, a: &ListAssignment<i32>, m: ListMove<i32>) -> String {
    match m {
        ListMove::Swap { left, right } => format!(
            "{} {{{}}} <-> {} {{{}}}",
            a.list(left.entity).unwrap()[left.index],
            at(left),
            a.list(right.entity).unwrap()[right.index],
            at(right)
        ),
        ListMove::Composite(parts) => match parts.as_slice() {
            [
                ListMove::Unassign { entity, start, .. },
                ListMove::Assign { value, destination },
            ] => format!(
                "{} {{{}->null}}+{} {{null->{}}}",
                a.list(*entity).unwrap()[*start],
                at(ListPosition {
                    entity: *entity,
                    index: *start
                }),
                value,
                at(*destination)
            ),
            [] => "No change".into(),
            _ => panic!("unexpected compound swap {parts:?} in {:?}", p.values()),
        },
        _ => panic!("unexpected movement"),
    }
}
fn verify(p: ListProblem<i32>, left: Vec<i32>, expected: &[&str]) {
    let a = p.initial_assignment();
    let p = Arc::new(p);
    let context = (p.clone(), a.clone());
    let mut source = ReplayedListValueSwapSource::new(
        Recorded(left),
        ListSwapPartnerOrder::Lcg48 { seed: 0 },
        ReplayedListSwapLimits {
            left_reads: NonZeroUsize::new(32).unwrap(),
            partner_attempts: NonZeroUsize::new(32).unwrap(),
        },
    );
    source.event(SelectionLifecycleEvent::SolveStarted, &context);
    source.event(SelectionLifecycleEvent::PhaseStarted, &context);
    assert!(<ReplayedListValueSwapSource<Recorded> as SelectionSource<
        (Arc<ListProblem<i32>>, ListAssignment<i32>),
        ListMove<i32>,
    >>::is_never_ending(&source));
    let mut it = source.open(&context).peekable();
    for expected in expected {
        assert!(it.peek().is_some());
        assert_eq!(&code(&p, &a, it.next().unwrap()), expected);
    }
    assert!(it.peek().is_some());
}
#[test]
fn random_with_entity_value_range() {
    verify(
        ListProblem::new(
            vec![1, 2, 3],
            vec![
                ListEntity::new(vec![2, 1]).with_value_range(vec![1, 2, 3]),
                ListEntity::new(vec![3]).with_value_range(vec![2, 3]),
            ],
        )
        .unwrap(),
        vec![2, 1, 3, 1, 3, 2, 1, 3, 1, 3],
        &[
            "2 {A[0]} <-> 3 {B[0]}",
            "1 {A[1]} <-> 2 {A[0]}",
            "3 {B[0]} <-> 2 {A[0]}",
            "1 {A[1]} <-> 2 {A[0]}",
        ],
    );
}
#[test]
fn random_with_pinning_and_entity_value_range() {
    verify(
        ListProblem::new(
            vec![1, 2, 3, 4],
            vec![
                ListEntity::new(vec![2, 1])
                    .with_value_range(vec![1, 2, 3])
                    .with_pinned_prefix(1),
                ListEntity::new(vec![4])
                    .with_value_range(vec![2, 4])
                    .with_pinned(true),
                ListEntity::new(vec![3]).with_value_range(vec![1, 3, 4]),
            ],
        )
        .unwrap(),
        vec![3, 1, 4, 1],
        &["3 {C[0]} <-> 1 {A[1]}", "1 {A[1]} <-> 3 {C[0]}"],
    );
}
#[test]
fn random_allows_unassigned_values_with_entity_value_range() {
    verify(
        ListProblem::new(
            vec![1, 2, 3, 4],
            vec![
                ListEntity::new(vec![2, 1]).with_value_range(vec![1, 2, 3, 4]),
                ListEntity::new(vec![3]).with_value_range(vec![2, 3]),
            ],
        )
        .unwrap()
        .allowing_unassigned(),
        vec![3, 1, 2, 4, 1],
        &[
            "3 {B[0]} <-> 2 {A[0]}",
            "1 {A[1]->null}+4 {null->A[1]}",
            "2 {A[0]->null}+4 {null->A[0]}",
            "1 {A[1]->null}+4 {null->A[1]}",
        ],
    );
}
