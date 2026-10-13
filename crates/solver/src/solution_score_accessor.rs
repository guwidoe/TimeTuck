//! Typed live score-member access validated against composed solution metadata.
use crate::{ComposedPlanningSolution, PlanningType, ScoreDefinition, SolutionMemberKind};
use std::{error::Error, fmt, marker::PhantomData, sync::Arc};
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SolutionScoreAccessError {
    SolutionType {
        expected: PlanningType,
        actual: PlanningType,
    },
    MissingMember(String),
    NotScore(String),
    ScoreType {
        expected: PlanningType,
        actual: PlanningType,
    },
}
impl fmt::Display for SolutionScoreAccessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "solution score access: {self:?}")
    }
}
impl Error for SolutionScoreAccessError {}
/// Native definition operations come from the actual score's existing trait.
/// The typed descriptor identifies the raw score, never its optional Arc storage.
#[derive(Clone, Copy, Debug)]
pub struct TypedScoreDefinition<S>(PhantomData<fn() -> S>);
impl<S: ScoreDefinition + 'static> TypedScoreDefinition<S> {
    pub fn score_type(&self) -> PlanningType {
        PlanningType::of::<S>()
    }
    pub fn zero_score(&self) -> S {
        S::zero_score()
    }
    pub fn level_count(&self) -> usize {
        S::level_count()
    }
    pub fn level_labels(&self) -> Vec<String> {
        S::level_labels()
    }
}
type ScoreRead<'a, P, S> = dyn for<'p> Fn(&'p P) -> Option<&'p Arc<S>> + 'a;
type ScoreWrite<'a, P, S> = dyn Fn(&mut P, Option<Arc<S>>) + 'a;
/// Bind explicit typed getter/setter callbacks to one actual declared score field.
/// Construction validates solution, member category and raw score type before any
/// callback invocation. Reads borrow the live stored handle; writes transfer the
/// exact supplied Arc, preserving caller score-object identity. This optional
/// domain boundary does not change ordinary by-value scores/director hot paths.
/// Callbacks must actually access the declared field. Java discovery/reflection
/// and member-name ABI are not inferred. Setter panics may have external effects.
pub struct SolutionScoreAccessor<'a, P, S> {
    layout: Arc<ComposedPlanningSolution>,
    member: String,
    read: Box<ScoreRead<'a, P, S>>,
    write: Box<ScoreWrite<'a, P, S>>,
}
impl<'a, P: 'static, S: ScoreDefinition + 'static> SolutionScoreAccessor<'a, P, S> {
    pub fn new(
        layout: Arc<ComposedPlanningSolution>,
        member: impl Into<String>,
        read: impl for<'p> Fn(&'p P) -> Option<&'p Arc<S>> + 'a,
        write: impl Fn(&mut P, Option<Arc<S>>) + 'a,
    ) -> Result<Self, SolutionScoreAccessError> {
        let expected = layout.schema().solution_type();
        let actual = PlanningType::of::<P>();
        if expected != actual {
            return Err(SolutionScoreAccessError::SolutionType { expected, actual });
        }
        let member = member.into();
        let field = layout
            .member(&member)
            .ok_or_else(|| SolutionScoreAccessError::MissingMember(member.clone()))?;
        let SolutionMemberKind::Score(expected) = field.kind() else {
            return Err(SolutionScoreAccessError::NotScore(member));
        };
        let actual = PlanningType::of::<S>();
        if *expected != actual {
            return Err(SolutionScoreAccessError::ScoreType {
                expected: *expected,
                actual,
            });
        }
        Ok(Self {
            layout,
            member,
            read: Box::new(read),
            write: Box::new(write),
        })
    }
    pub fn layout(&self) -> &Arc<ComposedPlanningSolution> {
        &self.layout
    }
    pub fn member(&self) -> &str {
        &self.member
    }
    pub fn score_definition(&self) -> TypedScoreDefinition<S> {
        TypedScoreDefinition(PhantomData)
    }
    pub fn read<'p>(&self, solution: &'p P) -> Option<&'p Arc<S>> {
        (self.read)(solution)
    }
    pub fn write(&self, solution: &mut P, score: Option<Arc<S>>) {
        (self.write)(solution, score)
    }
    /// Publish a completed manager/director score as one identity-preserving owned
    /// handle. Evaluation/validation belongs to the native scoring component; this
    /// method only materializes and writes its successfully calculated result.
    pub fn publish(&self, solution: &mut P, score: S) -> Arc<S> {
        let score = Arc::new(score);
        self.write(solution, Some(score.clone()));
        score
    }
    /// Calculate from live domain state before publishing anything. A calculation
    /// error preserves the stored score and never invokes the setter.
    pub fn refresh<E>(
        &self,
        solution: &mut P,
        calculate: impl FnOnce(&P) -> Result<S, E>,
    ) -> Result<Arc<S>, E> {
        let score = calculate(solution)?;
        Ok(self.publish(solution, score))
    }
}
