// Adapted from Community UniEnumeratingStreamTest, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md for attribution.
use timetuck::*;

// Stable identity, current list, whole-entity pin, pinned prefix.
type Entity = (usize, Vec<usize>, bool, usize);

fn options(include_null: bool, exclude_pinned: bool) -> DatasetEnumerationOptions {
    DatasetEnumerationOptions {
        include_null,
        exclude_pinned,
    }
}
fn expected(null: bool, ids: &[usize]) -> Vec<Option<usize>> {
    null.then_some(None)
        .into_iter()
        .chain(ids.iter().copied().map(Some))
        .collect()
}
fn entities_case(basic: bool, include_null: bool, pins: bool, exclude_pinned: bool) {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, Entity>();
    let d = EnumeratedDataset::capture(
        &mut b,
        source.port(),
        options(include_null, exclude_pinned),
        |e| e.2,
    )
    .unwrap();
    let mut graph = b.build();
    let count = if pins { 3 } else { 2 };
    let values = if pins { 5 } else { 2 };
    let mut tx = graph.transaction();
    for id in 0..count {
        let row = (
            id,
            (0..values).filter(|v| v % count == id).collect(),
            pins && id == 0,
            usize::from(pins && id == 1),
        );
        tx.apply(&source, TupleChange::put(id, row)).unwrap();
    }
    tx.commit().unwrap();
    let ids = |g: &StreamGraph<'_, SimpleScore>| {
        d.iter(g)
            .unwrap()
            .map(|row| row.map(|e| e.0))
            .collect::<Vec<_>>()
    };
    let initial = if exclude_pinned {
        vec![1, 2]
    } else {
        (0..count).collect()
    };
    assert_eq!(ids(&graph), expected(include_null, &initial));
    let added = (count, if basic { vec![0] } else { vec![] }, pins, 0);
    let mut tx = graph.transaction();
    tx.apply(&source, TupleChange::put(count, added)).unwrap();
    tx.apply(&source, TupleChange::retract(1)).unwrap();
    tx.commit().unwrap();
    let final_ids = if exclude_pinned {
        vec![2]
    } else if pins {
        vec![0, 2, 3]
    } else {
        vec![0, 2]
    };
    assert_eq!(ids(&graph), expected(include_null, &final_ids));
}

#[test]
fn for_each_basic_var() {
    entities_case(true, false, false, false);
}
#[test]
fn for_each_including_null_basic_var() {
    entities_case(true, true, false, false);
}
#[test]
fn for_each_list_var() {
    entities_case(false, false, false, false);
}
#[test]
fn for_each_including_null_list_var() {
    entities_case(false, true, false, false);
}
#[test]
fn for_each_list_var_including_pinned() {
    entities_case(false, false, true, false);
}
#[test]
fn for_each_including_null_list_var_including_pinned() {
    entities_case(false, true, true, false);
}
#[test]
fn for_each_excluding_pinned_list_var() {
    entities_case(false, false, true, true);
}
#[test]
fn for_each_excluding_pinned_including_null_list_var() {
    entities_case(false, true, true, true);
}

fn values_case(include_null: bool, exclude_pinned: bool) {
    // Match the explicit list overrides in the four upstream value cases.
    let p = ListProblem::new(
        (0..5).collect(),
        vec![
            ListEntity::new(vec![0]).with_pinned(true),
            ListEntity::new(vec![1, 2]).with_pinned_prefix(1),
            ListEntity::new(vec![3]),
        ],
    )
    .unwrap();
    // An entity that is not part of the problem must not pin its referenced value.
    let _not_registered = ListEntity::new(vec![4]).with_pinned(true);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let sources = ListStreamSources::new(&mut b);
    let d =
        list_value_dataset(&mut b, &sources, &p, options(include_null, exclude_pinned)).unwrap();
    let director =
        ListStreamScoreDirector::new(&p, b.build(), sources, p.initial_assignment()).unwrap();
    let actual: Vec<_> = d
        .iter(director.graph())
        .unwrap()
        .map(|row| row.map(|v| v.value))
        .collect();
    assert_eq!(
        actual,
        expected(
            include_null,
            if exclude_pinned {
                &[2, 3, 4]
            } else {
                &[0, 1, 2, 3, 4]
            }
        )
    );
}
#[test]
fn for_each_list_var_including_pinned_values() {
    values_case(false, false);
}
#[test]
fn for_each_including_null_list_var_including_pinned_values() {
    values_case(true, false);
}
#[test]
fn for_each_excluding_pinned_list_var_values() {
    values_case(false, true);
}
#[test]
fn for_each_excluding_pinned_including_null_list_var_values() {
    values_case(true, true);
}
