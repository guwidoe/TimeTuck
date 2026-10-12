//! Bounded proximity ordering of actual current-state solver moves.
use crate::*;

/// Rank the supplied child moves by a registered distance meter at every opening.
/// The origin callback observes the current immutable assignment and step. The
/// child owns reachability/pinning; the engine retains normal scoring/acceptance.
/// This adapter supplies native closest-first order, not Java random sampling.
/// Compose ordinary and nearby alternatives explicitly with ProviderUnion.
///
/// Collection reads at most maximum+1 proposals and publishes no partial result
/// on overflow or invalid distance. Child/metric callback effects cannot roll
/// back. Moves need neither Clone nor Eq. Zero avoids opening the child or origin.
/// No cache survives an opening; assignment-sensitive moves are always refreshed.
/// Errors are typed at try_moves and panic through the infallible provider trait.
pub struct NearbyMoveProvider<'r, 'meter, Q, F, O, M> {
    provider: Q,
    origin: F,
    selection: NearbySelectionConfig,
    registry: &'r NearbyDistanceRegistry<'meter, O, M>,
    maximum: usize,
}
impl<'r, 'meter, Q, F, O, M> NearbyMoveProvider<'r, 'meter, Q, F, O, M> {
    pub fn new(
        provider: Q,
        origin: F,
        config: &NearbyMoveConfig,
        schema: &PlanningSchema,
        registry: &'r NearbyDistanceRegistry<'meter, O, M>,
        maximum: usize,
    ) -> Result<Self, NearbyConfigError> {
        config.validate(schema)?;
        if config.kind == NearbyMoveKind::Union || !config.children.is_empty() {
            return Err(NearbyConfigError::Unsupported(config.kind));
        }
        let selection = config
            .selectors
            .values()
            .find_map(|s| s.nearby.as_ref())
            .ok_or(NearbyConfigError::InvalidOrigin)?
            .clone();
        if !registry.contains_meter(selection.meter) {
            return Err(NearbyConfigError::UnknownMeter(selection.meter));
        }
        Ok(Self {
            provider,
            origin,
            selection,
            registry,
            maximum,
        })
    }
    pub fn provider(&self) -> &Q {
        &self.provider
    }
    pub fn provider_mut(&mut self) -> &mut Q {
        &mut self.provider
    }
    pub fn into_inner(self) -> Q {
        self.provider
    }
    pub fn try_moves<P, A>(
        &mut self,
        problem: &P,
        assignment: &A,
        step: u64,
    ) -> Result<std::vec::IntoIter<M>, NearbyConfigError>
    where
        Q: MoveProvider<P, A, M>,
        F: FnMut(&P, &A, u64) -> O,
    {
        if self.maximum == 0 {
            return Ok(Vec::new().into_iter());
        }
        let origin = (self.origin)(problem, assignment, step);
        let mut child = self.provider.moves(problem, assignment, step);
        let mut candidates: Vec<M> = child.by_ref().take(self.maximum).collect();
        if child.next().is_some() {
            return Err(NearbyConfigError::CandidateLimit {
                count: self.maximum.saturating_add(1),
                limit: self.maximum,
            });
        }
        drop(child);
        let order = self
            .registry
            .rank(&self.selection, &origin, &candidates, self.maximum)?;
        // Consume each original occurrence once without cloning its payload.
        let mut slots: Vec<_> = candidates.drain(..).map(Some).collect();
        Ok(order
            .into_iter()
            .map(|i| slots[i].take().expect("unique ranked occurrence"))
            .collect::<Vec<_>>()
            .into_iter())
    }
}
impl<P, A, M, Q, F, O> MoveProvider<P, A, M> for NearbyMoveProvider<'_, '_, Q, F, O, M>
where
    Q: MoveProvider<P, A, M>,
    F: FnMut(&P, &A, u64) -> O,
{
    fn neighborhood_count(&self) -> Result<usize, NeighborhoodCountError> {
        self.provider.neighborhood_count()
    }
    fn event(&mut self, event: MoveProviderEvent) {
        self.provider.event(event);
    }
    fn construction_placement_completed(&mut self, installed: bool) -> bool {
        self.provider.construction_placement_completed(installed)
    }
    fn moves<'a>(&'a mut self, p: &'a P, a: &'a A, step: u64) -> Box<dyn Iterator<Item = M> + 'a>
    where
        M: 'a,
    {
        Box::new(self.try_moves(p, a, step).unwrap_or_else(|e| panic!("{e}")))
    }
}

impl<P, A, M, Q, F, O> FallibleMoveProvider<P, A, M> for NearbyMoveProvider<'_, '_, Q, F, O, M>
where
    Q: MoveProvider<P, A, M>,
    F: FnMut(&P, &A, u64) -> O,
{
    fn event(&mut self, event: MoveProviderEvent) {
        self.provider.event(event);
    }
    fn construction_placement_completed(&mut self, installed: bool) -> bool {
        self.provider.construction_placement_completed(installed)
    }
    fn try_moves<'a>(
        &'a mut self,
        p: &'a P,
        a: &'a A,
        step: u64,
    ) -> Result<Box<dyn Iterator<Item = Result<M, MoveProviderError>> + 'a>, MoveProviderError>
    where
        M: 'a,
    {
        Ok(Box::new(
            NearbyMoveProvider::try_moves(self, p, a, step)
                .map_err(MoveProviderError::new)?
                .map(Ok),
        ))
    }
}
