//! Per-step selection of scored candidates, with optional early stopping.
//!
//! Accepted candidates outrank rejected fallback candidates and use the configured
//! ranking. Rejected fallback maximizes score. Random ties retain every tied finalist;
//! saved candidates are selected without evaluating or replaying them again.
use crate::{AdditiveScore, MoveSelection, RejectedMovePolicy, Score, ScoreLevel};
use std::{cmp::Ordering, convert::Infallible, error::Error, fmt, num::NonZeroU64};

/// First-tie ordering retains one candidate. Random ties use native seeded randomness,
/// independent of move ordering and acceptance randomness; no Java PRNG parity.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TieBreaking {
    #[default]
    First,
    Random {
        seed: u64,
    },
}

/// How accepted candidates rank after acceptance and structural validity.
/// Strategic modes prefer overall improvement over the reference; otherwise,
/// the earliest ordinary level strictly improving its reference wins, with
/// lexicographic score breaking ties. Rejected fallback keeps highest-score order.
/// This total ordering intentionally differs from Community 2.7.0's observed
/// encounter-order behavior when a candidate equals the reference exactly.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum FinalistRanking {
    #[default]
    HighestScore,
    StrategicOscillationLastStep,
    StrategicOscillationBest,
}

/// References and limits for one neighborhood scan. A new forager starts a new
/// accepted-candidate count. Phase-wide accepted-move limits are separate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ForagerConfig<S> {
    pub selection: MoveSelection,
    pub ranking: FinalistRanking,
    pub current_score: S,
    pub best_score: S,
    pub accepted_candidate_limit: Option<NonZeroU64>,
    pub tie_breaking: TieBreaking,
    pub rejected_moves: RejectedMovePolicy,
}
impl<S: AdditiveScore> Default for ForagerConfig<S> {
    fn default() -> Self {
        Self {
            selection: MoveSelection::BestAccepted,
            ranking: FinalistRanking::HighestScore,
            current_score: S::ZERO,
            best_score: S::ZERO,
            accepted_candidate_limit: None,
            tie_breaking: TieBreaking::First,
            rejected_moves: RejectedMovePolicy::Stop,
        }
    }
}
// Preserve the construction scorers' existing fallback and custom early-pick
// callbacks while sharing the same candidate retention implementation.
impl<S: AdditiveScore> From<bool> for ForagerConfig<S> {
    fn from(allow_rejected: bool) -> Self {
        Self {
            rejected_moves: if allow_rejected {
                RejectedMovePolicy::BestScore
            } else {
                RejectedMovePolicy::Stop
            },
            ..Self::default()
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidFinalistIndex {
    pub index: usize,
    pub finalist_count: usize,
}
impl fmt::Display for InvalidFinalistIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "finalist index {} is outside 0..{}",
            self.index, self.finalist_count
        )
    }
}
impl Error for InvalidFinalistIndex {}

/// Retains one best candidate plus additional equal finalists only for random
/// ties. Candidate payloads need not implement Clone. Discarding the forager drops
/// all saved candidates; it never executes a move or a scoring callback.
///
/// ```
/// use timetuck::{ForagerConfig, MoveForager, SimpleScore, TieBreaking};
/// let mut candidates = MoveForager::new(ForagerConfig {
///     tie_breaking: TieBreaking::Random { seed: 42 },
///     ..ForagerConfig::default()
/// });
/// candidates.add(SimpleScore::new(3), true, "first");
/// candidates.add(SimpleScore::new(3), true, "second");
/// assert_eq!(candidates.pick_with(|count| { assert_eq!(count, 2); 1 }), Ok(Some("second")));
/// ```
pub struct MoveForager<C, S> {
    config: ForagerConfig<S>,
    best: Option<(bool, S, C)>,
    ties: Vec<C>,
    accepted: u128,
    quit: bool,
}
impl<C, S: Score> MoveForager<C, S> {
    pub fn new(config: ForagerConfig<S>) -> Self {
        Self {
            config,
            best: None,
            ties: Vec::new(),
            accepted: 0,
            quit: false,
        }
    }
    fn compare_scores(&self, a: &S, b: &S, accepted: bool) -> Ordering {
        let reference = match self.config.ranking {
            FinalistRanking::HighestScore => return a.cmp(b),
            FinalistRanking::StrategicOscillationLastStep => &self.config.current_score,
            FinalistRanking::StrategicOscillationBest => &self.config.best_score,
        };
        if !accepted {
            return a.cmp(b);
        }
        a.structural().cmp(&b.structural()).then_with(|| {
            let improving_a = a > reference;
            let improving_b = b > reference;
            improving_a.cmp(&improving_b).then_with(|| {
                if improving_a {
                    return a.cmp(b);
                }
                let first_improvement = |score: &S| {
                    score
                        .ordinary_levels()
                        .zip(reference.ordinary_levels())
                        .position(|(level, baseline)| level.numeric_cmp(&baseline).is_gt())
                        .unwrap_or(usize::MAX)
                };
                first_improvement(b)
                    .cmp(&first_improvement(a))
                    .then_with(|| a.cmp(b))
            })
        })
    }
    pub fn accepted_candidates(&self) -> u128 {
        self.accepted
    }
    pub fn finalist_count(&self) -> usize {
        usize::from(self.best.is_some()) + self.ties.len()
    }
    pub fn is_quit_early(&self) -> bool {
        self.quit
    }

