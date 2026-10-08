// Adapted from Timefold Community RandomSubListChangeMoveSelectorTest and
// RandomSubListSwapMoveSelectorTest, Apache-2.0. See compat/provenance.json.
use timetuck::*;
struct Destination(usize);
impl<C> SelectionSource<C, Option<ListPosition>> for Destination {
    fn is_never_ending(&self) -> bool {
        true
    }
    fn size(&mut self, _: &C) -> Option<usize> {
        Some(self.0)
    }
    fn open<'a>(&'a mut self, _: &'a C) -> Box<dyn Iterator<Item = Option<ListPosition>> + 'a>
    where
        Option<ListPosition>: 'a,
    {
        Box::new(std::iter::repeat(Some(ListPosition {
            entity: 1,
            index: 0,
        })))
    }
}
fn spans(min: usize, max: usize) -> SubListSource {
    SubListSource::configured(
        SelectionSourceRequest {
            random: true,
            cache_scope: SelectionCacheScope::Uncached,
        },
        SubListSize::new(min, max).unwrap(),
        0,
    )
}
fn problem(lists: Vec<Vec<i32>>) -> ListProblem<i32> {
    ListProblem::new(
        lists.iter().flatten().copied().collect(),
        lists.into_iter().map(ListEntity::new).collect(),
    )
    .unwrap()
}
fn repeats<C, T>(s: &impl SelectionSource<C, T>, _: &C) -> bool {
    s.is_never_ending()
}
#[test]
fn change_size_unrestricted() {
    let p = problem(vec![vec![1, 2, 3], vec![], vec![4, 5]]);
    let a = p.initial_assignment();
    let c = ListSelectionContext::new(&p, &a).unwrap();
    let mut s = SubListChangeSource::new(spans(1, usize::MAX), Destination(25), false, 0);
    assert_eq!(s.proposal_count(&c), Some(9 * 25));
    assert!(repeats(&s, &c));
}
#[test]
fn change_size_with_bounds() {
    // Complete native model includes C and D, which the upstream entity-selector
    // mock supplies even though the solution's entity list only contains A/B.
    let p = problem(vec![
        vec![1, 2, 3, 4, 5, 6, 7],
        vec![],
        vec![11, 12, 13],
        vec![21, 22, 23, 24],
    ]);
    let a = p.initial_assignment();
    let c = ListSelectionContext::new(&p, &a).unwrap();
    let mut s = SubListChangeSource::new(spans(3, 5), Destination(7), false, 0);
    assert_eq!(s.proposal_count(&c), Some(16 * 7));
    assert!(repeats(&s, &c));
}
#[test]
fn swap_size_unrestricted() {
    let p = problem(vec![vec![1, 2, 3], vec![], vec![4]]);
    let a = p.initial_assignment();
    let c = ListSelectionContext::new(&p, &a).unwrap();
    let mut s = SubListSwapSource::new(spans(1, usize::MAX), spans(1, usize::MAX), false, 0);
    assert_eq!(s.proposal_count(&c), Some(7 * 7));
    assert!(repeats(&s, &c));
}
#[test]
fn swap_size_with_bounds() {
    let p = problem(vec![
        vec![1, 2, 3, 4, 5, 6, 7],
        vec![],
        vec![11, 12, 13],
        vec![21, 22, 23, 24],
    ]);
    let a = p.initial_assignment();
    let c = ListSelectionContext::new(&p, &a).unwrap();
    let mut s = SubListSwapSource::new(spans(3, 5), spans(3, 5), false, 0);
    assert_eq!(s.proposal_count(&c), Some(16 * 16));
    assert!(repeats(&s, &c));
}
#[test]
fn change_empty_when_minimum_exceeds_list_size() {
    let p = problem(vec![vec![1, 2, 3]]);
    let a = p.initial_assignment();
    let c = ListSelectionContext::new(&p, &a).unwrap();
    let mut s = SubListChangeSource::new(spans(100, usize::MAX), Destination(0), false, 0);
    assert_eq!(s.proposal_count(&c), Some(0));
    assert_eq!(s.size(&c), Some(0));
    assert!(repeats(&s, &c));
    let mut i = s.open(&c);
    assert_eq!(i.next(), None);
    assert_eq!(i.next(), None);
}
#[test]
fn swap_empty_when_minimum_exceeds_list_size() {
    let p = problem(vec![vec![1, 2, 3]]);
    let a = p.initial_assignment();
    let c = ListSelectionContext::new(&p, &a).unwrap();
    let mut s = SubListSwapSource::new(spans(100, usize::MAX), spans(100, usize::MAX), false, 0);
    assert_eq!(s.proposal_count(&c), Some(0));
    assert_eq!(s.size(&c), Some(0));
    assert!(repeats(&s, &c));
    let mut i = s.open(&c);
    assert_eq!(i.next(), None);
    assert_eq!(i.next(), None);
}
