//! Domain/scoring fixture adapted from Timefold Community follower_set and
//! follower testdomain fixtures, Apache-2.0. See compat/provenance.json.
use timetuck::*;
#[derive(Clone)]
struct Follower {
    id: usize,
    leaders: Vec<usize>,
}
type Minimum = (Vec<u16>, usize, usize);
pub type Shadow = StreamConstraint<usize, (usize, Option<usize>)>;
pub fn graph(
    dependencies: &[Vec<usize>],
    codes: &[String],
) -> (
    StreamGraph<'static, SimpleScore>,
    PlanningStreamSource<usize>,
    Shadow,
) {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, (usize, Option<usize>)>();
    let followers = b.source::<usize, Follower>();
    let members = b
        .join_containing(followers.port(), s.port(), |f| f.leaders.clone(), |r| r.0)
        .unwrap();
    let keys: Vec<Vec<u16>> = codes.iter().map(|s| s.encode_utf16().collect()).collect();
    let minima = b
        .group_collect_conditionally::<_, _, _, Minimum, SetCollector<Minimum>>(
            &members,
            |(f, _)| f.id,
            |(_, r)| r.1.is_some(),
            move |(f, r)| {
                let value = r.1.unwrap();
                // Source supplier keeps the first leader for equal code strings.
                (
                    keys[value].clone(),
                    f.leaders.iter().position(|id| *id == r.0).unwrap(),
                    value,
                )
            },
        )
        .unwrap();
    let minimum = b
        .map(&minima, |(_, values)| {
            values.first_key_value().map(|(key, _)| key.2)
        })
        .unwrap();
    // Preserve every follower, including all-null and not-yet-bound groups.
    let followed = b
        .follow_reference(followers.port(), &minimum, |f| Some(f.id))
        .unwrap();
    let values = b.map(&followed, |(f, v)| (f.id, v.flatten())).unwrap();
    let shadow = b.score("derived", &values, |_| SimpleScore::ZERO).unwrap();
    let follower_values = b.map(&values, |(_, v)| *v).unwrap();
    let leader_values = b.map(s.port(), |r| r.1).unwrap();
    let all = b.concat(&leader_values, &follower_values).unwrap();
    let counts = b.group_collect(&all, |v| (*v, 1)).unwrap();
    b.score("Minimize value count", &counts, |(_, n)| {
        SimpleScore::new(-(n.count as i64).pow(2))
    })
    .unwrap();
    let mut g = b.build();
    let mut t = g.transaction();
    for (id, leaders) in dependencies.iter().enumerate() {
        t.apply(
            &followers,
            TupleChange::put(
                id,
                Follower {
                    id,
                    leaders: leaders.clone(),
                },
            ),
        )
        .unwrap();
    }
    t.commit().unwrap();
    (g, s, shadow)
}
