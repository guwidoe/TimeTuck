// Adapted Community TEST-only mixed generators and easy score calculators.
// Apache-2.0; provenance and modifications in compat/provenance.json.
use timetuck::*;
pub fn problem() -> MixedProblem<i32, i32> {
    // generateUninitializedSolution(2,3,3): two basic fields per entity,
    // three list values, three other values, two initially empty list rows.
    MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]); 4]).unwrap(),
        ListProblem::new(vec![0, 1, 2], vec![ListEntity::new(vec![]); 2]).unwrap(),
    )
}
pub fn network() -> MixedConstraintNetwork<'static, i32, i32, SimpleScore> {
    let mut n = MixedConstraintNetwork::new(4, 2, 3);
    // Single-entity and multi-entity TEST calculators have the same objective:
    // +1 per assigned basic field; +2 for singleton list, otherwise +1.
    n.add_constraint(
        "source objective",
        vec![
            MixedScope::Basic(0),
            MixedScope::Basic(1),
            MixedScope::Basic(2),
            MixedScope::Basic(3),
            MixedScope::ListEntity(0),
            MixedScope::ListEntity(1),
        ],
        |facts| {
            SimpleScore::new(
                facts
                    .iter()
                    .map(|f| match f {
                        MixedFact::Basic(v) => i64::from(v.is_some()),
                        MixedFact::ListEntity(row) => {
                            if row.len() == 1 {
                                2
                            } else {
                                1
                            }
                        }
                        _ => 0,
                    })
                    .sum(),
            )
        },
    )
    .unwrap();
    n
}
