// Adapted from pinned Community ListSwapMoveSelectorTest, Apache-2.0.
// Complete methods/helpers recorded in compat/provenance.json; no Java RNG parity.
use std::num::NonZeroUsize;
use timetuck::*;
struct Scripted(Vec<i32>);
impl<C> SelectionSource<C, i32> for Scripted {
    fn size(&mut self, _: &C) -> Option<usize> {
        None
    }
    // Source test mocks advertise random selection while furnishing finite
    // prefixes long enough to test continuation. No infinite-output guarantee.
    fn is_never_ending(&self) -> bool {
        true
    }
    fn open<'a>(&'a mut self, _: &'a C) -> Box<dyn Iterator<Item = i32> + 'a>
    where
        i32: 'a,
    {
        Box::new(self.0.iter().copied())
    }
}
fn render(a: &ListAssignment<i32>, m: ListMove<i32>, prefix: &str) -> String {
    let at = |p: ListPosition| format!("{}[{}]", char::from(b'A' + p.entity as u8), p.index);
    match m {
        ListMove::Swap { left, right } => format!(
            "{prefix}{} {{{}}} <-> {prefix}{} {{{}}}",
            a.lists()[left.entity][left.index],
            at(left),
            a.lists()[right.entity][right.index],
            at(right)
        ),
        ListMove::Composite(parts) if parts.is_empty() => "No change".into(),
        ListMove::Composite(parts) => {
            let [
                ListMove::Unassign { entity, start, end },
                ListMove::Assign { value, destination },
            ] = parts.as_slice()
            else {
                panic!("unexpected replacement")
            };
            assert_eq!(*end, *start + 1);
            assert_eq!(
                *destination,
                ListPosition {
                    entity: *entity,
                    index: *start
                }
            );
            let pos = at(*destination);
            format!(
                "{prefix}{} {{{pos}->null}}+{prefix}{value} {{null->{pos}}}",
                a.lists()[*entity][*start]
            )
        }
        _ => panic!("unexpected swap"),
    }
}

fn prefix(p: ListProblem<i32>, left: &[i32], right: &[i32], expected: &[&str]) {
    let a = p.initial_assignment();
    let c = ListSelectionContext::new(&p, &a).unwrap();
    let mut source = ListValueSwapSource::new(
        Scripted(left.to_vec()),
        Scripted(right.to_vec()),
        ListSwapSourceLimits {
            reads_per_side: NonZeroUsize::new(8).unwrap(),
            pairs_per_item: NonZeroUsize::new(8).unwrap(),
        },
        true,
    );
    fn repeats<C, T>(s: &impl SelectionSource<C, T>, _: &C) -> bool {
        s.is_never_ending()
    }
    // The helper only asserts this and continuation for nonempty expected codes.
    if !expected.is_empty() {
        assert!(repeats(&source, &c));
    }
    let mut moves = source.open(&c);
    let actual: Vec<_> = moves
        .by_ref()
        .take(expected.len())
        .map(|m| render(&a, m, ""))
        .collect();
    assert_eq!(actual, expected);
    if !expected.is_empty() {
        assert!(moves.next().is_some());
    }
}
#[test]
fn random() {
    let p = ListProblem::new(
        vec![1, 2, 3],
        vec![
            ListEntity::new(vec![1, 2]),
            ListEntity::new(vec![]),
            ListEntity::new(vec![3]),
        ],
    )
    .unwrap();
    prefix(
        p,
        &[2, 3, 2, 3, 2, 3, 1, 1, 1, 1],
        &[1, 2, 3, 1, 2, 3, 1, 2, 3, 1],
        &[
            "2 {A[1]} <-> 1 {A[0]}",
            "3 {C[0]} <-> 2 {A[1]}",
            "2 {A[1]} <-> 3 {C[0]}",
            "3 {C[0]} <-> 1 {A[0]}",
            "No change",
            "No change",
            "No change",
            "1 {A[0]} <-> 2 {A[1]}",
            "1 {A[0]} <-> 3 {C[0]}",
        ],
    );
}
#[test]
fn random_with_pinning() {
    let p = ListProblem::new(
        vec![1, 2, 3],
        vec![
            ListEntity::new(vec![1, 2]).with_pinned_prefix(1),
            ListEntity::new(vec![]).with_pinned(true),
            ListEntity::new(vec![3]),
        ],
    )
    .unwrap();
    prefix(
        p,
        &[1, 2, 3, 1, 2, 3, 1, 2, 3],
        &[1, 3, 2, 1, 3, 2, 1, 3, 2],
        &[
            "2 {A[1]} <-> 3 {C[0]}",
            "3 {C[0]} <-> 2 {A[1]}",
            "2 {A[1]} <-> 3 {C[0]}",
            "3 {C[0]} <-> 2 {A[1]}",
            "2 {A[1]} <-> 3 {C[0]}",
        ],
    );
}
#[test]
fn random_allows_unassigned_values() {
    let p = ListProblem::new(
        vec![1, 2, 3, 4],
        vec![ListEntity::new(vec![2, 1]), ListEntity::new(vec![3])],
    )
    .unwrap()
    .allowing_unassigned();
    prefix(
        p,
        &[2, 3, 4, 2, 3, 4, 2, 3, 4, 1, 1, 1, 1],
        &[1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 1, 4],
        &[
            "2 {A[0]} <-> 1 {A[1]}",
            "3 {B[0]} <-> 2 {A[0]}",
            "3 {B[0]->null}+4 {null->B[0]}",
            "2 {A[0]->null}+4 {null->A[0]}",
            "3 {B[0]} <-> 1 {A[1]}",
            "2 {A[0]->null}+4 {null->A[0]}",
            "2 {A[0]} <-> 3 {B[0]}",
            "3 {B[0]->null}+4 {null->B[0]}",
            "1 {A[1]->null}+4 {null->A[1]}",
        ],
    );
}

#[test]
fn random_with_entity_value_range_and_filtering() {
    let p = ListProblem::from_entity_ranges(vec![
        ListEntity::new(vec![2, 1]).with_value_range(vec![1, 2, 3]),
        ListEntity::new(vec![3]).with_value_range(vec![2, 3]),
    ])
    .unwrap();
    prefix(
        p.clone(),
        &[2, 2],
        &[3, 3, 3, 3],
        &["2 {A[0]} <-> 3 {B[0]}"],
    );
    // Original helper receives no expected codes in this second case: it adds no
    // exhaustion or metadata assertion. Stronger rejection is tested independently.
    prefix(p, &[1], &[3, 3, 3, 3], &[]);
}
