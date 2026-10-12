//! Read concrete solution fact members once and retain their real allocations.
use crate::{ComposedPlanningSolution, PlanningType, SolutionMemberCategory, SolutionMemberKind};
use std::{any::Any, collections::HashMap, fmt, sync::Arc};

#[derive(Clone)]
pub struct SolutionFactRecord {
    object: Arc<dyn Any + Send + Sync>,
    fact_type: PlanningType,
}
impl SolutionFactRecord {
    pub fn new<T: Any + Send + Sync>(object: Arc<T>) -> Self {
        Self {
            object,
            fact_type: PlanningType::of::<T>(),
        }
    }
    pub fn fact_type(&self) -> PlanningType {
        self.fact_type
    }
    pub fn object(&self) -> &Arc<dyn Any + Send + Sync> {
        &self.object
    }
    pub fn object_ref<T: Any>(&self) -> Option<&T> {
        self.object.downcast_ref()
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SolutionFactError {
    SolutionType {
        expected: PlanningType,
        actual: PlanningType,
    },
    UnknownMember(String),
    WrongRole(String),
    WrongType {
        member: String,
        actual: PlanningType,
    },
    EntityAsFact {
        member: String,
        actual: PlanningType,
    },
    DuplicateAccessor(String),
    MissingAccessor(String),
    SingletonCardinality(String),
}
impl fmt::Display for SolutionFactError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "solution facts: {self:?}")
    }
}
impl std::error::Error for SolutionFactError {}
type FactRead<'a, S> = dyn Fn(&S) -> Vec<SolutionFactRecord> + 'a;
/// Typed registration replaces reflective getters. Computed collections run on
/// every explicit read; a published snapshot's membership stays unchanged.
/// Interior mutation of a shared fact payload remains the caller's responsibility.
/// Readers may clone Arc handles, but fact payloads need no Clone/Eq/Hash bound.
/// Signature/role/type and complete-registration checks precede callback calls.
/// Heterogeneous payloads are checked before the new snapshot is published.
/// Callback side effects are not rolled back by a failed read.
pub struct SolutionFactAccessors<'a, S> {
    layout: Arc<ComposedPlanningSolution>,
    reads: HashMap<String, Box<FactRead<'a, S>>>,
}
impl<'a, S: 'static> SolutionFactAccessors<'a, S> {
    pub fn new(layout: Arc<ComposedPlanningSolution>) -> Result<Self, SolutionFactError> {
        let actual = PlanningType::of::<S>();
        let expected = layout.schema().solution_type();
        if actual != expected {
            return Err(SolutionFactError::SolutionType { actual, expected });
        }
        Ok(Self {
            layout,
            reads: HashMap::new(),
        })
    }
    fn member(&self, name: &str, singleton: bool) -> Result<(), SolutionFactError> {
        let member = self
            .layout
            .member(name)
            .ok_or_else(|| SolutionFactError::UnknownMember(name.into()))?;
        let eligible = if singleton {
            matches!(
                member.kind(),
                SolutionMemberKind::Facts {
                    singleton: true,
                    ..
                }
            )
        } else {
            member.is_problem_fact_collection()
        };
        if !eligible {
            return Err(SolutionFactError::WrongRole(name.into()));
        }
        if self.reads.contains_key(name) {
            return Err(SolutionFactError::DuplicateAccessor(name.into()));
        }
        Ok(())
    }
    fn fact_type(&self, name: &str, actual: PlanningType) -> Result<(), SolutionFactError> {
        if self.layout.schema().entity_type(actual).is_ok() {
            return Err(SolutionFactError::EntityAsFact {
                member: name.into(),
                actual,
            });
        }
        let member = self.layout.member(name).expect("validated fact member");
        let valid = match member.kind() {
            SolutionMemberKind::Facts { allowed_types, .. } => {
                allowed_types.is_empty() || allowed_types.contains(&actual)
            }
            SolutionMemberKind::ValueRange { value_type, .. } => *value_type == actual,
            _ => false,
        };
        if !valid {
            return Err(SolutionFactError::WrongType {
                member: name.into(),
                actual,
            });
        }
        Ok(())
    }
    pub fn singleton<T: Any + Send + Sync>(
        &mut self,
        name: impl Into<String>,
        read: impl Fn(&S) -> Option<Arc<T>> + 'a,
    ) -> Result<&mut Self, SolutionFactError> {
        let name = name.into();
        self.member(&name, true)?;
        self.fact_type(&name, PlanningType::of::<T>())?;
        self.reads.insert(
            name,
            Box::new(move |s| read(s).into_iter().map(SolutionFactRecord::new).collect()),
        );
        Ok(self)
    }
    pub fn collection<T: Any + Send + Sync, I: IntoIterator<Item = Arc<T>>>(
        &mut self,
        name: impl Into<String>,
        read: impl Fn(&S) -> I + 'a,
    ) -> Result<&mut Self, SolutionFactError> {
        let name = name.into();
        self.member(&name, false)?;
        self.fact_type(&name, PlanningType::of::<T>())?;
        self.reads.insert(
            name,
            Box::new(move |s| read(s).into_iter().map(SolutionFactRecord::new).collect()),
        );
        Ok(self)
    }
    pub fn heterogeneous(
        &mut self,
        name: impl Into<String>,
        read: impl Fn(&S) -> Vec<SolutionFactRecord> + 'a,
    ) -> Result<&mut Self, SolutionFactError> {
        let name = name.into();
        let single = self.layout.member(&name).is_some_and(|m| {
            matches!(
                m.kind(),
                SolutionMemberKind::Facts {
                    singleton: true,
                    ..
                }
            )
        });
        self.member(&name, single)?;
        self.reads.insert(name, Box::new(read));
        Ok(self)
    }
    pub fn read(&self, solution: &S) -> Result<BoundSolutionFacts, SolutionFactError> {
        let members: Vec<_> = self
            .layout
            .members()
            .iter()
            .filter(|m| {
                m.is_problem_fact_collection()
                    || m.kind().category() == SolutionMemberCategory::ProblemFact
            })
            .collect();
        for member in &members {
            if !self.reads.contains_key(member.name()) {
                return Err(SolutionFactError::MissingAccessor(member.name().into()));
            }
        }
        let mut records: Vec<SolutionFactRecord> = vec![];
        let mut by_object = HashMap::new();
        let mut collections = HashMap::new();
        let mut by_type: HashMap<PlanningType, Vec<usize>> = HashMap::new();
        for member in members {
            let read = (self.reads[member.name()])(solution);
            if matches!(
                member.kind(),
                SolutionMemberKind::Facts {
                    singleton: true,
                    ..
                }
            ) && read.len() > 1
            {
                return Err(SolutionFactError::SingletonCardinality(
                    member.name().into(),
                ));
            }
            let mut rows = vec![];
            for record in read {
                self.fact_type(member.name(), record.fact_type)?;
                let key = Arc::as_ptr(&record.object) as *const () as usize;
                let row = if let Some(&row) = by_object.get(&key) {
                    row
                } else {
                    let row = records.len();
                    by_object.insert(key, row);
                    by_type.entry(record.fact_type).or_default().push(row);
                    records.push(record);
                    row
                };
                rows.push(row);
            }
            collections.insert(member.name().into(), rows);
        }
        Ok(BoundSolutionFacts {
            layout: self.layout.clone(),
            records,
            collections,
            by_type,
        })
    }
}
/// Canonical identity rows plus each member's occurrence order (including aliases).
/// The descriptor and actual Arc allocations outlive the registry/solution.
/// Type queries use a prebuilt row index; they do not scan all fact payloads.
pub struct BoundSolutionFacts {
    layout: Arc<ComposedPlanningSolution>,
    records: Vec<SolutionFactRecord>,
    collections: HashMap<String, Vec<usize>>,
    by_type: HashMap<PlanningType, Vec<usize>>,
}
impl BoundSolutionFacts {
    pub fn descriptor(&self) -> &Arc<ComposedPlanningSolution> {
        &self.layout
    }
    pub fn records(&self) -> &[SolutionFactRecord] {
        &self.records
    }
    pub fn collection(&self, name: &str) -> Option<&[usize]> {
        self.collections.get(name).map(Vec::as_slice)
    }
    pub fn facts_of<T: Any>(&self) -> impl Iterator<Item = &T> {
        self.by_type
            .get(&PlanningType::of::<T>())
            .into_iter()
            .flatten()
            .map(|&row| {
                self.records[row]
                    .object_ref::<T>()
                    .expect("canonical typed fact record")
            })
    }
}
