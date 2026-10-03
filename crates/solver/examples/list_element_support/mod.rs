//! Fixtures adapted from Community testdomain/shadow/list_element, Apache-2.0.
//! See compat/provenance.json. Explicit stream projections replace annotations.
use timetuck::*;
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Times {
    pub owner: Option<usize>,
    pub previous: Option<usize>,
    pub start: Option<i32>,
    pub end: Option<i32>,
}
pub type Ends = StreamConstraint<usize, (usize, i32)>;
pub type Elements = StreamConstraint<usize, Times>;
pub type Totals = StreamConstraint<usize, (usize, Option<i32>)>;
pub fn ordered(
    starts: Vec<i32>,
    durations: Vec<i32>,
) -> (
    StreamGraph<'static, SimpleScore>,
    ListStreamSources<usize>,
    Ends,
    Elements,
) {
    let mut b = StreamGraphBuilder::new();
    let s = ListStreamSources::<usize>::new(&mut b);
    // Recompute only a changed owner's chain, in list order. Java int addition
    // wraps; no assumption that the last end is the maximum end is made.
    let chains = b
        .map(s.entities.port(), move |(e, list)| {
            let mut end = starts[*e];
            let times = list
                .iter()
                .map(|&v| {
                    let start = end;
                    end = end.wrapping_add(durations[v]);
                    (start, end)
                })
                .collect::<Vec<_>>();
            (*e, times, end)
        })
        .unwrap();
    let last = b.map(&chains, |(e, _, end)| (*e, *end)).unwrap();
    let ends = b
        .score("Minimize last end time", &last, |(_, end)| {
            SimpleScore::new(-i64::from(*end))
        })
        .unwrap();
    let linked = b
        .follow_reference(s.values.port(), &chains, |r| r.position.map(|p| p.entity))
        .unwrap();
    let times = b
        .map(&linked, |(r, chain)| {
            let time = r
                .position
                .zip(chain.as_ref())
                .and_then(|(p, (_, times, _))| times.get(p.index));
            Times {
                owner: r.position.map(|p| p.entity),
                previous: r.previous,
                start: time.map(|t| t.0),
                end: time.map(|t| t.1),
            }
        })
        .unwrap();
    let elements = b
        .score("Assign all values", &times, |t| {
            SimpleScore::new(if t.owner.is_none() { -100 } else { 0 })
        })
        .unwrap();
    (b.build(), s, ends, elements)
}
pub fn mixed() -> (
    StreamGraph<'static, SimpleScore>,
    MixedStreamSources<i32, usize>,
    Totals,
    Totals,
) {
    let mut b = StreamGraphBuilder::new();
    let s = MixedStreamSources::<i32, usize>::new(&mut b);
    let padded = b
        .map(s.basic.port(), |(v, duration)| {
            (*v, duration.map(|d| d.wrapping_add(1)))
        })
        .unwrap();
    let pads = b
        .score("padded values", &padded, |_| SimpleScore::ZERO)
        .unwrap();
    let members = b
        .join_containing(
            s.lists.entities.port(),
            &padded,
            |(_, list)| list.clone(),
            |(v, _)| *v,
        )
        .unwrap();
    let groups = b
        .group_list(&members, |((e, _), (_, duration))| (*e, *duration))
        .unwrap();
    let sums = b
        .map(&groups, |(_, rows)| {
            rows.iter()
                .try_fold(0_i32, |sum, value| value.map(|v| sum.wrapping_add(v)))
        })
        .unwrap();
    let linked = b
        .follow_reference(s.lists.entities.port(), &sums, |(e, _)| Some(*e))
        .unwrap();
    let totals = b
        .map(&linked, |((e, _), sum)| (*e, sum.unwrap_or(Some(0))))
        .unwrap();
    // The source mixed fixture has no constraint provider. This observation
    // constraint is score-neutral, including null totals and empty owners.
    let totals = b
        .score("total durations", &totals, |_| SimpleScore::ZERO)
        .unwrap();
    (b.build(), s, totals, pads)
}
