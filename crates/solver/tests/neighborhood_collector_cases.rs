// Adapted from pinned Timefold Community NeighborhoodsCollectorsTest.
// Apache-2.0; see compat/provenance.json for each source method.
use timetuck::*;
type List = ListCollector<u64, i32>;
struct Length;
impl CollectorFinisher<ListSnapshot<u64, i32>> for Length {
    type Output = usize;
    fn finish(list: ListSnapshot<u64, i32>) -> usize {
        list.len()
    }
}
fn list_values(list: ListSnapshot<u64, i32>) -> Vec<i32> {
    list.iter().copied().collect()
}
fn list_case<T>(one: T, two: T, map: impl Fn(u64, T) -> (u64, i32)) {
    let mut collector = CollectorSession::<(u64, i32), List>::new();
    assert_eq!(list_values(collector.finish()), []);
    let h1 = collector.insert_with(|id| map(id, one)).unwrap();
    assert_eq!(list_values(collector.finish()), [1]);
    let h2 = collector.insert_with(|id| map(id, two)).unwrap();
    assert_eq!(list_values(collector.finish()), [1, 2]);
    collector.remove(&h1).unwrap();
    assert_eq!(list_values(collector.finish()), [2]);
    collector.remove(&h2).unwrap();
    assert_eq!(list_values(collector.finish()), []);
}
#[test]
fn uni_to_list() {
    list_case(1, 2, |id, a| (id, a));
}
#[test]
fn bi_to_list() {
    list_case((1, "x"), (2, "y"), |id, (a, _b)| (id, a));
}
fn finishing_case<T>(one: T, two: T, map: impl Fn(u64, T) -> (u64, i32)) {
    let mut collector = CollectorSession::<(u64, i32), FinishingCollector<List, Length>>::new();
    assert_eq!(collector.finish(), 0);
    let h1 = collector.insert_with(|id| map(id, one)).unwrap();
    assert_eq!(collector.finish(), 1);
    let h2 = collector.insert_with(|id| map(id, two)).unwrap();
    assert_eq!(collector.finish(), 2);
    collector.remove(&h1).unwrap();
    assert_eq!(collector.finish(), 1);
    collector.remove(&h2).unwrap();
    assert_eq!(collector.finish(), 0);
}
#[test]
fn uni_collect_and_then() {
    finishing_case(1, 2, |id, a| (id, a));
}
#[test]
fn bi_collect_and_then() {
    finishing_case((1, "x"), (2, "y"), |id, (a, _b)| (id, a));
}
fn compose_case<T>(one: T, two: T, map: impl Fn(u64, T) -> ((u64, i32), (u64, i32))) {
    let mut collector =
        CollectorSession::<((u64, i32), (u64, i32)), PairCollector<List, List>>::new();
    let finish = |c: &CollectorSession<_, PairCollector<List, List>>| {
        let (a, b) = c.finish();
        (list_values(a), list_values(b))
    };
    assert_eq!(finish(&collector), (vec![], vec![]));
    let h1 = collector.insert_with(|id| map(id, one)).unwrap();
    assert_eq!(finish(&collector), (vec![1], vec![1]));
    let h2 = collector.insert_with(|id| map(id, two)).unwrap();
    assert_eq!(finish(&collector), (vec![1, 2], vec![1, 2]));
    collector.remove(&h1).unwrap();
    assert_eq!(finish(&collector), (vec![2], vec![2]));
    collector.remove(&h2).unwrap();
    assert_eq!(finish(&collector), (vec![], vec![]));
}
#[test]
fn uni_compose2() {
    compose_case(1, 2, |id, a| ((id, a), (id, a)));
}
#[test]
fn bi_compose2() {
    compose_case((1, "x"), (2, "y"), |id, (a, _b)| ((id, a), (id, a)));
}
