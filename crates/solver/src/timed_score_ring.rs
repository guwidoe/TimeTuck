//! Growable timestamped score storage retaining a sliding-window predecessor.
use std::{collections::VecDeque, error::Error, fmt};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimedScoreRingError {
    Empty,
    InvalidState,
    BackwardsTime,
    Capacity,
}
impl fmt::Display for TimedScoreRingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Empty => "score buffer is empty",
            Self::InvalidState => "invalid timed score ring state",
            Self::BackwardsTime => "score timestamps must be nondecreasing",
            Self::Capacity => "timed score ring capacity cannot be allocated",
        })
    }
}
impl Error for TimedScoreRingError {}

/// Owned physical layout for inspection/checkpoint restoration. Length explicitly
/// distinguishes empty from full when the cursors coincide. The default clock
/// is u128 nanoseconds; any Copy/Ord/Default clock type can be used. Its default
/// value is only vacant-slot padding, never an active-sample sentinel.
/// Inactive slots can retain old payloads in an imported checkpoint. Operations
/// never use those slots as live samples; overwrite, growth and clear release them.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TimedScoreRingState<S, T = u128> {
    pub read: usize,
    pub write: usize,
    pub len: usize,
    pub times: Vec<T>,
    pub scores: Vec<Option<S>>,
}
#[derive(Clone, Debug)]
struct Slot<S, T> {
    time: T,
    score: Option<S>,
}
fn vacant<S, T: Default>() -> Slot<S, T> {
    Slot {
        time: T::default(),
        score: None,
    }
}
/// Chronologically ordered, growable ring, defaulting to u128 timestamps. Native
/// duration clocks can retain their representation without per-query conversion.
/// Insertion is amortized O(1), eviction
/// O(number removed), and storage O(peak live samples). Neither insertion nor
/// growth clones a score. Capacity doubles on overflow; growth linearizes active
/// entries and clears inactive slots. Capacity does not shrink on eviction.
/// Timestamp queries are inclusive, retaining the newest sample at/before the
/// cutoff and every later sample. If the cutoff precedes all samples, the oldest
/// sample is retained. A query beyond all samples keeps the newest sample.
///
/// ```
/// use timetuck::{TimedScoreRing, SimpleScore};
/// let mut history = TimedScoreRing::new(2)?;
/// history.push(0, SimpleScore::ZERO)?;
/// history.push(10, SimpleScore::new(4))?;
/// history.push(20, SimpleScore::new(7))?;
/// assert_eq!(history.retain_floor(15)?, (10, &SimpleScore::new(4)));
/// assert_eq!(history.len(), 2);
/// # Ok::<(), timetuck::TimedScoreRingError>(())
/// ```
#[derive(Clone, Debug)]
pub struct TimedScoreRing<S, T = u128> {
    entries: VecDeque<(T, S)>,
    read: usize,
    capacity: usize,
    // Only imported checkpoints can contain inactive payloads/timestamps.
    // Normal runtime stores active samples only, avoiding an Option per score.
    inactive: Option<Vec<Slot<S, T>>>,
}
impl<S, T: Ord + Copy + Default> TimedScoreRing<S, T> {
    pub fn new(capacity: usize) -> Result<Self, TimedScoreRingError> {
        if capacity == 0 {
            return Err(TimedScoreRingError::Capacity);
        }
        let mut entries = VecDeque::new();
        entries
            .try_reserve_exact(capacity)
            .map_err(|_| TimedScoreRingError::Capacity)?;
        Ok(Self {
            entries,
            read: 0,
            capacity,
            inactive: None,
        })
    }
    /// Validate cursor/length consistency and active score/time ordering before
    /// accepting an owned checkpoint. Inactive payloads are preserved verbatim.
    pub fn from_state(state: TimedScoreRingState<S, T>) -> Result<Self, TimedScoreRingError> {
        let n = state.times.len();
        if n == 0
            || n != state.scores.len()
            || state.read >= n
            || state.write >= n
            || state.len > n
            || (state.read + state.len) % n != state.write
        {
            return Err(TimedScoreRingError::InvalidState);
        }
        let mut previous = None;
        for offset in 0..state.len {
            let i = (state.read + offset) % n;
            if state.scores[i].is_none() || previous.is_some_and(|t| t > state.times[i]) {
                return Err(TimedScoreRingError::InvalidState);
            }
            previous = Some(state.times[i]);
        }
        let mut result = Self::new(n)?;
        let mut slots: Vec<_> = state
            .times
            .into_iter()
            .zip(state.scores)
            .map(|(time, score)| Slot { time, score })
            .collect();
        for offset in 0..state.len {
            let i = (state.read + offset) % n;
            let slot = std::mem::replace(&mut slots[i], vacant());
            result
                .entries
                .push_back((slot.time, slot.score.expect("validated active slot")));
        }
        result.read = state.read;
        if slots
            .iter()
            .any(|s| s.time != T::default() || s.score.is_some())
        {
            result.inactive = Some(slots);
        }
        Ok(result)
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub fn capacity(&self) -> usize {
        self.capacity
    }
    fn index(&self, offset: usize) -> usize {
        let i = self.read + offset;
        if i >= self.capacity {
            i - self.capacity
        } else {
            i
        }
    }
    /// Sample by chronological offset, independent of physical wrap position.
    pub fn get(&self, offset: usize) -> Option<(T, &S)> {
        self.entries.get(offset).map(|(t, s)| (*t, s))
    }
    pub fn front(&self) -> Option<(T, &S)> {
        self.entries.front().map(|(t, s)| (*t, s))
    }
    pub fn back(&self) -> Option<(T, &S)> {
        self.entries.back().map(|(t, s)| (*t, s))
    }
    /// Double checkpoint capacity and linearize active entries in its layout.
    /// Allocation/capacity errors preserve the current state. Rust's deque owns
    /// the samples and moves them on allocation; no score clone is required.
    pub fn grow(&mut self) -> Result<(), TimedScoreRingError> {
        let capacity = self
            .capacity
            .checked_mul(2)
            .ok_or(TimedScoreRingError::Capacity)?;
        self.entries
            .try_reserve_exact(capacity - self.len())
            .map_err(|_| TimedScoreRingError::Capacity)?;
        self.capacity = capacity;
        self.read = 0;
        let previous = self.inactive.take();
        drop(previous);
        Ok(())
    }
    pub fn push(&mut self, time: T, score: S) -> Result<(), TimedScoreRingError> {
        if self.back().is_some_and(|(previous, _)| time < previous) {
            return Err(TimedScoreRingError::BackwardsTime);
        }
        self.push_chronological(time, score)
    }
    // The termination policy already clamps time and inspected the last score.
    pub(crate) fn push_chronological(
        &mut self,
        time: T,
        score: S,
    ) -> Result<(), TimedScoreRingError> {
        if self.len() == self.capacity {
            self.grow()?;
        }
        let write = self.index(self.len());
        let previous = self
            .inactive
            .as_mut()
            .map(|slots| std::mem::replace(&mut slots[write], vacant()));
        self.entries.push_back((time, score));
        drop(previous);
        Ok(())
    }
    /// Replace the last score at its existing timestamp, without changing layout.
    pub fn replace_latest_score(&mut self, score: S) -> Result<(), TimedScoreRingError> {
        let (_, current) = self.entries.back_mut().ok_or(TimedScoreRingError::Empty)?;
        let previous = std::mem::replace(current, score);
        drop(previous);
        Ok(())
    }
    pub fn peek(&self) -> Result<&S, TimedScoreRingError> {
        self.front()
            .map(|(_, s)| s)
            .ok_or(TimedScoreRingError::Empty)
    }
    pub fn retain_floor(&mut self, cutoff: T) -> Result<(T, &S), TimedScoreRingError> {
        if self.is_empty() {
            return Err(TimedScoreRingError::Empty);
        }
        while self.len() > 1 && self.entries[1].0 <= cutoff {
            let previous = self.entries.pop_front();
            self.read = self.index(1);
            drop(previous);
        }
        Ok(self.front().expect("retained sample"))
    }
    pub fn clear(&mut self) {
        while let Some(previous) = self.entries.pop_front() {
            self.read = self.index(1);
            drop(previous);
        }
        self.read = 0;
        let previous = self.inactive.take();
        drop(previous);
    }
    /// Materialize the physical checkpoint layout on demand. Runtime storage is
    /// a standard deque of active samples; inactive slots cost storage only when
    /// imported. Cursor/slot semantics do not depend on the deque's allocation.
    pub fn state(&self) -> TimedScoreRingState<S, T>
    where
        S: Clone,
    {
        let mut state = TimedScoreRingState {
            read: self.read,
            write: self.index(self.len()),
            len: self.len(),
            times: vec![T::default(); self.capacity],
            scores: std::iter::repeat_with(|| None)
                .take(self.capacity)
                .collect(),
        };
        if let Some(slots) = &self.inactive {
            for (i, slot) in slots.iter().enumerate() {
                state.times[i] = slot.time;
                state.scores[i] = slot.score.clone();
            }
        }
        for (offset, (time, score)) in self.entries.iter().enumerate() {
            let i = self.index(offset);
            state.times[i] = *time;
            state.scores[i] = Some(score.clone());
        }
        state
    }
}
