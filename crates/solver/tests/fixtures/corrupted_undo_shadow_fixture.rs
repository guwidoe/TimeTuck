// Adapted from Community TEST helpers CorruptedUndoShadow{Entity,Value,Solution,
// EasyScoreCalculator}. Apache-2.0; see compat/provenance.json.
#![allow(dead_code)]
use timetuck::*;
#[derive(Clone)]
pub struct CorruptedUndoShadowEntity {
    pub id: String,
    pub value: Option<i64>,
    pub value_clone: Option<i64>,
}
#[derive(Clone)]
pub struct CorruptedUndoShadowValue {
    pub value: i64,
    pub entities: Vec<usize>,
}
#[derive(Clone)]
pub struct CorruptedUndoShadowSolution {
    pub entities: Vec<CorruptedUndoShadowEntity>,
    pub values: Vec<CorruptedUndoShadowValue>,
}
#[derive(Clone)]
pub struct Calculator {
    pub solution: CorruptedUndoShadowSolution,
    pub policy: CalculatorUndoDiagnostics,
    pub updates: usize,
}
pub fn name(value: Option<i64>) -> String {
    match value {
        Some(0) => "v1".into(),
        Some(1) => "v2".into(),
        None => "null".into(),
        _ => unreachable!(),
    }
}
impl Calculator {
    pub fn new(mode: EnvironmentMode) -> Self {
        Self {
            solution: CorruptedUndoShadowSolution {
                entities: vec![
                    CorruptedUndoShadowEntity {
                        id: "e1".into(),
                        value: Some(0),
                        value_clone: Some(0),
                    },
                    CorruptedUndoShadowEntity {
                        id: "e2".into(),
                        value: Some(1),
                        value_clone: Some(1),
                    },
                ],
                values: vec![
                    CorruptedUndoShadowValue {
                        value: 0,
                        entities: vec![0],
                    },
                    CorruptedUndoShadowValue {
                        value: 1,
                        entities: vec![1],
                    },
                ],
            },
            policy: CalculatorUndoDiagnostics::for_environment_mode(mode),
            updates: 0,
        }
    }
    fn easy_score(&self) -> SimpleScore {
        SimpleScore::new(
            self.solution
                .entities
                .iter()
                .filter(|e| e.value == e.value_clone)
                .count() as i64,
        )
    }
}
impl IncrementalCalculator<Assignment<i64>> for Calculator {
    type Score = SimpleScore;
    type Justification = ();
    type Change = Vec<usize>;
    fn undo_diagnostics(&self) -> CalculatorUndoDiagnostics {
        self.policy
    }
    fn reset(
        &mut self,
        assignment: &Assignment<i64>,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        for v in &mut self.solution.values {
            v.entities.clear();
        }
        for (i, e) in self.solution.entities.iter_mut().enumerate() {
            e.value = assignment.values()[i];
            e.value_clone = e.value;
            if let Some(v) = e.value {
                self.solution.values[v as usize].entities.push(i);
            }
        }
        Ok(())
    }
    fn after_change(
        &mut self,
        assignment: &Assignment<i64>,
        changed: &Vec<usize>,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        for &i in changed {
            let e = &mut self.solution.entities[i];
            let next = assignment.values()[i];
            if e.value == next {
                continue;
            }
            if let Some(old) = e.value {
                self.solution.values[old as usize]
                    .entities
                    .retain(|&entity| entity != i);
            }
            e.value = next;
            if let Some(v) = next {
                self.solution.values[v as usize].entities.push(i);
            }
            // Preserve the TEST supplier's bug: undoing to v1 retains its clone.
            if e.value_clone.is_none() || next != Some(0) {
                e.value_clone = next;
            }
            self.updates += 1;
        }
        Ok(())
    }
    fn calculate_score(&mut self) -> SimpleScore {
        self.easy_score()
    }
    fn recompute_score_from(
        &mut self,
        working: &Self,
    ) -> Result<Option<SimpleScore>, MatchRegistryError> {
        // Easy score on the actual entities/shadows, without invoking suppliers.
        Ok(Some(working.easy_score()))
    }
    fn capture_undo_state(&self) -> Result<Option<CalculatorStateSnapshot>, MatchRegistryError> {
        let mut s = CalculatorStateSnapshot::new();
        for e in &self.solution.entities {
            for (variable, value) in [("value", e.value), ("valueClone", e.value_clone)] {
                s.capture(
                    CalculatorUndoField {
                        entity_key: e.id.clone(),
                        entity_type: "CorruptedUndoShadowEntity".into(),
                        entity_description: "CorruptedUndoShadowEntity".into(),
                        variable: variable.into(),
                    },
                    value,
                    name(value),
                )?;
            }
        }
        for v in &self.solution.values {
            let mut members = v.entities.clone();
            members.sort_unstable();
            s.capture(
                CalculatorUndoField {
                    entity_key: name(Some(v.value)),
                    entity_type: "CorruptedUndoShadowValue".into(),
                    entity_description: name(Some(v.value)),
                    variable: "entities".into(),
                },
                members.clone(),
                format!("{members:?}"),
            )?;
        }
        Ok(Some(s))
    }
}
