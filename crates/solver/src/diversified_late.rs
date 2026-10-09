//! Diversified late acceptance, independently implemented from public rules.
use crate::{AcceptanceError, Score};

/// A circular history with acceptance against its worst score. Equality with
/// the current score is also accepted. Accepted trials replace the active slot
/// when worse than that slot, or when better than both that slot and current.
/// Rejected trials leave history unchanged. Completed steps only advance the
/// cursor, including rejected fallbacks; they do not overwrite a slot.
///
/// Every queried child in a composite policy keeps its trial updates even if a
/// later child rejects. History starts afresh per phase. Scores use their full
/// ordering; there is no hard-level history reset.
#[derive(Clone, Debug)]
pub struct DiversifiedLateAcceptance<S> {
    history: Vec<S>,
    cursor: usize,
    worst: S,
    occurrences: usize,
}
impl<S: Score> DiversifiedLateAcceptance<S> {
    pub fn new(history_size: usize, initial: S) -> Result<Self, AcceptanceError> {
        if history_size == 0 {
            return Err(AcceptanceError::EmptyHistory);
        }
        let mut history = Vec::new();
        history
            .try_reserve_exact(history_size)
            .map_err(|_| AcceptanceError::HistoryAllocation)?;
        history.resize(history_size, initial.clone());
        Ok(Self {
            history,
            cursor: 0,
            worst: initial,
            occurrences: history_size,
        })
    }
    pub fn history(&self) -> &[S] {
        &self.history
    }
    pub fn cursor(&self) -> usize {
        self.cursor
    }
    pub fn worst_score(&self) -> &S {
        &self.worst
    }
    pub fn worst_occurrences(&self) -> usize {
        self.occurrences
    }
    pub fn accepts(&mut self, current: S, candidate: S) -> bool {
        if candidate != current && candidate <= self.worst {
            return false;
        }
        let slot = &self.history[self.cursor];
        if candidate < *slot || (candidate > *slot && candidate > current) {
            // Only removing the last minimum requires a scan. Prepare every
            // comparison and fallible score clone before publishing the slot.
            let (worst, occurrences) = match candidate.cmp(&self.worst) {
                std::cmp::Ordering::Less => (Some(candidate.clone()), 1),
                std::cmp::Ordering::Equal => {
                    (None, self.occurrences + usize::from(*slot != self.worst))
                }
                std::cmp::Ordering::Greater => {
                    let removes_minimum = *slot == self.worst;
                    if !removes_minimum || self.occurrences > 1 {
                        (None, self.occurrences - usize::from(removes_minimum))
                    } else {
                        let mut minimum = &candidate;
                        let mut count = 1;
                        for (i, score) in self.history.iter().enumerate() {
                            if i == self.cursor {
                                continue;
                            }
                            match score.cmp(minimum) {
                                std::cmp::Ordering::Less => {
                                    minimum = score;
                                    count = 1;
                                }
                                std::cmp::Ordering::Equal => count += 1,
                                std::cmp::Ordering::Greater => {}
                            }
                        }
                        (Some(minimum.clone()), count)
                    }
                }
            };
            self.history[self.cursor] = candidate;
            if let Some(worst) = worst {
                self.worst = worst;
            }
            self.occurrences = occurrences;
        }
        true
    }
    pub fn step_ended(&mut self) {
        self.cursor = if self.cursor + 1 == self.history.len() {
            0
        } else {
            self.cursor + 1
        };
    }
}

#[cfg(test)]
mod cases {
    // Adapted Apache-2.0 pinned Community DiversifiedLateAcceptanceAcceptorTest.
    // Native scores and private state replace Java scopes and InnerScore wrappers.
    use super::*;
    use crate::SimpleScore as S;
    #[test]
    fn acceptance_criterion() {
        let mut a = DiversifiedLateAcceptance::new(3, S::new(-1000)).unwrap();
        assert!(a.accepts(S::new(-1000), S::new(-1000)));
        assert!(a.accepts(S::new(-1000), S::new(-999)));
        a = DiversifiedLateAcceptance::new(3, S::new(-1000)).unwrap();
        a.worst = S::new(-2001);
        a.occurrences = 1;
        a.history = vec![S::new(-2001), S::new(-2001), S::new(-2000)];
        a.accepts(S::new(-2001), S::new(-2000));
        assert_eq!(a.history[0], S::new(-2000));
        assert_eq!(a.worst, S::new(-2001));
        assert_eq!(a.occurrences, 1);
    }
    #[test]
    fn replacement_criterion() {
        for (candidate, late, current, expected) in [
            (-2000, -1999, -1998, -2000),
            (-2001, -1999, -2000, -2001),
            (-2001, -1999, -2001, -2001),
            (-1998, -2000, -1999, -1998),
            (-1998, -1999, -2000, -1998),
            (-2000, -1999, -2001, -2000),
            (-2000, -2001, -1999, -2001),
            (-2000, -2001, -2000, -2001),
        ] {
            let mut a = DiversifiedLateAcceptance::new(3, S::new(-1000)).unwrap();
            a.worst = S::new(-2005);
            a.history[0] = S::new(late);
            a.accepts(S::new(current), S::new(candidate));
            assert_eq!(a.history[0], S::new(expected));
        }
    }
}
