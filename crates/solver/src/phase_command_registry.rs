//! Resolve native custom-command factories before allocating per-solve commands.
use crate::*;
use std::collections::BTreeMap;
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CustomCommandNames(Vec<String>);
impl CustomCommandNames {
    pub fn new(names: impl IntoIterator<Item = String>) -> Self {
        Self(names.into_iter().collect())
    }
    /// Nullable names are an explicit configuration/foreign-input boundary.
    /// A bad item rejects the entire update before changing an existing program.
    pub fn from_optional(
        names: impl IntoIterator<Item = Option<String>>,
    ) -> Result<Self, SolverPreflightError> {
        names
            .into_iter()
            .collect::<Option<Vec<_>>>()
            .map(Self)
            .ok_or_else(|| {
                SolverPreflightError("Custom phase commands must not contain null".into())
            })
    }
    pub fn names(&self) -> &[String] {
        &self.0
    }
    pub fn from_xml(config: &SolverXmlConfiguration) -> Vec<(usize, Self)> {
        config
            .root()
            .children
            .iter()
            .filter(|p| {
                matches!(
                    p.name.as_str(),
                    "constructionHeuristic"
                        | "localSearch"
                        | "customPhase"
                        | "exhaustiveSearch"
                        | "partitionedSearch"
                )
            })
            .enumerate()
            .filter(|(_, p)| p.name == "customPhase")
            .map(|(i, p)| {
                (
                    i,
                    Self::new(
                        p.children
                            .iter()
                            .filter(|n| n.name == "customPhaseCommandClass")
                            .map(|n| n.text.trim().to_owned()),
                    ),
                )
            })
            .collect()
    }
}
pub type NativePhaseCommand<A, M, S, E> =
    dyn for<'c> FnMut(&mut CustomPhaseContext<'c, A, M, S, E>) -> Result<(), CustomPhaseError<E>>;
type CommandFactory<A, M, S, E> = dyn Fn() -> Box<NativePhaseCommand<A, M, S, E>>;
/// Registrations create fresh mutable commands for each bound solve. Repeated
/// names instantiate independently; no JVM class loading or shared command state.
pub struct PhaseCommandRegistry<A, M, S, E> {
    factories: BTreeMap<String, Box<CommandFactory<A, M, S, E>>>,
}
impl<A, M, S, E> Default for PhaseCommandRegistry<A, M, S, E> {
    fn default() -> Self {
        Self {
            factories: BTreeMap::new(),
        }
    }
}
impl<A, M, S, E> PhaseCommandRegistry<A, M, S, E> {
    pub fn register(
        &mut self,
        name: impl Into<String>,
        factory: impl Fn() -> Box<NativePhaseCommand<A, M, S, E>> + 'static,
    ) -> Result<(), SolverPreflightError> {
        let name = name.into();
        if name.trim().is_empty() || self.factories.contains_key(&name) {
            return Err(SolverPreflightError(format!(
                "duplicate/empty customPhaseCommandClass ({name})"
            )));
        }
        self.factories.insert(name, Box::new(factory));
        Ok(())
    }
    pub fn bind(
        &self,
        sequence: &SolverSequence<'_, S>,
        declarations: &[(usize, CustomCommandNames)],
    ) -> Result<BoundPhaseCommands<A, M, S, E>, SolverPreflightError> {
        let mut seen = std::collections::BTreeSet::new();
        for (phase, names) in declarations {
            if !matches!(sequence.phases.get(*phase), Some(SolverPhase::Custom(_)))
                || !seen.insert(*phase)
            {
                return Err(SolverPreflightError(format!(
                    "invalid/duplicate custom phase index ({phase})"
                )));
            }
            for name in names.names() {
                if !self.factories.contains_key(name) {
                    return Err(SolverPreflightError(format!(
                        "unregistered customPhaseCommandClass ({name})"
                    )));
                }
            }
        }
        for (phase, p) in sequence.phases.iter().enumerate() {
            if matches!(p, SolverPhase::Custom(_)) && !seen.contains(&phase) {
                return Err(SolverPreflightError(format!(
                    "missing Custom phase commands for phase ({phase})"
                )));
            }
        }
        let commands = declarations
            .iter()
            .map(|(phase, names)| {
                (
                    *phase,
                    names
                        .names()
                        .iter()
                        .map(|name| self.factories[name]())
                        .collect(),
                )
            })
            .collect();
        Ok(BoundPhaseCommands { commands })
    }
}
type CommandList<A, M, S, E> = Vec<Box<NativePhaseCommand<A, M, S, E>>>;
/// Owned commands supplied directly by callers. Null is rejected at this input
/// boundary; array and Vec callers use the same whole-list validation.
pub struct CustomPhaseCommands<A, M, S, E> {
    commands: CommandList<A, M, S, E>,
}
impl<A, M, S, E> CustomPhaseCommands<A, M, S, E> {
    pub fn from_optional(
        commands: impl IntoIterator<Item = Option<Box<NativePhaseCommand<A, M, S, E>>>>,
    ) -> Result<Self, SolverPreflightError> {
        let commands = commands
            .into_iter()
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| {
                SolverPreflightError("Custom phase commands must not contain null".into())
            })?;
        Ok(Self { commands })
    }
    pub fn into_bound(
        self,
        sequence: &SolverSequence<'_, S>,
        phase: usize,
    ) -> Result<BoundPhaseCommands<A, M, S, E>, SolverPreflightError> {
        if !matches!(sequence.phases.get(phase), Some(SolverPhase::Custom(_)))
            || sequence
                .phases
                .iter()
                .enumerate()
                .any(|(i, p)| i != phase && matches!(p, SolverPhase::Custom(_)))
        {
            return Err(SolverPreflightError(
                "direct commands require exactly their one custom phase".into(),
            ));
        }
        Ok(BoundPhaseCommands {
            commands: BTreeMap::from([(phase, self.commands)]),
        })
    }
}
pub struct BoundPhaseCommands<A, M, S, E> {
    commands: BTreeMap<usize, CommandList<A, M, S, E>>,
}
impl<A: Clone, M, S: Score, E> BoundPhaseCommands<A, M, S, E> {
    /// Suitable for SolverExtensions::command. The existing context enforces the
    /// shared budget, retained-best restoration and poison/error behavior.
    pub fn execute(
        &mut self,
        phase: usize,
        context: &mut CustomPhaseContext<'_, A, M, S, E>,
    ) -> Result<(), CustomPhaseError<E>> {
        let commands = self.commands.get_mut(&phase).ok_or_else(|| {
            CustomPhaseError::Command(format!("unbound Custom phase commands for phase ({phase})"))
        })?;
        context.run_commands(commands.iter_mut()).map(|_| ())
    }
}
