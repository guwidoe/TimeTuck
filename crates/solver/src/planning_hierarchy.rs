//! Optional executable planning bindings for Rust composition and trait access.
//! Bindings project real parent storage; no reflection or annotation scan occurs.
use crate::{
    Assignment, FiniteProblem, PlanningEntityDefinition, PlanningSchema,
    PlanningVariableDefinition, Variable,
};
use std::{error::Error, fmt, sync::Arc};

type Read<E, V> = dyn Fn(&E) -> Option<V> + Send + Sync;
type Write<E, V> = dyn Fn(&mut E, Option<V>) + Send + Sync;
struct Member<E, V> {
    name: String,
    range: String,
    read: Arc<Read<E, V>>,
    write: Arc<Write<E, V>>,
}
/// Executable genuine members, including members lifted from contained parents.
pub struct PlanningEntityBinding<E, V> {
    members: Vec<Member<E, V>>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanningHierarchyError(pub String);
impl fmt::Display for PlanningHierarchyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl Error for PlanningHierarchyError {}
impl<E: 'static, V: 'static> Default for PlanningEntityBinding<E, V> {
    fn default() -> Self {
        Self { members: vec![] }
    }
}
impl<E: 'static, V: 'static> PlanningEntityBinding<E, V> {
    pub fn member(
        mut self,
        name: impl Into<String>,
        range: impl Into<String>,
        read: impl Fn(&E) -> Option<V> + Send + Sync + 'static,
        write: impl Fn(&mut E, Option<V>) + Send + Sync + 'static,
    ) -> Result<Self, PlanningHierarchyError> {
        let name = name.into();
        if self.members.iter().any(|m| m.name == name) {
            return Err(PlanningHierarchyError(format!(
                "redefines the genuine variables ([{name}]), which is not permitted."
            )));
        }
        let range = range.into();
        if name.is_empty() || range.is_empty() {
            return Err(PlanningHierarchyError(
                "member and range names must not be empty".into(),
            ));
        }
        self.members.push(Member {
            name,
            range,
            read: Arc::new(read),
            write: Arc::new(write),
        });
        Ok(self)
    }
    /// Lift each callable parent member through immutable and mutable projections.
    pub fn inherit<P: 'static>(
        mut self,
        parent: PlanningEntityBinding<P, V>,
        project: impl for<'a> Fn(&'a E) -> &'a P + Send + Sync + 'static,
        project_mut: impl for<'a> Fn(&'a mut E) -> &'a mut P + Send + Sync + 'static,
    ) -> Result<Self, PlanningHierarchyError> {
        let project = Arc::new(project);
        let project_mut = Arc::new(project_mut);
        for m in parent.members {
            let p = project.clone();
            let w = project_mut.clone();
            self = self.member(
                m.name,
                m.range,
                move |e| (m.read)(p(e)),
                move |e, v| (m.write)(w(e), v),
            )?;
        }
        Ok(self)
    }
    pub fn definition(&self) -> PlanningEntityDefinition {
        PlanningEntityDefinition::of::<E>(
            self.members
                .iter()
                .map(|m| PlanningVariableDefinition::basic::<V>(&m.name, false))
                .collect(),
        )
    }
}
type Open<S, V> = dyn Fn(&S) -> Result<Vec<Variable<V>>, PlanningHierarchyError> + Send + Sync;
type Commit<S, V> = dyn Fn(&mut S, &[Option<V>]) + Send + Sync;
type MutableSlots<S> = dyn Fn(&mut S) -> Result<usize, PlanningHierarchyError> + Send + Sync;
struct Collection<S, V> {
    definition: PlanningEntityDefinition,
    open: Box<Open<S, V>>,
    commit: Box<Commit<S, V>>,
    mutable_slots: Box<MutableSlots<S>>,
}
/// An owned model layout. Opening and committing both resolve current collections.
/// Values/ranges are read from the supplied solution, never cached across solves.
pub struct PlanningSolutionBinding<S, V> {
    collections: Vec<Collection<S, V>>,
}
impl<S: 'static, V: Clone + Eq + 'static> Default for PlanningSolutionBinding<S, V> {
    fn default() -> Self {
        Self {
            collections: vec![],
        }
    }
}
impl<S: 'static, V: Clone + Eq + 'static> PlanningSolutionBinding<S, V> {
    pub fn collection<E: 'static>(
        mut self,
        binding: PlanningEntityBinding<E, V>,
        read: impl for<'a> Fn(&'a S) -> &'a [E] + Send + Sync + 'static,
        write: impl for<'a> Fn(&'a mut S) -> &'a mut [E] + Send + Sync + 'static,
        range: impl Fn(&S, &str) -> Option<Vec<V>> + Send + Sync + 'static,
    ) -> Self {
        let definition = binding.definition();
        let binding = Arc::new(binding);
        let b = binding.clone();
        let open = move |s: &S| {
            let mut variables = vec![];
            for e in read(s) {
                for m in &b.members {
                    let domain = range(s, &m.range).ok_or_else(|| {
                        PlanningHierarchyError(format!(
                            "missing value range ({}) for member ({})",
                            m.range, m.name
                        ))
                    })?;
                    let mut v = Variable::new(domain);
                    if let Some(initial) = (m.read)(e) {
                        v = v.with_initial(initial)
                    }
                    variables.push(v);
                }
            }
            Ok(variables)
        };
        let write = Arc::new(write);
        let size_projection = write.clone();
        let width = binding.members.len();
        let mutable_slots = move |s: &mut S| {
            size_projection(s).len().checked_mul(width).ok_or_else(|| {
                PlanningHierarchyError("mutable collection slot count overflow".into())
            })
        };
        let commit = move |s: &mut S, values: &[Option<V>]| {
            let mut i = 0;
            for e in write(s) {
                for m in &binding.members {
                    (m.write)(e, values[i].clone());
                    i += 1;
                }
            }
        };
        self.collections.push(Collection {
            definition,
            open: Box::new(open),
            commit: Box::new(commit),
            mutable_slots: Box::new(mutable_slots),
        });
        self
    }
    /// Compose inherited solution collections through real parent storage.
    pub fn inherit<P: 'static>(
        mut self,
        parent: PlanningSolutionBinding<P, V>,
        project: impl for<'a> Fn(&'a S) -> &'a P + Send + Sync + 'static,
        project_mut: impl for<'a> Fn(&'a mut S) -> &'a mut P + Send + Sync + 'static,
    ) -> Self {
        let p = Arc::new(project);
        let w = Arc::new(project_mut);
        for c in parent.collections {
            let p = p.clone();
            let w = w.clone();
            let size_projection = w.clone();
            self.collections.push(Collection {
                definition: c.definition,
                open: Box::new(move |s| (c.open)(p(s))),
                commit: Box::new(move |s, v| (c.commit)(w(s), v)),
                mutable_slots: Box::new(move |s| (c.mutable_slots)(size_projection(s))),
            });
        }
        self
    }
    pub fn schema(&self) -> Result<PlanningSchema, PlanningHierarchyError> {
        let mut defs: Vec<PlanningEntityDefinition> = vec![];
        for c in &self.collections {
            if let Some(d) = defs
                .iter()
                .find(|d| d.entity_type() == c.definition.entity_type())
            {
                if !d
                    .variables()
                    .iter()
                    .map(|v| (v.name(), v.value_type(), v.kind()))
                    .eq(c
                        .definition
                        .variables()
                        .iter()
                        .map(|v| (v.name(), v.value_type(), v.kind())))
                {
                    return Err(PlanningHierarchyError(
                        "inconsistent bindings for the same entity type".into(),
                    ));
                }
            } else {
                defs.push(c.definition.clone());
            }
        }
        PlanningSchema::new::<S>(defs).map_err(|e| PlanningHierarchyError(e.to_string()))
    }
    pub fn open(&self, solution: &S) -> Result<FiniteProblem<V>, PlanningHierarchyError> {
        self.schema()?;
        let mut variables = vec![];
        for c in &self.collections {
            variables.extend((c.open)(solution)?)
        }
        FiniteProblem::new(variables).map_err(|e| PlanningHierarchyError(e.to_string()))
    }
    /// Validate all ranges and collection lengths before executing any setter.
    /// Callers must keep membership unchanged between solve and commit; a fresh
    /// opening validates the live solution rather than retaining stale pointers.
    /// Read/write projections must be pure and refer to the same storage. Slot
    /// counts are checked before any setter; equal-length wrong storage cannot
    /// be detected. Setters must not change another collection's membership.
    pub fn commit(
        &self,
        solution: &mut S,
        assignment: &Assignment<V>,
    ) -> Result<(), PlanningHierarchyError> {
        let model = self.open(solution)?;
        model
            .validate(assignment)
            .map_err(|e| PlanningHierarchyError(e.to_string()))?;
        let lengths = self
            .collections
            .iter()
            .map(|c| (c.open)(solution).map(|v| v.len()))
            .collect::<Result<Vec<_>, _>>()?;
        for (index, (c, expected)) in self.collections.iter().zip(&lengths).enumerate() {
            let actual = (c.mutable_slots)(solution)?;
            if actual != *expected {
                return Err(PlanningHierarchyError(format!(
                    "collection ({index}) read/write slot count mismatch: read {expected}, write {actual}"
                )));
            }
        }
        let mut offset = 0;
        for (c, len) in self.collections.iter().zip(lengths) {
            (c.commit)(solution, &assignment.values()[offset..offset + len]);
            offset += len;
        }
        Ok(())
    }
}
