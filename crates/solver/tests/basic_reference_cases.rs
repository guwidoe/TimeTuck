// Adapted from Timefold Community 2.7.0, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md for test/fixture attribution.
use timetuck::*;
#[test]
fn declarative_shadow_tracks_basic_variable_change_through_non_externalized_inverse_supply() {
    // Stable IDs: entity=0, groupA=1, groupB=2, ownerX=10, ownerY=11.
    let p = FiniteProblem::new(vec![
        Variable::new(vec![1, 2]).with_initial(1),
        Variable::new(vec![10, 11]).with_initial(10),
        Variable::new(vec![10, 11]).with_initial(11),
    ])
    .unwrap();
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, (usize, Option<usize>)>();
    let entities = b.filter(source.port(), |r| r.0 == 0).unwrap();
    let groups = b.filter(source.port(), |r| r.0 != 0).unwrap();
    let followed = b.follow_reference(&entities, &groups, |r| r.1).unwrap();
    let codes = b
        .map(&followed, |(_, g)| {
            g.as_ref().and_then(|r| r.1).map(|id| match id {
                10 => "ownerX",
                11 => "ownerY",
                _ => unreachable!(),
            })
        })
        .unwrap();
    let shadow = b.score("ownerCode", &codes, |_| SimpleScore::ZERO).unwrap();
    let mut d = StreamScoreDirector::new(&p, b.build(), source, p.initial_assignment()).unwrap();
    assert_eq!(
        d.graph().constraint_matches(&shadow).unwrap()[&0].0,
        Some("ownerX")
    );
    d.apply(&Move::Change {
        variable: 0,
        value: Some(2),
    })
    .unwrap()
    .commit();
    assert_eq!(d.assignment().values()[0], Some(2));
    assert_eq!(
        d.graph().constraint_matches(&shadow).unwrap()[&0].0,
        Some("ownerY")
    );
    d.apply(&Move::Change {
        variable: 2,
        value: Some(10),
    })
    .unwrap()
    .commit();
    assert_eq!(
        d.graph().constraint_matches(&shadow).unwrap()[&0].0,
        Some("ownerX")
    );
    d.apply(&Move::Change {
        variable: 1,
        value: Some(11),
    })
    .unwrap()
    .commit();
    assert_eq!(
        d.graph().constraint_matches(&shadow).unwrap()[&0].0,
        Some("ownerX")
    );
}
