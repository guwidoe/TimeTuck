// TEST-derived fixtures, Apache-2.0; see compat/provenance.json.
use timetuck::*;
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Config {
    Pooled {
        moves: Vec<String>,
    },
    Entity {
        entities: Vec<String>,
        primary: Vec<String>,
        secondary: Vec<String>,
    },
    Value {
        entities: Vec<String>,
        values: Vec<String>,
    },
}
pub struct Keys {
    pub values: Vec<String>,
    pub events: Vec<SelectionLifecycleEvent>,
}
impl SelectionSource<(), String> for Keys {
    fn event(&mut self, e: SelectionLifecycleEvent, _: &()) {
        self.events.push(e);
    }
    fn size(&mut self, _: &()) -> Option<usize> {
        Some(self.values.len())
    }
    fn open<'a>(&'a mut self, _: &'a ()) -> Box<dyn Iterator<Item = String> + 'a>
    where
        String: 'a,
    {
        Box::new(self.values.iter().cloned())
    }
}
pub struct Pool {
    pub config: Config,
    pub events: Vec<SelectionLifecycleEvent>,
    pub opens: Vec<String>,
}
impl PlacementSource<(), String, String> for Pool {
    fn event(&mut self, e: SelectionLifecycleEvent, _: &()) {
        self.events.push(e);
    }
    fn size(&mut self, _: &(), _: &String) -> Option<usize> {
        Some(match &self.config {
            Config::Pooled { moves } => moves.len(),
            Config::Entity {
                primary, secondary, ..
            } => primary.len() * secondary.len(),
            Config::Value { entities, .. } => entities.len(),
        })
    }
    fn open<'a>(&'a mut self, _: &'a (), key: &'a String) -> Box<dyn Iterator<Item = String> + 'a>
    where
        String: 'a,
    {
        self.opens.push(key.clone());
        match &self.config {
            Config::Pooled { moves } => Box::new(moves.iter().cloned()),
            Config::Entity {
                primary, secondary, ..
            } => Box::new(primary.iter().flat_map(move |a| {
                secondary
                    .iter()
                    .map(move |b| format!("{key}->{a}+{key}->{b}"))
            })),
            Config::Value { entities, .. } => {
                Box::new(entities.iter().map(move |e| format!("{e}->{key}")))
            }
        }
    }
}
pub type Queue = PlacementQueue<(), String, String, Keys, Pool>;
pub fn queue(c: &Config) -> Result<Queue, PlacementError> {
    let values = match c {
        Config::Pooled { .. } => vec![String::new()],
        Config::Entity { entities, .. } => entities.clone(),
        Config::Value { values, .. } => values.clone(),
    };
    PlacementQueue::new(
        Keys {
            values,
            events: vec![],
        },
        vec![Pool {
            config: c.clone(),
            events: vec![],
            opens: vec![],
        }],
        PlacementOrder::Once,
        100,
        100,
    )
}
pub fn open(q: &mut Queue) {
    q.event(SelectionLifecycleEvent::SolveStarted, &()).unwrap();
    q.event(SelectionLifecycleEvent::PhaseStarted, &()).unwrap();
}
pub fn next(q: &mut Queue) -> Vec<String> {
    q.event(SelectionLifecycleEvent::StepStarted, &()).unwrap();
    let m = q.next_placement(&()).unwrap().unwrap().moves().to_vec();
    q.event(SelectionLifecycleEvent::StepEnded, &()).unwrap();
    m
}
