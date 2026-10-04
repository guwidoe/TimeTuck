//! Acceptance with typed, structurally compared move identities.
use crate::{Acceptance, AcceptanceError, Acceptor, FadingTabuMemory, Score, TabuMemory};

/// A fresh acceptance session receiving both written variable/entity keys and
/// the complete move descriptor. Retained moves require only stable Eq + Clone;
/// no hashing, pointer identity or debug-string conversion is used.
///
/// Native solver moves use their existing structural equality: swap orientation,
/// list positions and composite ordering remain significant. Equivalent successor
/// states or inverse moves are not automatically equal descriptors. This is not
/// a claim of matching Java move equality or Java random traces.
///
/// ```
/// use timetuck::{Acceptance, Move, SimpleScore};
/// let mut state = Acceptance::MoveTabu { tenure: 2, aspiration: false }
///     .start_with_moves(SimpleScore::ZERO)?;
/// let first = Move::Change { variable: 0, value: Some("a") };
/// let second = Move::Change { variable: 0, value: Some("b") };
/// state.step_ended(SimpleScore::ZERO, &[0], &first);
/// assert!(!state.accepts(SimpleScore::ZERO, SimpleScore::ZERO, &[0], &first));
/// assert!(state.accepts(SimpleScore::ZERO, SimpleScore::ZERO, &[0], &second));
/// # Ok::<(), timetuck::AcceptanceError>(())
/// ```
#[derive(Clone, Debug)]
pub struct MoveAcceptor<S, M> {
    state: ContextAcceptor<S, M, ()>,
}
/// Acceptance with explicit planning-value keys in addition to move descriptors.
/// Values must use stable Eq identities; Option keys can track unassignment.
/// Each child policy receives its own kind of identity. Trials short-circuit;
/// every completed step updates all children, including rejected fallbacks.
///
/// ```
/// use timetuck::{Acceptance, SimpleScore};
/// let z = SimpleScore::ZERO;
/// let mut state = Acceptance::ValueTabu { tenure: 2, aspiration: false }
///     .start_with_values(z)?;
/// state.step_ended(z, &[0], &"change", &[None::<i32>]);
/// assert!(!state.accepts(z, z, &[1], &"another move", &[None]));
/// assert!(state.accepts(z, z, &[0], &"change", &[Some(3)]));
/// # Ok::<(), timetuck::AcceptanceError>(())
/// ```
#[derive(Clone, Debug)]
pub struct ValueAcceptor<S, M, K> {
    state: ContextAcceptor<S, M, K>,
}
#[derive(Clone, Debug)]
struct ContextAcceptor<S, M, K> {
    state: State<S, M, K>,
}
#[derive(Clone, Debug)]
enum State<S, M, K> {
    Ordinary(Acceptor<S>),
    Tabu {
        memory: TabuMemory<M>,
        best: S,
    },
    Fading {
        memory: FadingTabuMemory<M>,
        best: S,
    },
    Values {
        memory: TabuMemory<K>,
        best: S,
    },
    FadingValues {
        memory: FadingTabuMemory<K>,
        best: S,
    },
    All(Vec<ContextAcceptor<S, M, K>>),
}
impl<S: Score, M: Eq + Clone> MoveAcceptor<S, M> {
    pub(crate) fn new(p: &Acceptance<S>, initial: S) -> Result<Self, AcceptanceError> {
        Ok(Self {
            state: ContextAcceptor::new(p, initial, false)?,
        })
    }
    /// Ordered conjunction; all policies validate before constructing state.
    /// Value policies require ValueAcceptor instead. Empty accepts everything.
    pub fn from_policies(p: &[Acceptance<S>], initial: S) -> Result<Self, AcceptanceError> {
        Ok(Self {
            state: ContextAcceptor::start_composite(p.iter(), initial, false)?,
        })
    }
    pub fn accepts(&mut self, current: S, candidate: S, variables: &[usize], movement: &M) -> bool {
        self.state
            .accepts(current, candidate, variables, movement, &[])
    }
    pub fn step_ended(&mut self, score: S, variables: &[usize], movement: &M) {
        self.state.step_ended(score, variables, movement, &[])
    }
}
impl<S: Score, M: Eq + Clone, K: Eq + Clone> ValueAcceptor<S, M, K> {
    pub(crate) fn new(p: &Acceptance<S>, initial: S) -> Result<Self, AcceptanceError> {
        Ok(Self {
            state: ContextAcceptor::new(p, initial, true)?,
        })
    }
    /// Ordered conjunction; all policies validate before constructing state.
    /// Empty accepts everything. Each call creates fresh independent memory.
    pub fn from_policies(p: &[Acceptance<S>], initial: S) -> Result<Self, AcceptanceError> {
        Self::start_composite(p.iter(), initial)
    }
    pub(crate) fn start_composite<'a>(
        p: impl Iterator<Item = &'a Acceptance<S>> + Clone,
        initial: S,
    ) -> Result<Self, AcceptanceError>
    where
        S: 'a,
    {
        Ok(Self {
            state: ContextAcceptor::start_composite(p, initial, true)?,
        })
    }
    /// Values are those assigned/moved by this trial, captured consistently from
    /// its pre-move state. Unselected trials never age or refresh memory.
    pub fn accepts(
        &mut self,
        current: S,
        candidate: S,
        variables: &[usize],
        movement: &M,
        values: &[K],
    ) -> bool {
        self.state
            .accepts(current, candidate, variables, movement, values)
    }
    /// Advance and retain identities of a selected step, including fallback.
    pub fn step_ended(&mut self, score: S, variables: &[usize], movement: &M, values: &[K]) {
        self.state.step_ended(score, variables, movement, values)
    }
}
impl<S: Score, M: Eq + Clone, K: Eq + Clone> ContextAcceptor<S, M, K> {
    fn validate(p: &Acceptance<S>, values: bool) -> Result<(), AcceptanceError> {
        p.validate()?;
        if !values
            && matches!(
                p,
                Acceptance::ValueTabu { .. } | Acceptance::FadingValueTabu { .. }
            )
        {
            return Err(AcceptanceError::ValueContextRequired);
        }
        Ok(())
    }
    fn new(p: &Acceptance<S>, initial: S, values: bool) -> Result<Self, AcceptanceError> {
        Self::validate(p, values)?;
        let state = match p {
            Acceptance::MoveTabu { tenure, aspiration } => State::Tabu {
                memory: TabuMemory::new(*tenure, *aspiration)?,
                best: initial,
            },
            Acceptance::FadingMoveTabu {
                hard_tenure,
                fading_tenure,
                aspiration,
                seed,
            } => State::Fading {
                memory: FadingTabuMemory::new(*hard_tenure, *fading_tenure, *aspiration, *seed)?,
                best: initial,
            },
            Acceptance::ValueTabu { tenure, aspiration } => State::Values {
                memory: TabuMemory::new(*tenure, *aspiration)?,
                best: initial,
            },
            Acceptance::FadingValueTabu {
                hard_tenure,
                fading_tenure,
                aspiration,
                seed,
            } => State::FadingValues {
                memory: FadingTabuMemory::new(*hard_tenure, *fading_tenure, *aspiration, *seed)?,
                best: initial,
            },
            _ => State::Ordinary(p.start(initial)?),
        };
        Ok(Self { state })
    }
    fn start_composite<'a>(
        policies: impl Iterator<Item = &'a Acceptance<S>> + Clone,
        initial: S,
        values: bool,
    ) -> Result<Self, AcceptanceError>
    where
        S: 'a,
    {
        for p in policies.clone() {
            Self::validate(p, values)?;
        }
        let mut children = Vec::new();
        children
            .try_reserve_exact(policies.size_hint().0)
            .map_err(|_| AcceptanceError::AcceptorAllocation)?;
        for p in policies {
            children.push(Self::new(p, initial.clone(), values)?);
        }
        Ok(Self {
            state: State::All(children),
        })
    }
    fn accepts(
        &mut self,
        current: S,
        candidate: S,
        variables: &[usize],
        movement: &M,
        values: &[K],
    ) -> bool {
        match &mut self.state {
            State::Ordinary(s) => s.accepts(current, candidate, variables),
            State::Tabu { memory, best } => {
                memory.accepts(std::slice::from_ref(movement), candidate, best.clone())
            }
            State::Fading { memory, best } => {
                memory.accepts(std::slice::from_ref(movement), candidate, best.clone())
            }
            State::Values { memory, best } => memory.accepts(values, candidate, best.clone()),
            State::FadingValues { memory, best } => memory.accepts(values, candidate, best.clone()),
            State::All(children) => children.iter_mut().all(|c| {
                c.accepts(
                    current.clone(),
                    candidate.clone(),
                    variables,
                    movement,
                    values,
                )
            }),
        }
    }
    fn step_ended(&mut self, score: S, variables: &[usize], movement: &M, values: &[K]) {
        match &mut self.state {
            State::Ordinary(s) => s.step_ended(score, variables),
            State::Tabu { memory, best } => {
                memory.step_ended(std::slice::from_ref(movement));
                if score > *best {
                    *best = score;
                }
            }
            State::Fading { memory, best } => {
                memory.step_ended(std::slice::from_ref(movement));
                if score > *best {
                    *best = score;
                }
            }
            State::Values { memory, best } => {
                memory.step_ended(values);
                if score > *best {
                    *best = score;
                }
            }
            State::FadingValues { memory, best } => {
                memory.step_ended(values);
                if score > *best {
                    *best = score;
                }
            }
            State::All(children) => {
                for c in children {
                    c.step_ended(score.clone(), variables, movement, values);
                }
            }
        }
    }
}
