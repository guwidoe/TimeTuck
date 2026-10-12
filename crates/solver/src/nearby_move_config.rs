//! Explicit nearby selector trees and typed distance ranking.
use crate::*;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    error::Error,
    fmt,
};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NearbyMoveKind {
    Change,
    Swap,
    ListChange,
    ListSwap,
    KOpt,
    Union,
    Cartesian,
    IteratorFactory,
    ListFactory,
    SubListChange,
    SubListSwap,
    PillarChange,
    PillarSwap,
}
impl NearbyMoveKind {
    pub fn supports_auto_nearby(self) -> bool {
        matches!(
            self,
            Self::Change
                | Self::Swap
                | Self::ListChange
                | Self::ListSwap
                | Self::KOpt
                | Self::Union
        )
    }
    pub fn supports_nearby_in_mixed(self) -> bool {
        matches!(self, Self::ListChange | Self::ListSwap | Self::KOpt)
    }
    fn roles(self) -> Option<(NearbySelectorRole, NearbySelectorRole)> {
        use NearbySelectorRole::*;
        Some(match self {
            Self::Change => (Entity, Value),
            Self::Swap => (Entity, SecondaryEntity),
            Self::ListChange => (Value, Destination),
            Self::ListSwap => (Value, SecondaryValue),
            Self::KOpt => (OriginValue, Value),
            _ => return None,
        })
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum NearbySelectorRole {
    Entity,
    SecondaryEntity,
    Value,
    SecondaryValue,
    Destination,
    OriginValue,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NearbySelectionConfig {
    pub origin_id: String,
    pub origin_role: NearbySelectorRole,
    pub meter: PlanningType,
}
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct NearbyElementConfig {
    pub id: Option<String>,
    pub order: Option<SelectionOrder>,
    pub nearby: Option<NearbySelectionConfig>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NearbyMoveConfig {
    pub kind: NearbyMoveKind,
    pub order: Option<SelectionOrder>,
    pub selectors: BTreeMap<NearbySelectorRole, NearbyElementConfig>,
    pub children: Vec<NearbyMoveConfig>,
    auto_expanded: bool,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NearbyConfigError {
    Unsupported(NearbyMoveKind),
    InvalidOrigin,
    MixedModel(NearbyMoveKind),
    NearbySorted,
    UnknownMeter(PlanningType),
    DuplicateMeter(PlanningType),
    InvalidDistance { candidate: usize },
    CandidateLimit { count: usize, limit: usize },
}
impl fmt::Display for NearbyConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "nearby selection: {self:?}")
    }
}
impl Error for NearbyConfigError {}
impl NearbyMoveConfig {
    pub fn new(kind: NearbyMoveKind) -> Self {
        Self {
            kind,
            order: None,
            selectors: BTreeMap::new(),
            children: vec![],
            auto_expanded: false,
        }
    }
    pub fn union(children: Vec<Self>) -> Self {
        Self {
            children,
            ..Self::new(NearbyMoveKind::Union)
        }
    }
    pub fn selector(&self, role: NearbySelectorRole) -> Option<&NearbyElementConfig> {
        self.selectors.get(&role)
    }
    pub fn selector_mut(&mut self, role: NearbySelectorRole) -> &mut NearbyElementConfig {
        self.selectors.entry(role).or_default()
    }
    pub fn has_nearby(&self) -> bool {
        self.selectors.values().any(|s| s.nearby.is_some())
            || self.children.iter().any(Self::has_nearby)
    }
    /// Persistent transformation. Unions retain original leaf alternatives next
    /// to nearby variants; nested unions preserve their topology. Unsupported
    /// children stay untouched. Reapplying a transform does not duplicate it.
    /// Native seeded IDs are unique within the supplied tree, not Java RNG IDs.
    pub fn enable_nearby(&self, meter: PlanningType, seed: u64) -> Result<Self, NearbyConfigError> {
        if !self.kind.supports_auto_nearby() {
            return Err(NearbyConfigError::Unsupported(self.kind));
        }
        let mut ids = HashSet::new();
        self.collect_ids(&mut ids);
        let mut count = 0;
        self.transform(meter, seed, &mut count, &mut ids)
    }
    fn collect_ids(&self, ids: &mut HashSet<String>) {
        for s in self.selectors.values() {
            if let Some(id) = &s.id {
                ids.insert(id.clone());
            }
            if let Some(n) = &s.nearby {
                ids.insert(n.origin_id.clone());
            }
        }
        for c in &self.children {
            c.collect_ids(ids);
        }
    }
    fn transform(
        &self,
        meter: PlanningType,
        seed: u64,
        count: &mut u64,
        ids: &mut HashSet<String>,
    ) -> Result<Self, NearbyConfigError> {
        if self.auto_expanded {
            return Ok(self.clone());
        }
        let mut out = self.clone();
        if self.kind == NearbyMoveKind::Union {
            out.children.clear();
            for child in &self.children {
                if child.kind == NearbyMoveKind::Union {
                    out.children.push(child.transform(meter, seed, count, ids)?);
                } else if child.kind.supports_auto_nearby() && !child.has_nearby() {
                    out.children.push(child.clone());
                    out.children.push(child.transform(meter, seed, count, ids)?);
                } else {
                    out.children.push(child.clone());
                }
            }
            out.auto_expanded = true;
            return Ok(out);
        }
        if self.has_nearby() {
            return Ok(out);
        }
        let (origin, receiver) = self
            .kind
            .roles()
            .ok_or(NearbyConfigError::Unsupported(self.kind))?;
        let id = loop {
            let id = format!("nearby-{seed}-{}", *count);
            *count = count
                .checked_add(1)
                .ok_or(NearbyConfigError::InvalidOrigin)?;
            if ids.insert(id.clone()) {
                break id;
            }
        };
        out.selector_mut(origin).id = Some(id.clone());
        out.selector_mut(receiver).nearby = Some(NearbySelectionConfig {
            origin_id: id,
            origin_role: origin,
            meter,
        });
        out.auto_expanded = true;
        Ok(out)
    }
    /// Validate references and model compatibility before any metric executes.
    pub fn validate(&self, schema: &PlanningSchema) -> Result<(), NearbyConfigError> {
        let basic = schema
            .genuine_entities()
            .flat_map(|e| e.genuine_variables())
            .any(|v| matches!(v.kind(), PlanningVariableKind::Basic { .. }));
        let lists = schema
            .genuine_entities()
            .flat_map(|e| e.genuine_variables())
            .any(|v| matches!(v.kind(), PlanningVariableKind::List { .. }));
        self.validate_model(basic && lists)
    }
    fn validate_model(&self, mixed: bool) -> Result<(), NearbyConfigError> {
        for (role, selector) in &self.selectors {
            if let Some(nearby) = &selector.nearby {
                let (origin, receiver) = self
                    .kind
                    .roles()
                    .ok_or(NearbyConfigError::Unsupported(self.kind))?;
                if *role != receiver
                    || nearby.origin_role != origin
                    || self.selector(origin).and_then(|s| s.id.as_ref()) != Some(&nearby.origin_id)
                {
                    return Err(NearbyConfigError::InvalidOrigin);
                }
                if mixed && !self.kind.supports_nearby_in_mixed() {
                    return Err(NearbyConfigError::MixedModel(self.kind));
                }
                if selector.order == Some(SelectionOrder::Sorted) {
                    return Err(NearbyConfigError::NearbySorted);
                }
            }
        }
        for child in &self.children {
            child.validate_model(mixed)?;
        }
        Ok(())
    }
}
type Distance<'a, O, D> = dyn Fn(&O, &D) -> f64 + 'a;
/// Explicit meter registration replaces reflective distance classes. Ranking
/// evaluates each supplied candidate once, rejects invalid distances, keeps
/// encounter order for ties, and returns original occurrence indices. Callers
/// supply reachable/pin-filtered candidates; no value clone or Eq bound needed.
/// This defines native proximity ranking, not Java nearby sampling distributions.
pub struct NearbyDistanceRegistry<'a, O, D> {
    meters: HashMap<PlanningType, Box<Distance<'a, O, D>>>,
}
impl<O, D> Default for NearbyDistanceRegistry<'_, O, D> {
    fn default() -> Self {
        Self {
            meters: HashMap::new(),
        }
    }
}
impl<'a, O, D> NearbyDistanceRegistry<'a, O, D> {
    pub fn contains_meter(&self, meter: PlanningType) -> bool {
        self.meters.contains_key(&meter)
    }
    pub fn register<M: 'static>(
        &mut self,
        meter: impl Fn(&O, &D) -> f64 + 'a,
    ) -> Result<&mut Self, NearbyConfigError> {
        let key = PlanningType::of::<M>();
        if self.meters.contains_key(&key) {
            return Err(NearbyConfigError::DuplicateMeter(key));
        }
        self.meters.insert(key, Box::new(meter));
        Ok(self)
    }
    pub fn rank(
        &self,
        spec: &NearbySelectionConfig,
        origin: &O,
        candidates: &[D],
        maximum_candidates: usize,
    ) -> Result<Vec<usize>, NearbyConfigError> {
        if candidates.len() > maximum_candidates {
            return Err(NearbyConfigError::CandidateLimit {
                count: candidates.len(),
                limit: maximum_candidates,
            });
        }
        let meter = self
            .meters
            .get(&spec.meter)
            .ok_or(NearbyConfigError::UnknownMeter(spec.meter))?;
        let mut distances = Vec::with_capacity(candidates.len());
        for (i, c) in candidates.iter().enumerate() {
            let distance = meter(origin, c);
            if !distance.is_finite() || distance < 0.0 {
                return Err(NearbyConfigError::InvalidDistance { candidate: i });
            }
            distances.push((i, if distance == 0.0 { 0.0 } else { distance }));
        }
        distances.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
        Ok(distances.into_iter().map(|(i, _)| i).collect())
    }
}
