// Adapted from Timefold Community AbstractScoreDirectorSemanticsTest and its
// IncrementalScoreDirectorSemanticsTest weight calculator, Apache-2.0.
// See compat/provenance.json for exact test/fixture identities and substitutions.
use std::sync::Arc;
use timetuck::*;

#[derive(Debug)]
struct Entity {
    value: usize,
}
struct Solution {
    values: Vec<usize>,
    entities: Vec<Arc<Entity>>,
    weight: Option<i64>,
}
impl Solution {
    fn generate(values: usize, entities: usize) -> Self {
        Self {
            values: (0..values).collect(),
            entities: (0..entities)
                .map(|i| Arc::new(Entity { value: i % values }))
                .collect(),
            weight: None,
        }
    }
}
enum Change {
    Add,
    Remove,
    Weight,
}
#[derive(Default)]
struct Counter {
    count: usize,
    weight: Option<i64>,
}
impl IncrementalCalculator<Solution> for Counter {
    type Score = SimpleScore;
    type Justification = ();
    type Change = Change;
    fn supports_matches(&self) -> bool {
        true
    }
    fn reset(
        &mut self,
        s: &Solution,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.count = s.entities.len();
        self.weight = s.weight;
        Ok(())
    }
    fn after_change(
        &mut self,
        s: &Solution,
        _: &Change,
        r: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.reset(s, r)
    }
    fn calculate_score(&mut self) -> SimpleScore {
        SimpleScore::new(self.count as i64)
    }
    fn calculate_score_with_matches(
        &mut self,
        r: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<SimpleScore, MatchRegistryError> {
        let raw = self.calculate_score();
        if r.is_enabled()
            && let Some(weight) = self.weight
        {
            // The upstream test calculator registers the unweighted contribution.
            r.register("First weight", raw, ())?;
            Ok(SimpleScore::new((self.count as i64) * weight))
        } else {
            Ok(raw)
        }
    }
}
#[test]
fn independent_score_directors() {
    let factory = Counter::default;
    let mut first = CalculatorSession::new(Solution::generate(1, 1), factory(), false).unwrap();
    assert_eq!(first.score().unwrap(), SimpleScore::new(1));
    let mut second = CalculatorSession::new(Solution::generate(2, 2), factory(), false).unwrap();
    assert_eq!(second.score().unwrap(), SimpleScore::new(2));
    assert_eq!(first.score().unwrap(), SimpleScore::new(1));
    let entity = second.solution().entities[1].clone();
    second
        .change(&Change::Remove, |s| {
            s.entities.remove(1);
        })
        .unwrap();
    assert_eq!(second.score().unwrap(), SimpleScore::new(1));
    assert_eq!(first.score().unwrap(), SimpleScore::new(1));
    first
        .change(&Change::Add, |s| s.entities.push(entity.clone()))
        .unwrap();
    assert_eq!(first.score().unwrap(), SimpleScore::new(2));
    assert_eq!(second.score().unwrap(), SimpleScore::new(1));
    // Original assertions retain the moved entity's identity and value despite
    // its value being outside the first solution's unchanged value range.
    assert!(Arc::ptr_eq(&entity, &first.solution().entities[1]));
    assert_eq!(entity.value, 1);
    assert_eq!(first.solution().values, vec![0]);
}
#[test]
fn solution_based_score_weights() {
    let mut director =
        CalculatorSession::new(Solution::generate(1, 1), Counter::default(), true).unwrap();
    assert_eq!(director.score().unwrap(), SimpleScore::new(1));
    let mut replacement = Solution::generate(1, 1);
    replacement.weight = Some(2);
    director.reset(replacement).unwrap();
    assert_eq!(director.score().unwrap(), SimpleScore::new(2));
    let mut replacement = Solution::generate(1, 1);
    replacement.weight = Some(0);
    director.reset(replacement).unwrap();
    assert_eq!(director.score().unwrap(), SimpleScore::ZERO);
}
#[test]
fn mutable_constraint_configuration() {
    let mut director =
        CalculatorSession::new(Solution::generate(1, 1), Counter::default(), true).unwrap();
    assert_eq!(director.score().unwrap(), SimpleScore::new(1));
    director
        .change(&Change::Weight, |s| s.weight = Some(2))
        .unwrap();
    assert_eq!(director.score().unwrap(), SimpleScore::new(2));
}
