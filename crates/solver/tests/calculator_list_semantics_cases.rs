// Adapted from Timefold Community AbstractScoreDirectorSemanticsTest and its
// IncrementalScoreDirectorSemanticsTest calculator fixtures, Apache-2.0.
// Exact source/fixture identities and native substitutions: compat/provenance.json.
use timetuck::*;
#[derive(Default)]
struct EntityCount(usize);
impl IncrementalCalculator<ListAssignment<usize>> for EntityCount {
    type Score = SimpleScore;
    type Justification = ();
    type Change = Vec<usize>;
    fn reset(
        &mut self,
        a: &ListAssignment<usize>,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.0 = a.lists().len();
        Ok(())
    }
    fn calculate_score(&mut self) -> SimpleScore {
        SimpleScore::new(-(self.0 as i64))
    }
}
#[test]
fn list_variable_entity_pinning_supported() {
    let p = ListProblem::new(
        vec![0, 1],
        vec![
            ListEntity::new(vec![0]).with_pinned(true),
            ListEntity::new(vec![]),
        ],
    )
    .unwrap();
    let mut d =
        ListCalculatorScoreDirector::new(&p, p.initial_assignment(), EntityCount::default, false)
            .unwrap();
    assert_eq!(d.score().unwrap(), SimpleScore::new(-2).with_structural(-1));
    d.apply(&ListMove::Assign {
        value: 1,
        destination: ListPosition {
            entity: 1,
            index: 0,
        },
    })
    .unwrap()
    .commit();
    assert_eq!(d.score().unwrap(), SimpleScore::new(-2));
}
#[test]
fn list_variable_index_pinning_supported() {
    let p = ListProblem::new(
        vec![0, 1, 2],
        vec![
            ListEntity::new(vec![0]).with_pinned(true),
            ListEntity::new(vec![1]).with_pinned_prefix(1),
            ListEntity::new(vec![]),
        ],
    )
    .unwrap();
    let mut d =
        ListCalculatorScoreDirector::new(&p, p.initial_assignment(), EntityCount::default, false)
            .unwrap();
    assert_eq!(d.score().unwrap(), SimpleScore::new(-3).with_structural(-1));
    d.apply(&ListMove::Assign {
        value: 2,
        destination: ListPosition {
            entity: 2,
            index: 0,
        },
    })
    .unwrap()
    .commit();
    assert_eq!(d.score().unwrap(), SimpleScore::new(-3));
}
