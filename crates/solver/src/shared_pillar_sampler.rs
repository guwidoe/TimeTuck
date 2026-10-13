//! Stateful, shareable pillar sampling with complete owned sample publication.
use crate::{
    Assignment, FiniteProblem, ModelError, Move, MoveProvider, Pillar, PillarSelectionPolicy,
    pillar_groups,
};
use std::{cell::RefCell, rc::Rc};

/// A reusable sampler session. Starting any sample resets unfinished state,
/// stages an actual policy draw, and resets its traversal cursor. Finishing
/// transfers the complete owned pillar, including members already observed via
/// next_member. Sharing is safe when each sample finishes before the next begins.
/// Native sampling uses existing pillar policy draws; Java RNG traces are not
/// implied. This session stores only one selected sample, never prior samples.
#[derive(Debug)]
pub struct StatefulPillarSampler {
    policy: PillarSelectionPolicy,
    pending: Option<Pillar>,
    cursor: usize,
    resets: u64,
    completed: u64,
}
impl StatefulPillarSampler {
    pub fn new(policy: PillarSelectionPolicy) -> Self {
        Self {
            policy,
            pending: None,
            cursor: 0,
            resets: 0,
            completed: 0,
        }
    }
    pub fn shared(policy: PillarSelectionPolicy) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self::new(policy)))
    }
    pub fn policy(&self) -> &PillarSelectionPolicy {
        &self.policy
    }
    pub fn reset(&mut self) {
        self.pending = None;
        self.cursor = 0;
        self.resets = self.resets.saturating_add(1);
    }
    pub fn reset_count(&self) -> u64 {
        self.resets
    }
    pub fn completed_count(&self) -> u64 {
        self.completed
    }
    pub fn pending_rows(&self) -> usize {
        self.pending.as_ref().map_or(0, |p| p.rows().len())
    }
    pub fn remaining_rows(&self) -> usize {
        self.pending_rows().saturating_sub(self.cursor)
    }
    pub fn begin(&mut self, pillar: &Pillar, seed: u64) -> bool {
        self.begin_random(pillar, &mut crate::random::SplitMix64::new(seed))
    }
    fn begin_random(&mut self, pillar: &Pillar, random: &mut crate::random::SplitMix64) -> bool {
        self.reset();
        self.pending = self
            .policy
            .prepare_samples(pillar)
            .map(|prepared| prepared.draw(random));
        self.pending.is_some()
    }
    pub fn next_member(&mut self) -> Option<Vec<usize>> {
        let row = self.pending.as_ref()?.rows().get(self.cursor)?.clone();
        self.cursor += 1;
        Some(row)
    }
    pub fn finish(&mut self) -> Option<Pillar> {
        let result = self.pending.take()?;
        self.cursor = 0;
        self.completed = self.completed.saturating_add(1);
        Some(result)
    }
    fn sample_random(
        &mut self,
        pillar: &Pillar,
        random: &mut crate::random::SplitMix64,
    ) -> Option<Pillar> {
        self.begin_random(pillar, random);
        self.finish()
    }
}
/// Optional shared-state swap provider. Both sides use the caller's actual
/// sampler handles; the left sample is completely owned before right resets.
/// Each opening regroups current facts/pins and reseeds its one working random
/// stream with seed+step. Distinct eligible groups are selected directly; invalid
/// receiver ranges consume attempts. Member selection stays with the respective
/// sampler policy, so separate left/right rules remain independent. Metadata is
/// bounded by current groups and one staged sample per distinct sampler; no move
/// catalog or combination enumeration occurs. Existing search providers unchanged.
#[derive(Clone, Debug)]
pub struct SharedSamplerSubPillarSwaps {
    pub left: Rc<RefCell<StatefulPillarSampler>>,
    pub right: Rc<RefCell<StatefulPillarSampler>>,
    pub seed: u64,
    pub attempts_per_step: u64,
}
impl SharedSamplerSubPillarSwaps {
    pub fn select<'a, V: Eq + Clone>(
        &'a self,
        p: &'a FiniteProblem<V>,
        a: &'a Assignment<V>,
        step: u64,
    ) -> Result<Box<dyn Iterator<Item = Move<V>> + 'a>, ModelError> {
        p.validate(a)?;
        let groups = pillar_groups(p, a)?;
        let eligible = |sampler: &Rc<RefCell<StatefulPillarSampler>>| {
            let minimum = sampler.borrow().policy.sizes().map_or(1, |s| s.0);
            groups
                .iter()
                .enumerate()
                .filter_map(|(i, g)| (g.rows().len() >= minimum).then_some(i))
                .collect::<Vec<_>>()
        };
        let left = eligible(&self.left);
        let right = eligible(&self.right);
        let mut random = crate::random::SplitMix64::new(self.seed.wrapping_add(step));
        let attempts = if left.is_empty() || right.is_empty() {
            0
        } else {
            self.attempts_per_step
        };
        Ok(Box::new((0..attempts).filter_map(move |_| {
            let l = left[random.below(left.len() as u64) as usize];
            let excluded = right.binary_search(&l).ok();
            let eligible_right = right.len() - usize::from(excluded.is_some());
            if eligible_right == 0 {
                return None;
            }
            let rank = random.below(eligible_right as u64) as usize;
            let r = right[rank + usize::from(excluded.is_some_and(|index| rank >= index))];
            let left = self
                .left
                .borrow_mut()
                .sample_random(&groups[l], &mut random)?;
            let right = self
                .right
                .borrow_mut()
                .sample_random(&groups[r], &mut random)?;
            let accepts = |to: &Pillar, from: &Pillar| {
                to.rows().iter().all(|row| {
                    row.iter().zip(&from.rows()[0]).all(|(&to, &from)| {
                        crate::pillar_schema::accepts(p, to, a.values()[from].as_ref())
                    })
                })
            };
            if !accepts(&left, &right) || !accepts(&right, &left) {
                return None;
            }
            Some(
                left.swap_move(&right, a)
                    .expect("current disjoint uniform groups"),
            )
        })))
    }
}
impl<V: Eq + Clone> MoveProvider<FiniteProblem<V>, Assignment<V>, Move<V>>
    for SharedSamplerSubPillarSwaps
{
    fn moves<'a>(
        &'a mut self,
        p: &'a FiniteProblem<V>,
        a: &'a Assignment<V>,
        step: u64,
    ) -> Box<dyn Iterator<Item = Move<V>> + 'a>
    where
        Move<V>: 'a,
    {
        self.select(p, a, step)
            .expect("valid shared sampler assignment")
    }
}
