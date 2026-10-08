// Adapted from pinned Timefold Community list selector/iterator tests and helpers.
// Apache-2.0; source identities and substitutions are in compat/provenance.json.
use timetuck::*;
struct Script<T> {
    items: Vec<T>,
    repeat: bool,
}
impl<T: Clone> SelectionSource<(), T> for Script<T> {
    fn is_never_ending(&self) -> bool {
        self.repeat
    }
    fn size(&mut self, _: &()) -> Option<usize> {
        Some(self.items.len())
    }
    fn open<'a>(&'a mut self, _: &'a ()) -> Box<dyn Iterator<Item = T> + 'a>
    where
        T: 'a,
    {
        if self.repeat {
            Box::new(self.items.iter().cloned().cycle())
        } else {
            Box::new(self.items.iter().cloned())
        }
    }
}
fn script<T>(items: Vec<T>, repeat: bool) -> Script<T> {
    Script { items, repeat }
}
fn at(entity: usize, index: usize) -> Option<ListPosition> {
    Some(ListPosition { entity, index })
}
fn position(p: Option<ListPosition>) -> String {
    p.map_or_else(
        || "null".into(),
        |p| format!("{}[{}]", char::from(b'A' + p.entity as u8), p.index),
    )
}
fn change_code(c: ListValueChange<i32>) -> String {
    format!(
        "{} {{{}->{}}}",
        c.value(),
        position(c.source()),
        position(c.destination())
    )
}
fn problem(optional: bool) -> ListProblem<i32> {
    let p = ListProblem::new(
        if optional {
            vec![1, 2, 3, 4]
        } else {
            vec![1, 2, 3]
        },
        vec![
            ListEntity::new(vec![1, 2]),
            ListEntity::new(vec![]),
            ListEntity::new(vec![3]),
        ],
    )
    .unwrap();
    if optional { p.allowing_unassigned() } else { p }
}
fn repeating_changes(optional: bool) {
    let p = problem(optional);
    let a = p.initial_assignment();
    let (values, destinations, expected) = if optional {
        (
            vec![2, 1, 4, 3, 3, 3],
            vec![at(1, 0), at(0, 2), at(0, 0), at(0, 1), at(0, 2), None],
            vec![
                "2 {A[1]->B[0]}",
                "1 {A[0]->A[2]}",
                "4 {null->A[0]}",
                "3 {C[0]->A[1]}",
                "3 {C[0]->A[2]}",
            ],
        )
    } else {
        (
            vec![2, 1, 3, 3, 3],
            vec![at(1, 0), at(0, 2), at(0, 0), at(0, 1), at(0, 2)],
            vec![
                "2 {A[1]->B[0]}",
                "1 {A[0]->A[2]}",
                "3 {C[0]->A[0]}",
                "3 {C[0]->A[1]}",
                "3 {C[0]->A[2]}",
            ],
        )
    };
    let mut paired = ZippedSelectionSource::new(script(values, true), script(destinations, true));
    paired.event(SelectionLifecycleEvent::SolveStarted, &());
    paired.event(SelectionLifecycleEvent::PhaseStarted, &());
    assert!(paired.is_never_ending());
    let mut iter = paired.open(&()).map(|(v, d)| {
        change_code(
            list_value_changes(&p, &a, &[v], &[d], ListChangePairing::Paired, true)
                .unwrap()
                .next()
                .unwrap(),
        )
    });
    for code in expected {
        assert_eq!(iter.next().as_deref(), Some(code));
    }
    assert!(iter.next().is_some());
}
#[test]
fn random_changes() {
    repeating_changes(false);
}
#[test]
fn random_changes_allow_unassigned() {
    repeating_changes(true);
}
#[test]
fn random_swap_iterator() {
    let p = problem(false);
    let a = p.initial_assignment();
    let mut paired = ZippedSelectionSource::new(
        script(vec![1, 1, 1, 3], false),
        script(vec![1, 2, 3, 1], false),
    );
    let mut iter = paired.open(&()).map(|(l, r)| {
        let m = list_value_swap_moves(&p, &a, &[l], &[r], ListSwapPairing::Paired, true)
            .unwrap()
            .next()
            .unwrap();
        match m {
            ListMove::Composite(parts) if parts.is_empty() => "No change".into(),
            ListMove::Swap { left, right } => format!(
                "{l} {{{}}} <-> {r} {{{}}}",
                position(Some(left)),
                position(Some(right))
            ),
            _ => panic!("unexpected move"),
        }
    });
    for code in [
        "No change",
        "1 {A[0]} <-> 2 {A[1]}",
        "1 {A[0]} <-> 3 {C[0]}",
        "3 {C[0]} <-> 1 {A[0]}",
    ] {
        assert_eq!(iter.next().as_deref(), Some(code));
    }
    assert_eq!(iter.next(), None);
}
#[test]
fn random_change_iterator() {
    let p = problem(false);
    let a = p.initial_assignment();
    let order = ListDestinationSelector::new(&p, &a, &[1, 0, 2], &[2, 3], false)
        .unwrap()
        .all();
    let mut draws = [3, 0, 1].into_iter();
    let mut last_bound = 0;
    let destinations: Vec<_> = order
        .random_iter_with(|bound| {
            last_bound = bound;
            draws.next().unwrap()
        })
        .take(3)
        .collect();
    let mut paired =
        ZippedSelectionSource::new(script(vec![1, 2, 3], false), script(destinations, false));
    let actual: Vec<_> = paired
        .open(&())
        .map(|(v, d)| {
            change_code(
                list_value_changes(&p, &a, &[v], &[d], ListChangePairing::Paired, true)
                    .unwrap()
                    .next()
                    .unwrap(),
            )
        })
        .collect();
    assert_eq!(
        actual,
        ["1 {A[0]->A[2]}", "2 {A[1]->B[0]}", "3 {C[0]->A[0]}"]
    );
    assert_eq!(last_bound, 5);
}
