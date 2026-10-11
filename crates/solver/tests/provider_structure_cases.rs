// Adapted pinned Community MixedMoveSelectorTest; Apache-2.0, see provenance.
use timetuck::*;
fn unused(_: &(), _: &(), _: u64) -> Vec<i32> {
    panic!("counting must not open a provider")
}
type Provider<'a> = &'a mut dyn MoveProvider<(), (), i32>;
fn union(children: Vec<Provider<'_>>) -> ProviderUnion<'_, (), (), i32> {
    ProviderUnion::unweighted(children, UnionOrder::Random { seed: 0 }).unwrap()
}
#[test]
fn filtering_sees_through_to_child() {
    let (mut a, mut b, mut c) = (unused, unused, unused);
    let inner = union(vec![&mut a, &mut b, &mut c]);
    let filtering = FilteredMoveProvider::new(
        inner,
        usize::MAX,
        usize::MAX,
        |_: &(), _: &(), _: u64, _: &i32| true,
    );
    assert_eq!(filtering.neighborhood_count().unwrap(), 3);
}
#[test]
fn caching_sees_through_to_child() {
    let (mut a, mut b) = (unused, unused);
    let caching = CachedProviderMoves::<_, i32>::new(
        union(vec![&mut a, &mut b]),
        ProviderCacheScope::Phase,
        100,
    );
    assert_eq!(caching.neighborhood_count().unwrap(), 2);
    assert!(caching.cached().is_none());
}
#[test]
fn selected_count_limit_sees_through_to_child() {
    let (mut a, mut b) = (unused, unused);
    let limited = LimitedMoveProvider::new(union(vec![&mut a, &mut b]), 5);
    assert_eq!(limited.neighborhood_count().unwrap(), 2);
}
struct Source;
impl SelectionSource<(), &'static str> for Source {
    fn size(&mut self, _: &()) -> Option<usize> {
        Some(1)
    }
    fn open<'a>(&'a mut self, _: &'a ()) -> Box<dyn Iterator<Item = &'static str> + 'a>
    where
        &'static str: 'a,
    {
        Box::new(std::iter::once("leaf"))
    }
}
#[test]
fn cartesian_product_counts_as_one() {
    let mut product = SelectionProduct::new(vec![Source, Source, Source], true, 1);
    let provider = move |_: &(), _: &(), _: u64| {
        product
            .snapshot(&())
            .unwrap()
            .samples(0)
            .take(1)
            .collect::<Vec<_>>()
    };
    assert_eq!(provider.neighborhood_count().unwrap(), 1);
}
#[test]
fn probability_selector_counts_as_one_and_is_not_rejected() {
    let probability = WeightedSelection::new(vec![("a1", 1.0)]).unwrap();
    let provider = move |_: &(), _: &(), _: u64| vec![*probability.choose(0.0).unwrap().unwrap()];
    assert_eq!(provider.neighborhood_count().unwrap(), 1);
}
#[test]
fn weighted_union_throws() {
    let (mut a, mut b) = (unused, unused);
    let weighted = ProviderUnion::new(
        vec![(&mut a as Provider<'_>, 1), (&mut b, 2)],
        UnionOrder::Random { seed: 0 },
    )
    .unwrap();
    let error = weighted.neighborhood_count().unwrap_err();
    assert_eq!(error, NeighborhoodCountError::WeightedUnion);
    assert!(error.to_string().contains("Neighborhoods API"));
}
#[test]
fn weighted_nested_union_throws() {
    let (mut a, mut b, mut c) = (unused, unused, unused);
    let mut weighted = ProviderUnion::new(
        vec![(&mut a as Provider<'_>, 1), (&mut b, 2)],
        UnionOrder::Random { seed: 0 },
    )
    .unwrap();
    let outer = union(vec![&mut weighted, &mut c]);
    let error = outer.neighborhood_count().unwrap_err();
    assert_eq!(error, NeighborhoodCountError::WeightedUnion);
    assert!(error.to_string().contains("Neighborhoods API"));
}
