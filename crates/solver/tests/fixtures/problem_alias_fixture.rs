// Adapted TEST-only Lavish/MockProblemChangeDirector fixtures, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::sync::{Arc, Mutex};
use timetuck::*;
pub type Object = Arc<Item>;
pub struct Item {
    pub code: String,
    pub state: Mutex<State>,
}
pub struct State {
    pub group: Option<Arc<String>>,
    pub value: Option<Object>,
    pub long: i64,
}
pub fn item(code: impl Into<String>, group: Arc<String>, value: Option<Object>) -> Object {
    Arc::new(Item {
        code: code.into(),
        state: Mutex::new(State {
            group: Some(group),
            value,
            long: 0,
        }),
    })
}
pub struct Problem {
    pub entities: Vec<Object>,
    pub facts: Vec<Object>,
    pub entity_groups: Vec<Arc<String>>,
    pub fact_groups: Vec<Arc<String>>,
}
impl Problem {
    pub fn generated() -> Self {
        let fact_groups: Vec<_> = (0..2)
            .map(|i| Arc::new(format!("Generated ValueGroup {i}")))
            .collect();
        let facts: Vec<_> = (0..5)
            .map(|i| {
                item(
                    format!("Generated Value {i}"),
                    fact_groups[i % 2].clone(),
                    None,
                )
            })
            .collect();
        let entity_groups: Vec<_> = (0..3)
            .map(|i| Arc::new(format!("Generated EntityGroup {i}")))
            .collect();
        let entities = (0..7)
            .map(|i| {
                item(
                    format!("Generated Entity {i}"),
                    entity_groups[i % 3].clone(),
                    Some(facts[i % 5].clone()),
                )
            })
            .collect();
        Self {
            entities,
            facts,
            entity_groups,
            fact_groups,
        }
    }
    pub fn pool(&self, kind: ProblemObjectKind) -> &[Object] {
        match kind {
            ProblemObjectKind::Entity => &self.entities,
            ProblemObjectKind::Fact => &self.facts,
        }
    }
    pub fn score(&self) -> i64 {
        self.entities
            .iter()
            .map(|e| {
                let s = e.state.lock().unwrap();
                i64::from(s.group.is_some()) + i64::from(s.value.is_some())
            })
            .sum::<i64>()
            + self.facts.len() as i64
    }
}
/// Explicit caller bindings retain both actual objects, never guessed names/IDs.
/// Resolve fresh against current membership on every edit; indices may shift.
#[derive(Clone, Default)]
pub struct Bindings {
    pub rows: Vec<(ProblemObjectKind, Object, Object)>,
}
impl Bindings {
    pub fn bind(&mut self, kind: ProblemObjectKind, external: Object, canonical: Object) {
        self.rows.push((kind, external, canonical));
    }
    pub fn lookup(
        &self,
        p: &Problem,
        kind: ProblemObjectKind,
        external: &Object,
    ) -> ProblemLookup<usize> {
        let mut bindings = self
            .rows
            .iter()
            .filter(|(k, e, _)| *k == kind && Arc::ptr_eq(e, external));
        let canonical = if let Some((_, _, canonical)) = bindings.next() {
            if bindings.next().is_some() {
                return ProblemLookup::Duplicate;
            }
            canonical
        } else {
            external
        };
        let mut matches = p
            .pool(kind)
            .iter()
            .enumerate()
            .filter(|(_, e)| Arc::ptr_eq(e, canonical))
            .map(|(i, _)| i);
        match (matches.next(), matches.next()) {
            (None, _) => ProblemLookup::Missing,
            (Some(i), None) => ProblemLookup::Unique(i),
            _ => ProblemLookup::Duplicate,
        }
    }
}
#[derive(Default)]
pub struct Calculator {
    pub notifications: Vec<bool>,
    pub resets: usize,
    pub flushes: usize,
    pub scores: usize,
    pub cached: i64,
    pub old: i64,
}
impl IncrementalCalculator<Problem> for Calculator {
    type Score = SimpleScore;
    type Justification = ();
    type Change = ProblemEdit<Object, &'static str>;
    fn reset(
        &mut self,
        p: &Problem,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.resets += 1;
        self.cached = p.score();
        Ok(())
    }
    fn before_change(
        &mut self,
        p: &Problem,
        _: &Self::Change,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.old = p.score();
        self.notifications.push(true);
        Ok(())
    }
    fn after_change(
        &mut self,
        p: &Problem,
        _: &Self::Change,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.cached += p.score() - self.old;
        self.notifications.push(false);
        Ok(())
    }
    fn flush_changes(
        &mut self,
        _: &mut Problem,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.flushes += 1;
        Ok(())
    }
    fn calculate_score(&mut self) -> SimpleScore {
        self.scores += 1;
        SimpleScore::new(self.cached)
    }
}
pub type Session = CalculatorSession<Problem, Calculator>;
pub fn session(p: Problem) -> Session {
    CalculatorSession::new(p, Calculator::default(), false).unwrap()
}
