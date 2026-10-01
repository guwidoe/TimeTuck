//! Tabu memory keyed by stable identities, with tenure measured in completed steps.
use crate::{AcceptanceError, Score};

/// Retains keys touched in the most recent `tenure` completed steps. All keys in
/// one step receive the same lifetime; tenure is not a cap on the number of keys.
/// Equality/identity must remain stable while a key is retained.
#[derive(Clone, Debug)]
pub struct TabuMemory<K> {
    tenure: usize,
    aspiration: bool,
    remaining: Vec<(K, usize)>,
}
impl<K: Eq + Clone> TabuMemory<K> {
    pub fn new(tenure: usize, aspiration: bool) -> Result<Self, AcceptanceError> {
        if tenure == 0 {
            return Err(AcceptanceError::EmptyTabu);
        }
        Ok(Self {
            tenure,
            aspiration,
            remaining: Vec::new(),
        })
    }
    pub fn tenure(&self) -> usize {
        self.tenure
    }
    pub fn is_tabu(&self, key: &K) -> bool {
        self.remaining.iter().any(|(retained, _)| retained == key)
    }
    /// Trial checks never age or refresh memory. Aspiration requires a strict
    /// improvement over the supplied global best, not merely the current score.
    pub fn accepts<S: Score>(&self, keys: &[K], candidate: S, best: S) -> bool {
        (self.aspiration && candidate > best) || !keys.iter().any(|key| self.is_tabu(key))
    }
    /// Advance one completed step and refresh all touched keys. Repeated keys
    /// have one entry. Empty steps age memory; unselected trials must not call it.
    pub fn step_ended(&mut self, keys: &[K]) {
        self.remaining.retain_mut(|(_, remaining)| {
            *remaining -= 1;
            *remaining != 0
        });
        for key in keys {
            if let Some((_, remaining)) = self
                .remaining
                .iter_mut()
                .find(|(retained, _)| retained == key)
            {
                *remaining = self.tenure;
            } else {
                self.remaining.push((key.clone(), self.tenure));
            }
        }
    }
}
