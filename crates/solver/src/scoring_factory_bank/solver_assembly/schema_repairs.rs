//! Validate metadata-derived reconstruction pools before scoring preparation.
use super::*;
use crate::{SchemaRuinError, SchemaRuinPhaseConfiguration, SchemaRuinSelector};
pub(super) fn prepare_schema_repairs<S, E>(
    schema: &PlanningSchema,
    phases: &[SolverPhase<'_, S>],
    declarations: &[SchemaRuinPhaseConfiguration],
) -> Result<Vec<Option<SchemaRuinSelector>>, FactoryAssemblyError<E>> {
    if declarations.is_empty() {
        return Ok(vec![]);
    }
    let mut selectors = vec![None; phases.len()];
    for declaration in declarations {
        let phase = declaration.phase;
        let checked = || {
            let plan = phases.get(phase).ok_or(SchemaRuinError::PhaseIndex {
                phase,
                count: phases.len(),
            })?;
            if !matches!(plan, SolverPhase::RuinRecreate(_)) {
                return Err(SchemaRuinError::NotRuinPhase { phase });
            }
            if selectors[phase].is_some() {
                return Err(SchemaRuinError::DuplicatePhase { phase });
            }
            declaration
                .selector
                .bind_schema_ruin(schema, &declaration.bindings)
        };
        let selector =
            checked().map_err(|error| FactoryAssemblyError::SchemaRuin { phase, error })?;
        selectors[phase] = Some(selector);
    }
    Ok(selectors)
}
impl<'a, M: 'a, P: 'a, I: 'a, D: ContextScoreResource + 'a> AssembledSolverFactory<'a, M, P, I, D>
where
    D::Error: 'static,
{
    pub fn assembled_ruin_selector(&self, phase: usize) -> Option<&SchemaRuinSelector> {
        self.schema_ruin.get(phase)?.as_ref()
    }
    /// The application phase binding receives the prepared selector alongside
    /// its actual phase/resource. Install the validated candidate pool in that
    /// native repair call. The ordinary callback API remains available for
    /// callers that bind metadata themselves, just as they bind other settings.
    pub fn solve_assembled_with_schema_repairs<W: ContextScoreWork>(
        &self,
        input: I,
        mut run: impl FnMut(
            &mut D,
            &SolverPhase<'a, D::Score>,
            Option<&SchemaRuinSelector>,
        ) -> Result<W, D::Error>,
    ) -> Result<FactoryAssemblySolution<I, D::Score, W>, FactoryExecutionFailure<D::Error>> {
        let mut phase = 0;
        self.solve_assembled_with(input, |resource, plan| {
            let selector = self.assembled_ruin_selector(phase);
            phase += 1;
            run(resource, plan, selector)
        })
    }
}
