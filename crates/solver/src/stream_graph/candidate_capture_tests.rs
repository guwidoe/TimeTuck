//! Original saved-candidate controls; no upstream method credit.
use super::*;
use crate::{HardSoftScore, IntCollector};
use std::cell::Cell;

type Row = (usize, usize, usize); // person, session, group
type Pair = (usize, usize);
type Groups = StreamDataset<Pair, (Pair, IntSummary)>;
type Graph = StreamGraph<'static, HardSoftScore>;

type Fixture = (
    Graph,
    StreamSource<usize, Row>,
    Groups,
    Groups,
    Rc<Cell<usize>>,
);

fn problem() -> Fixture {
    let mut b = StreamGraphBuilder::new();
    let calls = Rc::new(Cell::new(0));
    let source = b.source::<usize, Row>();
    let count = calls.clone();
    let present = b
        .filter(source.port(), move |_| {
            count.set(count.get() + 1);
            true
        })
        .unwrap();
    let assigned = b.map(&present, |row| *row).unwrap();
    let count = calls.clone();
    let loads = b
        .group_collect(&assigned, move |r| {
            count.set(count.get() + 1);
            ((r.1, r.2), 1)
        })
        .unwrap();
    let load_data = b.dataset(&loads).unwrap();
    let count = calls.clone();
    b.score("loads", &loads, move |(_, summary)| {
        count.set(count.get() + 1);
        HardSoftScore::new(-summary.sum * summary.sum, 0)
    })
    .unwrap();
    let joined = b
        .join_equal(&assigned, &assigned, |r| (r.1, r.2), |r| (r.1, r.2))
        .unwrap();
    let count = calls.clone();
    let pairs = b
        .filter(&joined, move |(a, b)| {
            count.set(count.get() + 1);
            a.0 < b.0
        })
        .unwrap();
    let encounters = b.group_collect(&pairs, |(a, b)| ((a.0, b.0), 1)).unwrap();
    let pair_data = b.dataset(&encounters).unwrap();
    let count = calls.clone();
    b.score("encounters", &encounters, move |(_, summary)| {
        count.set(count.get() + 1);
        HardSoftScore::new(0, summary.sum * summary.sum)
    })
    .unwrap();
    let mut graph = b.build();
    let mut initial = graph.transaction();
    for id in 0..40 {
        initial
            .apply(&source, TupleChange::put(id, (id % 8, id / 8, id % 3)))
            .unwrap();
    }
    initial.commit().unwrap();
    graph.bind_fixed_planning_source(&source, 40).unwrap();
    (graph, source, load_data, pair_data, calls)
}

fn assert_state(graph: &Graph, source: &StreamSource<usize, Row>, loads: &Groups, pairs: &Groups) {
    let rows = graph.source_rows(source).unwrap();
    let mut load_counts = BTreeMap::<Pair, IntCollector>::new();
    let mut pair_counts = BTreeMap::<Pair, IntCollector>::new();
    for &(_, session, group) in rows.values() {
        load_counts.entry((session, group)).or_default().insert(1);
    }
    for &(person, session, group) in rows.values() {
        for &(other, other_session, other_group) in rows.values() {
            if person < other && session == other_session && group == other_group {
                pair_counts.entry((person, other)).or_default().insert(1);
            }
        }
    }
    let actual = |dataset: &Groups| {
        graph
            .dataset_rows(dataset)
            .unwrap()
            .iter()
            .map(|(key, (_, summary))| (*key, *summary))
            .collect::<BTreeMap<_, _>>()
    };
    assert_eq!(
        actual(loads),
        load_counts
            .iter()
            .map(|(&key, values)| (key, values.summary()))
            .collect()
    );
    assert_eq!(
        actual(pairs),
        pair_counts
            .iter()
            .map(|(&key, values)| (key, values.summary()))
            .collect()
    );
    assert_eq!(
        graph.score(),
        HardSoftScore::new(
            -load_counts
                .values()
                .map(|values| values.summary().sum.pow(2))
                .sum::<i64>(),
            pair_counts
                .values()
                .map(|values| values.summary().sum.pow(2))
                .sum(),
        )
    );
}

#[test]
fn graph_capture_keeps_multiple_candidates_from_one_base_and_rolls_back_outer_install() {
    let (mut graph, source, loads, pairs, calls) = problem();
    assert_state(&graph, &source, &loads, &pairs);
    let base_rows = graph.source_rows(&source).unwrap().clone();
    let base = graph.capture_full();
    let (first, first_again, first_rows) = {
        let mut trial = graph.transaction();
        trial
            .apply(&source, TupleChange::put(0, (0, 0, 2)))
            .unwrap();
        trial
            .apply(&source, TupleChange::put(1, (1, 0, 0)))
            .unwrap();
        assert_state(trial.graph, &source, &loads, &pairs);
        let before = calls.get();
        let first = trial.capture().unwrap();
        let again = trial.capture().unwrap();
        assert_eq!(calls.get(), before);
        (
            first,
            again,
            trial.graph.source_rows(&source).unwrap().clone(),
        )
    };
    assert_eq!(graph.source_rows(&source).unwrap(), &base_rows);
    let (second, second_rows) = {
        let mut trial = graph.transaction();
        trial
            .apply(&source, TupleChange::put(0, (0, 0, 1)))
            .unwrap();
        trial
            .apply(&source, TupleChange::put(9, (1, 1, 2)))
            .unwrap();
        (
            trial.capture().unwrap(),
            trial.graph.source_rows(&source).unwrap().clone(),
        )
    };
    let before = calls.get();
    graph.install(first);
    assert_eq!(calls.get(), before);
    assert_eq!(graph.source_rows(&source).unwrap(), &first_rows);
    assert_state(&graph, &source, &loads, &pairs);
    graph.assert_consistent().unwrap();
    graph.install(base);
    assert_eq!(graph.source_rows(&source).unwrap(), &base_rows);
    {
        let mut outer = graph.transaction();
        for id in 0..outer.graph.nodes.len() {
            outer.checkpoint(id);
        }
        outer.graph.install(first_again);
        assert_state(outer.graph, &source, &loads, &pairs);
        assert_eq!(outer.graph.source_rows(&source).unwrap(), &first_rows);
    }
    assert_eq!(graph.source_rows(&source).unwrap(), &base_rows);
    assert_state(&graph, &source, &loads, &pairs);
    graph.install(second);
    assert_eq!(graph.source_rows(&source).unwrap(), &second_rows);
    assert_state(&graph, &source, &loads, &pairs);
    graph.assert_consistent().unwrap();
}
