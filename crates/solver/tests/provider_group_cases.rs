// Adapted from pinned Community MixedMoveSelectorTest and
// MixedMoveSelectorSplitBiasIT with BiasReport helpers. Apache-2.0; see provenance.
use timetuck::*;
fn unused(_: &(), _: &(), _: u64) -> Vec<i32> {
    panic!("counting must not open a provider")
}
#[test]
fn plain_selector_counts_as_one() {
    let mut provider = unused;
    assert_eq!(MoveProviderGroup::leaf(&mut provider).leaf_count(), 1);
}
#[test]
fn flat_union_counts_children() {
    let (mut a, mut b, mut c) = (unused, unused, unused);
    let group = MoveProviderGroup::group([
        MoveProviderGroup::leaf(&mut a),
        MoveProviderGroup::leaf(&mut b),
        MoveProviderGroup::leaf(&mut c),
    ]);
    assert_eq!(group.leaf_count(), 3);
}
#[test]
fn nested_union_counts_leaves() {
    let (mut a, mut b, mut c) = (unused, unused, unused);
    let inner = MoveProviderGroup::group([
        MoveProviderGroup::leaf(&mut a),
        MoveProviderGroup::leaf(&mut b),
    ]);
    let outer = MoveProviderGroup::group([inner, MoveProviderGroup::leaf(&mut c)]);
    assert_eq!(outer.leaf_count(), 3);
}
struct Tagged(&'static str);
impl MoveProvider<FiniteProblem<i32>, Assignment<i32>, &'static str> for Tagged {
    fn moves<'a>(
        &'a mut self,
        _: &'a FiniteProblem<i32>,
        _: &'a Assignment<i32>,
        _: u64,
    ) -> Box<dyn Iterator<Item = &'static str> + 'a>
    where
        &'static str: 'a,
    {
        Box::new(std::iter::repeat(self.0))
    }
}
#[test]
fn mixed_groups_split_proportionally_to_mover_count() {
    let p = FiniteProblem::new((0..3).map(|_| Variable::new(vec![0, 1])).collect()).unwrap();
    let a = p.initial_assignment();
    let (mut ca, mut cb, mut cc) = (
        Tagged("classic-a"),
        Tagged("classic-b"),
        Tagged("classic-c"),
    );
    let (mut na, mut nb) = (Tagged("neighborhood-a"), Tagged("neighborhood-b"));
    let group = MoveProviderGroup::group([
        MoveProviderGroup::group([
            MoveProviderGroup::leaf(&mut ca),
            MoveProviderGroup::leaf(&mut cb),
            MoveProviderGroup::leaf(&mut cc),
        ]),
        MoveProviderGroup::group([
            MoveProviderGroup::leaf(&mut na),
            MoveProviderGroup::leaf(&mut nb),
        ]),
    ]);
    let mut union = group.into_union(UnionOrder::Random { seed: 0 }).unwrap();
    for event in [
        MoveProviderEvent::SolveStarted,
        MoveProviderEvent::PhaseStarted,
        MoveProviderEvent::StepStarted,
    ] {
        union.event(event);
    }
    let mut counts = [0; 2];
    {
        let mut moves = union.moves(&p, &a, 0);
        for _ in 0..100_000 {
            let code = moves.next().unwrap();
            counts[usize::from(!code.starts_with("classic"))] += 1;
        }
    }
    for event in [
        MoveProviderEvent::StepEnded,
        MoveProviderEvent::PhaseEnded,
        MoveProviderEvent::SolveEnded,
    ] {
        union.event(event);
    }
    for (count, p) in counts.into_iter().zip([0.6, 0.4]) {
        assert!(count > 0);
        let sigma = (count as f64 - 100_000.0 * p).abs() / (100_000.0 * p * (1.0 - p)).sqrt();
        assert!(sigma <= 5.0, "count={count}, p={p}, sigma={sigma}");
    }
}
