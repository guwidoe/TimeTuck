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
    state: State<S, M>,
}
#[derive(Clone, Debug)]
enum State<S, M> {
    Ordinary(Acceptor<S>),
    Tabu {
        memory: TabuMemory<M>,
        best: S,
    },
    Fading {
        memory: FadingTabuMemory<M>,
        best: S,
    },
    All(Vec<MoveAcceptor<S, M>>),
}
impl<S: Score, M: Eq + Clone> MoveAcceptor<S, M> {
    pub(crate) fn new(policy: &Acceptance<S>, initial: S) -> Result<Self, AcceptanceError> {
        policy.validate()?;
        let state = match policy {
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
            _ => State::Ordinary(policy.start(initial)?),
        };
        Ok(Self { state })
    }
    /// Ordered conjunction, with trial short-circuiting and completed-step updates
    /// to every child. Empty conjunctions accept every candidate. All policies
    /// validate before constructing any state; each call creates a fresh session.
    pub fn from_policies(policies: &[Acceptance<S>], initial: S) -> Result<Self, AcceptanceError> {
        Self::start_composite(policies.iter(), initial)
    }
    pub(crate) fn start_composite<'a>(
        policies: impl Iterator<Item = &'a Acceptance<S>> + Clone,
        initial: S,
    ) -> Result<Self, AcceptanceError>
    where
        S: 'a,
    {
        for policy in policies.clone() {
            policy.validate()?;
        }
        let mut children = Vec::new();
        children
            .try_reserve_exact(policies.size_hint().0)
            .map_err(|_| AcceptanceError::AcceptorAllocation)?;
        for policy in policies {
            children.push(Self::new(policy, initial.clone())?);
        }
        Ok(Self {
            state: State::All(children),
        })
    }
    /// Examine a scored trial without retaining its descriptor or aging memory.
    /// Fading checks can consume a seeded draw. Aspiration uses the best score
    /// observed at initialization or a completed step, not an unselected trial.
    pub fn accepts(&mut self, current: S, candidate: S, variables: &[usize], movement: &M) -> bool {
        match &mut self.state {
            State::Ordinary(state) => state.accepts(current, candidate, variables),
            State::Tabu { memory, best } => {
                memory.accepts(std::slice::from_ref(movement), candidate, best.clone())
            }
            State::Fading { memory, best } => {
                memory.accepts(std::slice::from_ref(movement), candidate, best.clone())
            }
            State::All(children) => children.iter_mut().all(|child| {
                child.accepts(current.clone(), candidate.clone(), variables, movement)
            }),
        }
    }
    /// Retain the selected descriptor and advance one completed step, including
    /// a configured rejected fallback. Unselected trials must not call this.
    pub fn step_ended(&mut self, score: S, variables: &[usize], movement: &M) {
        match &mut self.state {
            State::Ordinary(state) => state.step_ended(score, variables),
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
            State::All(children) => {
                for child in children {
                    child.step_ended(score.clone(), variables, movement);
                }
            }
        }
    }
}
