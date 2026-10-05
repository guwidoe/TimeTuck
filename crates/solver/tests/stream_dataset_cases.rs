// Adapted from pinned Community Uni/BiEnumeratingStreamTest, Apache-2.0.
// See compat/provenance.json and notices. Stable native IDs replace Java entities.
use std::cell::Cell;
use timetuck::*;
type Entity = (usize, usize);
type Source = StreamSource<usize, Entity>;
fn entities(b: &mut StreamGraphBuilder<'_, SimpleScore>) -> Source {
    b.source()
}
fn load(g: &mut StreamGraph<'_, SimpleScore>, s: &Source, n: usize) {
    change(g, s, (0..n).map(|i| TupleChange::put(i, (i, i % 2))));
}
fn change(
    g: &mut StreamGraph<'_, SimpleScore>,
    s: &Source,
    changes: impl IntoIterator<Item = TupleChange<usize, Entity>>,
) {
    let mut t = g.transaction();
    for c in changes {
        t.apply(s, c).unwrap();
    }
    t.commit().unwrap();
}
fn rows<I: Ord + 'static, T: Ord + Clone + 'static>(
    g: &StreamGraph<'_, SimpleScore>,
    d: &StreamDataset<I, T>,
) -> Vec<T> {
    let mut r = g
        .dataset_rows(d)
        .unwrap()
        .values()
        .cloned()
        .collect::<Vec<_>>();
    r.sort();
    r
}
#[test]
fn uni_concat_uni_uni() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = entities(&mut b);
    let left = b.filter(s.port(), |e| e.1 == 0).unwrap();
    let right = b.filter(s.port(), |e| e.1 == 1).unwrap();
    let both = b.concat(&left, &right).unwrap();
    let d = b.dataset(&both).unwrap();
    let mut g = b.build();
    load(&mut g, &s, 4);
    assert_eq!(rows(&g, &d), vec![(0, 0), (1, 1), (2, 0), (3, 1)]);
    change(&mut g, &s, [TupleChange::put(1, (1, 0))]);
    assert_eq!(rows(&g, &d), vec![(0, 0), (1, 0), (2, 0), (3, 1)]);
    change(&mut g, &s, [TupleChange::retract(3)]);
    assert_eq!(rows(&g, &d), vec![(0, 0), (1, 0), (2, 0)]);
}
#[test]
fn uni_concat_self_produces_duplicates() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = entities(&mut b);
    let both = b.concat(s.port(), s.port()).unwrap();
    let d = b.dataset(&both).unwrap();
    let mut g = b.build();
    load(&mut g, &s, 2);
    assert_eq!(rows(&g, &d), vec![(0, 0), (0, 0), (1, 1), (1, 1)]);
}
#[test]
fn uni_group_by_1_mapping_0_collector() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = entities(&mut b);
    let keys = b.map(s.port(), |e| e.1).unwrap();
    let keys = b.distinct(&keys).unwrap();
    let d = b.dataset(&keys).unwrap();
    let mut g = b.build();
    load(&mut g, &s, 4);
    assert_eq!(rows(&g, &d), vec![0, 1]);
    change(&mut g, &s, [TupleChange::put(2, (2, 1))]);
    assert_eq!(rows(&g, &d), vec![0, 1]);
    change(&mut g, &s, [TupleChange::put(0, (0, 1))]);
    assert_eq!(rows(&g, &d), vec![1]);
}
#[test]
fn uni_group_by_1_mapping_1_collector() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = entities(&mut b);
    let grouped = b.group_list(s.port(), |e| (e.1, e.0)).unwrap();
    let labels = b
        .map(&grouped, |(k, vs)| {
            format!("Generated Value {k}={}", vs.len())
        })
        .unwrap();
    let d = b.dataset(&labels).unwrap();
    let mut g = b.build();
    load(&mut g, &s, 4);
    assert_eq!(
        rows(&g, &d),
        vec!["Generated Value 0=2", "Generated Value 1=2"]
    );
    change(&mut g, &s, [TupleChange::put(4, (4, 0))]);
    assert_eq!(
        rows(&g, &d),
        vec!["Generated Value 0=3", "Generated Value 1=2"]
    );
}
#[test]
fn uni_group_by_0_mapping_1_collector() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = entities(&mut b);
    let list = b.collect_list(s.port(), |e| e.0).unwrap();
    let d = b.dataset(&list).unwrap();
    let mut g = b.build();
    load(&mut g, &s, 3);
    assert_eq!(g.dataset_rows(&d).unwrap().len(), 1);
    change(&mut g, &s, [TupleChange::put(3, (3, 0))]);
    assert_eq!(g.dataset_rows(&d).unwrap().len(), 1);
}
#[test]
fn uni_distinct() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = entities(&mut b);
    let keys = b.map(s.port(), |e| e.1).unwrap();
    let unique = b.distinct(&keys).unwrap();
    let d = b.dataset(&unique).unwrap();
    let mut g = b.build();
    load(&mut g, &s, 4);
    assert_eq!(rows(&g, &d), vec![0, 1]);
}
#[derive(Clone, Default)]
struct CustomCount(usize);
impl ReversibleCollector<Entity> for CustomCount {
    type Output = usize;
    fn insert(&mut self, _: &Entity) {
        self.0 += 1;
    }
    fn remove(&mut self, _: &Entity) -> bool {
        if self.0 == 0 {
            return false;
        }
        self.0 -= 1;
        true
    }
    fn finish(&self) -> usize {
        self.0
    }
}
#[test]
fn uni_group_by_1_mapping_1_collector_custom_collector() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = entities(&mut b);
    let groups = b
        .group_collect_with::<_, _, _, _, CustomCount>(s.port(), |e| (e.1, *e))
        .unwrap();
    let labels = b
        .map(&groups, |(k, n)| format!("Generated Value {k}={n}"))
        .unwrap();
    let d = b.dataset(&labels).unwrap();
    let mut g = b.build();
    load(&mut g, &s, 4);
    assert_eq!(
        rows(&g, &d),
        vec!["Generated Value 0=2", "Generated Value 1=2"]
    );
    change(&mut g, &s, [TupleChange::retract(0)]);
    assert_eq!(
        rows(&g, &d),
        vec!["Generated Value 0=1", "Generated Value 1=2"]
    );
}
#[test]
fn uni_group_by_1_mapping_0_collector_solution_view() {
    let count = Cell::new(0);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = entities(&mut b);
    let mapped = b
        .map(s.port(), |e| {
            count.set(count.get() + 1);
            e.1
        })
        .unwrap();
    let unique = b.distinct(&mapped).unwrap();
    let d = b.dataset(&unique).unwrap();
    let mut g = b.build();
    load(&mut g, &s, 4);
    assert_eq!(rows(&g, &d), vec![0, 1]);
    assert!(count.get() > 0);
    let old = count.get();
    change(&mut g, &s, [TupleChange::put(2, (2, 1))]);
    assert_eq!(rows(&g, &d), vec![0, 1]);
    assert!(count.get() > old);
}
fn load_bi(g: &mut StreamGraph<'_, SimpleScore>, s: &Source, v: &StreamSource<usize, usize>) {
    let mut t = g.transaction();
    for i in 0..2 {
        t.apply(v, TupleChange::put(i, i)).unwrap();
    }
    for i in 0..4 {
        t.apply(s, TupleChange::put(i, (i, i % 2))).unwrap();
    }
    t.commit().unwrap();
}
#[test]
fn bi_concat_bi_bi() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = entities(&mut b);
    let v = b.source::<usize, usize>();
    let join = b.join_equal(s.port(), v.port(), |e| e.1, |v| *v).unwrap();
    let l = b.filter(&join, |(_, v)| *v == 0).unwrap();
    let r = b.filter(&join, |(_, v)| *v == 1).unwrap();
    let joined = b.concat(&l, &r).unwrap();
    let d = b.dataset(&joined).unwrap();
    let mut g = b.build();
    load_bi(&mut g, &s, &v);
    assert_eq!(
        rows(&g, &d),
        vec![((0, 0), 0), ((1, 1), 1), ((2, 0), 0), ((3, 1), 1)]
    );
}
fn padding(bi_first: bool) {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = entities(&mut b);
    let v = b.source::<usize, usize>();
    let l = b.filter(s.port(), |e| e.1 == 0).unwrap();
    let r = b.filter(s.port(), |e| e.1 == 1).unwrap();
    let mut g;
    if bi_first {
        let joined = b.join_equal(&l, v.port(), |e| e.1, |v| *v).unwrap();
        let padded = b.map(&r, |e| (*e, e.1)).unwrap();
        let both = b.concat(&joined, &padded).unwrap();
        let d = b.dataset(&both).unwrap();
        g = b.build();
        load_bi(&mut g, &s, &v);
        assert_eq!(
            rows(&g, &d),
            vec![((0, 0), 0), ((1, 1), 1), ((2, 0), 0), ((3, 1), 1)]
        );
    } else {
        let padded = b.map(&l, |e| (*e, e.1)).unwrap();
        let joined = b.join_equal(&r, v.port(), |e| e.1, |v| *v).unwrap();
        let both = b.concat(&padded, &joined).unwrap();
        let d = b.dataset(&both).unwrap();
        g = b.build();
        load_bi(&mut g, &s, &v);
        assert_eq!(
            rows(&g, &d),
            vec![((0, 0), 0), ((1, 1), 1), ((2, 0), 0), ((3, 1), 1)]
        );
    }
}
#[test]
fn bi_concat_uni_to_bi_with_padding() {
    padding(false);
}
#[test]
fn bi_concat_bi_to_uni_with_padding() {
    padding(true);
}
#[test]
fn bi_group_by_1_mapping_0_collector() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = entities(&mut b);
    let v = b.source::<usize, usize>();
    let joined = b.join_equal(s.port(), v.port(), |e| e.1, |v| *v).unwrap();
    let labels = b
        .map(&joined, |(_, v)| format!("Generated Value {v}"))
        .unwrap();
    let groups = b.distinct(&labels).unwrap();
    let d = b.dataset(&groups).unwrap();
    let mut g = b.build();
    load_bi(&mut g, &s, &v);
    assert_eq!(rows(&g, &d), vec!["Generated Value 0", "Generated Value 1"]);
}
#[test]
fn bi_group_by_1_mapping_1_collector() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = entities(&mut b);
    let v = b.source::<usize, usize>();
    let joined = b.join_equal(s.port(), v.port(), |e| e.1, |v| *v).unwrap();
    let groups = b
        .group_list(&joined, |(e, v)| (*v, format!("Generated Entity {}", e.0)))
        .unwrap();
    let labels = b
        .map(&groups, |(k, vs)| {
            format!("Generated Value {k}={}", vs.len())
        })
        .unwrap();
    let d = b.dataset(&labels).unwrap();
    let mut g = b.build();
    load_bi(&mut g, &s, &v);
    assert_eq!(
        rows(&g, &d),
        vec!["Generated Value 0=2", "Generated Value 1=2"]
    );
}
