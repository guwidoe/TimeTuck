//! Identity conversion for an owned model used as input, solver model and output.
//! Required boundary arguments are checked before any solve callback executes.
use std::{error::Error, fmt, marker::PhantomData, sync::Arc};

/// A declared previous-output state. An absent declaration in a raw request is
/// distinct from explicitly declaring that no previous result exists.
pub enum PreviousModelOutput<M> {
    Empty,
    Present(Arc<M>),
}
pub struct ModelConversionRequest<M, C> {
    pub model_input: Option<Arc<M>>,
    pub model_config: Option<Arc<C>>,
    pub last_model_output: Option<PreviousModelOutput<M>>,
}
impl<M, C> ModelConversionRequest<M, C> {
    /// The ordinary typed path always supplies the required previous declaration.
    pub fn new(input: Arc<M>, config: Arc<C>, previous: Option<Arc<M>>) -> Self {
        Self {
            model_input: Some(input),
            model_config: Some(config),
            last_model_output: Some(
                previous.map_or(PreviousModelOutput::Empty, PreviousModelOutput::Present),
            ),
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelConversionError {
    MissingArgument(&'static str),
}
impl fmt::Display for ModelConversionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingArgument(name) => f.write_str(name),
        }
    }
}
impl Error for ModelConversionError {}

/// Validated owned handles. Identity conversion retains input identity; a solve
/// callback receives configuration and any previous output explicitly. Whether
/// to use that output as a warm start is the caller's native solve policy.
pub struct PreparedModelConversion<M, C> {
    pub model_input: Arc<M>,
    pub model_config: Arc<C>,
    pub last_model_output: Option<Arc<M>>,
}
pub struct ConvertedModelOutput<M> {
    pub model_output: Arc<M>,
    pub replacement_input: Arc<M>,
}
#[derive(Debug)]
pub enum ConvertingSolveError<E> {
    Conversion(ModelConversionError),
    Solve(E),
}
impl<E: fmt::Display> fmt::Display for ConvertingSolveError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Conversion(error) => error.fmt(f),
            Self::Solve(error) => error.fmt(f),
        }
    }
}
impl<E: Error + 'static> Error for ConvertingSolveError<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Conversion(error) => Some(error),
            Self::Solve(error) => Some(error),
        }
    }
}
/// The same real M type is used throughout conversion; names or flags never
/// establish type identity. Models and configuration stay owned by their callers.
pub struct TrivialModelConvertor<M, C = ()>(PhantomData<fn() -> (M, C)>);
impl<M, C> Default for TrivialModelConvertor<M, C> {
    fn default() -> Self {
        Self::new()
    }
}
impl<M, C> TrivialModelConvertor<M, C> {
    pub const fn new() -> Self {
        Self(PhantomData)
    }
    pub fn prepare(
        &self,
        request: &ModelConversionRequest<M, C>,
    ) -> Result<PreparedModelConversion<M, C>, ModelConversionError> {
        let input = request
            .model_input
            .as_ref()
            .ok_or(ModelConversionError::MissingArgument("modelInput"))?;
        let config = request
            .model_config
            .as_ref()
            .ok_or(ModelConversionError::MissingArgument("modelConfig"))?;
        let previous = request
            .last_model_output
            .as_ref()
            .ok_or(ModelConversionError::MissingArgument("lastModelOutput"))?;
        Ok(PreparedModelConversion {
            model_input: Arc::clone(input),
            model_config: Arc::clone(config),
            last_model_output: match previous {
                PreviousModelOutput::Empty => None,
                PreviousModelOutput::Present(output) => Some(Arc::clone(output)),
            },
        })
    }
    pub fn to_solver_model(
        &self,
        request: &ModelConversionRequest<M, C>,
    ) -> Result<Arc<M>, ModelConversionError> {
        Ok(self.prepare(request)?.model_input)
    }
    pub fn to_model_output(
        &self,
        solver_model: Option<&Arc<M>>,
    ) -> Result<Arc<M>, ModelConversionError> {
        solver_model
            .cloned()
            .ok_or(ModelConversionError::MissingArgument("solverModel"))
    }
    pub fn apply_output_to_input(
        &self,
        model_input: Option<&Arc<M>>,
        model_output: Option<&Arc<M>>,
    ) -> Result<Arc<M>, ModelConversionError> {
        model_input.ok_or(ModelConversionError::MissingArgument("modelInput"))?;
        model_output
            .cloned()
            .ok_or(ModelConversionError::MissingArgument("modelOutput"))
    }
    /// Validate once before invoking an actual solver. Failures produce no output
    /// or replacement handle. Input itself is never overwritten by this wrapper.
    pub fn solve<E>(
        &self,
        request: &ModelConversionRequest<M, C>,
        solve: impl FnOnce(PreparedModelConversion<M, C>) -> Result<Arc<M>, E>,
    ) -> Result<ConvertedModelOutput<M>, ConvertingSolveError<E>> {
        let prepared = self
            .prepare(request)
            .map_err(ConvertingSolveError::Conversion)?;
        let output = solve(prepared).map_err(ConvertingSolveError::Solve)?;
        Ok(ConvertedModelOutput {
            model_output: Arc::clone(&output),
            replacement_input: output,
        })
    }
}
