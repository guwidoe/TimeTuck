// Adapted from Community single/distinct cascading shadow tests and fixtures.
// Apache-2.0; see compat/provenance.json. Native explicit callbacks replace reflection.
use timetuck::*;
fn problem() -> ListProblem<i32> {
    ListProblem::new(
        vec![1, 2, 3],
        vec![ListEntity::new(vec![]), ListEntity::new(vec![])],
    )
    .unwrap()
}
fn assigned() -> ListAssignment<i32> {
    ListAssignment::new(vec![vec![1, 2, 3], vec![]])
}
fn update(
    cache: &mut ListCascadeCache<i32, i32>,
    a: &ListAssignment<i32>,
    dirty: &[i32],
    calls: &mut [usize; 3],
) {
    cache
        .update(a, dirty, |c| {
            c.position?;
            calls[*c.value as usize - 1] += 1;
            Some(*c.value + 1)
        })
        .unwrap();
}
#[test]
fn single_update_all_next_values() {
    let p = problem();
    let mut cache = ListCascadeCache::from_values(&p, &p.initial_assignment(), []).unwrap();
    let mut calls = [0; 3];
    update(&mut cache, &assigned(), &[], &mut calls);
    assert_eq!(
        cache.shadows().iter().map(|(_, v)| *v).collect::<Vec<_>>(),
        [Some(2), Some(3), Some(4)]
    );
    assert_eq!(calls, [1, 1, 1]);
}
#[test]
fn single_stop_update_next_values() {
    // Import the upstream preseeded second shadow, then dirty the first value.
    // This replaces the deliberately partial Java owner/previous notifications.
    let mut cache = ListCascadeCache::from_values(&problem(), &assigned(), [(2, 3)]).unwrap();
    let mut calls = [0; 3];
    update(&mut cache, &assigned(), &[1], &mut calls);
    assert_eq!(
        cache.shadows().iter().map(|(_, v)| *v).collect::<Vec<_>>(),
        [Some(2), Some(3), None]
    );
    assert_eq!(calls, [1, 1, 0]);
}
#[test]
fn distinct_update_all_next_values() {
    let p = problem();
    let mut first = ListCascadeCache::from_values(&p, &p.initial_assignment(), []).unwrap();
    let mut second = first.clone();
    let (mut calls, mut second_calls) = ([0; 3], [0; 3]);
    update(&mut first, &assigned(), &[], &mut calls);
    update(&mut second, &assigned(), &[], &mut second_calls);
    assert_eq!(first.shadows()[&1], Some(2));
    assert_eq!(second.shadows()[&1], Some(2));
    assert_eq!(calls[0], 1);
    assert_eq!(second_calls[0], 1);
    assert_eq!(first.shadows()[&2], Some(3));
    assert_eq!(second.shadows()[&2], Some(3));
    assert_eq!(calls[1], 1);
    assert_eq!(second_calls[1], 1);
    assert_eq!(second.shadows()[&3], Some(4));
    assert_eq!(second_calls[2], 1);
}
#[test]
fn distinct_stop_update_next_values() {
    let p = problem();
    let mut first = ListCascadeCache::from_values(&p, &assigned(), [(2, 3)]).unwrap();
    let mut second = ListCascadeCache::from_values(&p, &assigned(), []).unwrap();
    let (mut calls, mut second_calls) = ([0; 3], [0; 3]);
    update(&mut first, &assigned(), &[1], &mut calls);
    update(&mut second, &assigned(), &[1], &mut second_calls);
    assert_eq!(
        first.shadows().iter().map(|(_, v)| *v).collect::<Vec<_>>(),
        [Some(2), Some(3), None]
    );
    assert_eq!(
        second.shadows().iter().map(|(_, v)| *v).collect::<Vec<_>>(),
        [Some(2), Some(3), Some(4)]
    );
    assert_eq!(calls, [1, 1, 0]);
    assert_eq!(second_calls, [1, 1, 1]);
}