    pub fn add(&mut self, score: S, accepted: bool, candidate: C) {
        let result: Result<(), Infallible> = self.try_add_with(score, accepted, || Ok(candidate));
        match result {
            Ok(()) => (),
            Err(never) => match never {},
        }
    }

    /// Save a candidate only when it can be selected. A failed or panicking save
    /// leaves selection, counts and early-stop state unchanged. Rejected structurally
    /// flawed scores never enter the fallback pool. Callers stop after is_quit_early.
    pub fn try_add_with<E>(
        &mut self,
        score: S,
        accepted: bool,
        save: impl FnOnce() -> Result<C, E>,
    ) -> Result<(), E> {
        if !accepted
            && (self.config.rejected_moves == RejectedMovePolicy::Stop || score.structural() < 0)
        {
            return Ok(());
        }
        let count = self.accepted.saturating_add(u128::from(accepted));
        let pick_early = accepted
            && match self.config.selection {
                MoveSelection::FirstAccepted => true,
                MoveSelection::BestAccepted => false,
                MoveSelection::FirstBestScoreImproving => score > self.config.best_score,
                MoveSelection::FirstLastStepScoreImproving => score > self.config.current_score,
            };
        let early = pick_early
            || (accepted
                && self
                    .config
                    .accepted_candidate_limit
                    .is_some_and(|limit| count >= u128::from(limit.get())));
        let rank = self
            .best
            .as_ref()
            .map_or(Ordering::Greater, |(old_accepted, old_score, _)| {
                if pick_early {
                    Ordering::Greater
                } else {
                    accepted
                        .cmp(old_accepted)
                        .then_with(|| self.compare_scores(&score, old_score, accepted))
                }
            });
        match rank {
            Ordering::Greater => {
                let candidate = save()?;
                self.best = Some((accepted, score, candidate));
                self.ties.clear();
            }
            Ordering::Equal if matches!(self.config.tie_breaking, TieBreaking::Random { .. }) => {
                self.ties.push(save()?);
            }
            _ => (),
        }
        self.accepted = count;
        self.quit |= early;
        Ok(())
    }

    /// Select using native seeded randomness (or first-tie ordering).
    pub fn pick(self) -> Option<C> {
        let seed = match self.config.tie_breaking {
            TieBreaking::First => 0,
            TieBreaking::Random { seed } => seed,
        };
        self.pick_with(|count| crate::random::SplitMix64::new(seed).below(count as u64) as usize)
            .expect("bounded native finalist draw")
    }

    /// Supply a zero-based index into the equal finalists in encounter order.
    /// The supplier runs once only for a random tie of two or more finalists.
    /// Invalid indices reject without executing any candidate.
    pub fn pick_with(
        mut self,
        choose: impl FnOnce(usize) -> usize,
    ) -> Result<Option<C>, InvalidFinalistIndex> {
        let count = self.finalist_count();
        let index = if count > 1 && matches!(self.config.tie_breaking, TieBreaking::Random { .. }) {
            choose(count)
        } else {
            0
        };
        if index >= count && count > 0 {
            return Err(InvalidFinalistIndex {
                index,
                finalist_count: count,
            });
        }
        Ok(if index == 0 {
            self.best.map(|(_, _, candidate)| candidate)
        } else {
            Some(self.ties.swap_remove(index - 1))
        })
    }
}
