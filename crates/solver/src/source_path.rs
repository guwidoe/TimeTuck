//! Typed, borrowed traversal of declarative shadow source paths.
use crate::{PlanningSchema, PlanningType, PlanningVariableKind, PlanningVariableRef};
use std::{any::Any, collections::BTreeSet, error::Error, fmt};

/// The operational role of an explicitly registered member. Variable uses the
/// schema's basic/list/shadow kind; Previous, Next and Declarative refine shadow fields.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceMemberRole {
    Fact,
    Variable,
    Previous,
    Next,
    Declarative,
    Inconsistent,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceParent {
    None,
    Previous,
    Next,
    Indirect,
    Group,
    ListElement,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourcePathError {
    pub path: String,
    pub owner: PlanningType,
    pub member: String,
    pub reason: String,
}
impl fmt::Display for SourcePathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "source path ({}) on ({}) member ({}): {}",
            self.path,
            self.owner.name(),
            self.member,
            self.reason
        )
    }
}
impl Error for SourcePathError {}
type Getter = dyn for<'v> Fn(&'v dyn Any) -> Vec<&'v dyn Any> + Send + Sync;
struct Member<'s> {
    owner: PlanningType,
    name: String,
    value: PlanningType,
    many: bool,
    role: SourceMemberRole,
    variable: Option<PlanningVariableRef<'s>>,
    get: Box<Getter>,
}
/// Accessors are registered once and type checked against canonical metadata.
/// They must read immutable snapshots. Returned entities borrow the supplied
/// snapshot, never the registry. No unsafe pointers, reflection or cloning of
/// domain objects is used. Null links and empty groups produce no entities.
///
/// ```
/// use timetuck::*;
/// struct Schedule;
/// struct Task { previous: Option<Box<Task>>, finish: i32 }
/// let schema=PlanningSchema::new::<Schedule>(vec![PlanningEntityDefinition::of::<Task>(vec![
///     PlanningVariableDefinition::shadow::<Task>("previous"),
///     PlanningVariableDefinition::shadow::<i32>("finish")])])?;
/// let mut access=SourcePathRegistry::new(&schema);
/// access.one("previous", SourceMemberRole::Previous, |t:&Task|t.previous.as_deref())?;
/// access.one("finish", SourceMemberRole::Declarative, |t:&Task|Some(&t.finish))?;
/// let path=access.compile::<Task>("finish","previous.finish")?;
/// let t=Task { previous: Some(Box::new(Task { previous: None, finish: 4 })), finish: 0 };
/// assert_eq!(path.value_entities(&t)?.len(),1);
/// # Ok::<(),Box<dyn std::error::Error>>(())
/// ```
pub struct SourcePathRegistry<'s> {
    schema: &'s PlanningSchema,
    members: Vec<Member<'s>>,
}
impl<'s> SourcePathRegistry<'s> {
    pub fn new(schema: &'s PlanningSchema) -> Self {
        Self {
            schema,
            members: vec![],
        }
    }
    pub fn is_variable<E: 'static>(&self, name: &str) -> bool {
        self.schema
            .entity::<E>()
            .is_ok_and(|e| e.variable(name).is_ok())
    }
    /// Bind a supplier's optional solution parameter to the registered actual
    /// solution type, validating before any callback executes.
    pub fn solution_supplier<E: 'static, P: 'static, V: 'static>(
        &self,
        target: &str,
        call: impl Fn(&E, &P) -> Option<V> + Send + Sync + 'static,
    ) -> Result<crate::SolutionSourceSupplier<'s, E, P, V>, SourcePathError> {
        let owner = PlanningType::of::<E>();
        let fail = |reason: String| SourcePathError {
            path: target.into(),
            owner,
            member: target.into(),
            reason,
        };
        if PlanningType::of::<P>() != self.schema.solution_type() {
            return Err(fail(format!(
                "Maybe you included a parameter which is not a planning solution ({})?",
                self.schema.solution_type().name()
            )));
        }
        let target = self
            .schema
            .entity::<E>()
            .and_then(|e| e.shadow_variable(target))
            .map_err(|e| fail(e.to_string()))?;
        if target.value_type() != PlanningType::of::<V>() {
            return Err(fail(
                "supplier return type differs from shadow metadata".into(),
            ));
        }
        Ok(crate::SolutionSourceSupplier {
            target,
            call: Box::new(call),
        })
    }
    pub fn one<E: 'static, V: 'static>(
        &mut self,
        name: &str,
        role: SourceMemberRole,
        get: impl for<'v> Fn(&'v E) -> Option<&'v V> + Send + Sync + 'static,
    ) -> Result<(), SourcePathError> {
        self.insert::<E, V>(
            name,
            role,
            false,
            Box::new(move |e| {
                get(e.downcast_ref::<E>().expect("validated accessor owner"))
                    .into_iter()
                    .map(|v| v as &dyn Any)
                    .collect()
            }),
        )
    }
    pub fn many<E: 'static, V: 'static>(
        &mut self,
        name: &str,
        role: SourceMemberRole,
        get: impl for<'v> Fn(&'v E) -> Vec<&'v V> + Send + Sync + 'static,
    ) -> Result<(), SourcePathError> {
        self.insert::<E, V>(
            name,
            role,
            true,
            Box::new(move |e| {
                get(e.downcast_ref::<E>().expect("validated accessor owner"))
                    .into_iter()
                    .map(|v| v as &dyn Any)
                    .collect()
            }),
        )
    }
    fn insert<E: 'static, V: 'static>(
        &mut self,
        name: &str,
        role: SourceMemberRole,
        many: bool,
        get: Box<Getter>,
    ) -> Result<(), SourcePathError> {
        self.insert_erased(
            PlanningType::of::<E>(),
            PlanningType::of::<V>(),
            name,
            role,
            many,
            get,
        )
    }
    /// Install the chosen callable and its genuine projection to canonical schema
    /// metadata. Candidate selection happens once; traversal never reselects.
    pub fn one_selected<E: 'static>(
        &mut self,
        name: &str,
        role: SourceMemberRole,
        selected: crate::SelectedSourceAccessor<E>,
        hierarchy: &crate::SourceTypeHierarchy,
    ) -> Result<(), SourcePathError> {
        let owner = PlanningType::of::<E>();
        let value = self
            .schema
            .entity::<E>()
            .ok()
            .and_then(|e| e.variable(name).ok())
            .map_or(selected.value_type(), |v| v.value_type());
        let read = selected
            .projected(value, hierarchy)
            .map_err(|error| SourcePathError {
                path: name.into(),
                owner,
                member: name.into(),
                reason: error.to_string(),
            })?;
        self.insert_erased(
            owner,
            value,
            name,
            role,
            false,
            Box::new(move |owner| read(owner).into_iter().collect()),
        )
    }
    fn insert_erased(
        &mut self,
        owner: PlanningType,
        value: PlanningType,
        name: &str,
        role: SourceMemberRole,
        many: bool,
        get: Box<Getter>,
    ) -> Result<(), SourcePathError> {
        let fail = |reason: &str| SourcePathError {
            path: name.into(),
            owner,
            member: name.into(),
            reason: reason.into(),
        };
        if name.is_empty() || name.contains(['.', '[', ']']) {
            return Err(fail("invalid member name"));
        }
        if self
            .members
            .iter()
            .any(|m| m.owner == owner && m.name == name)
        {
            return Err(fail("duplicate accessor"));
        }
        let variable = self
            .schema
            .entity_type(owner)
            .ok()
            .and_then(|e| e.variable(name).ok());
        match role {
            SourceMemberRole::Fact | SourceMemberRole::Inconsistent => {
                if variable.is_some() {
                    return Err(fail(
                        "a planning variable cannot be registered as a fact or consistency flag",
                    ));
                }
            }
            _ => {
                let v =
                    variable.ok_or_else(|| fail("variable does not exist in planning schema"))?;
                if v.value_type() != value {
                    return Err(fail("accessor value type differs from planning schema"));
                }
                if many != matches!(v.kind(), PlanningVariableKind::List { .. }) {
                    return Err(fail("accessor cardinality differs from planning schema"));
                }
                if matches!(
                    role,
                    SourceMemberRole::Previous
                        | SourceMemberRole::Next
                        | SourceMemberRole::Declarative
                ) && v.is_genuine()
                {
                    return Err(fail("shadow role requires a shadow variable"));
                }
            }
        }
        self.members.push(Member {
            owner,
            name: name.into(),
            value,
            many,
            role,
            variable,
            get,
        });
        Ok(())
    }
    pub fn compile<E: 'static>(
        &self,
        target: &str,
        path: &str,
    ) -> Result<CompiledSourcePath<'_, 's>, SourcePathError> {
        let root = PlanningType::of::<E>();
        let fail = |owner, member: &str, reason: &str| SourcePathError {
            path: path.into(),
            owner,
            member: member.into(),
            reason: reason.into(),
        };
        let target = self
            .schema
            .entity::<E>()
            .and_then(|e| e.shadow_variable(target))
            .map_err(|e| fail(root, target, &e.to_string()))?;
        let mut owner = root;
        let mut steps = vec![];
        let mut refs = vec![];
        let mut group = None;
        let mut last_fact = false;
        let mut seen_declarative = false;
        let mut parent = SourceParent::None;
        let mut list_variable = None;
        for (index, part) in path.split('.').enumerate() {
            let (name, expand) = part
                .strip_suffix("[]")
                .map_or((part, false), |name| (name, true));
            let member = self
                .members
                .iter()
                .find(|m| m.owner == owner && m.name == name)
                .ok_or_else(|| fail(owner, name, "member does not exist"))?;
            if member.role == SourceMemberRole::Inconsistent {
                return Err(fail(
                    owner,
                    name,
                    "consistency properties cannot be sources; suppliers only run when dependencies are consistent",
                ));
            }
            if member.many != expand {
                return Err(fail(
                    owner,
                    name,
                    "collections require [] followed by a variable; scalar members cannot use []",
                ));
            }
            if expand {
                if group.is_some() {
                    return Err(fail(
                        owner,
                        name,
                        "collection after another collection is not allowed",
                    ));
                }
                if !refs.is_empty() {
                    return Err(fail(
                        owner,
                        name,
                        "collection after a variable is not supported",
                    ));
                }
                if let Some(v) = member.variable {
                    if index != 0 {
                        return Err(fail(
                            owner,
                            name,
                            "planning list variable via another member is not allowed",
                        ));
                    }
                    list_variable = Some(v);
                    parent = SourceParent::ListElement;
                } else {
                    parent = SourceParent::Group;
                }
                group = Some(index);
                last_fact = false;
            } else if member.variable.is_some() {
                if seen_declarative {
                    return Err(fail(
                        owner,
                        name,
                        "variable after a declarative shadow variable is not allowed",
                    ));
                }
                if refs.len() == 2 {
                    return Err(fail(
                        owner,
                        name,
                        "references more than the maximum of 2 variables",
                    ));
                }
                if list_variable.is_some() && member.role != SourceMemberRole::Declarative {
                    return Err(fail(
                        owner,
                        name,
                        "planning list elements require a declarative shadow variable",
                    ));
                }
                if !refs.is_empty() && last_fact && member.role != SourceMemberRole::Declarative {
                    return Err(fail(
                        owner,
                        name,
                        "non-declarative variable accessed via a fact after another variable",
                    ));
                }
                refs.push(index);
                seen_declarative = member.role == SourceMemberRole::Declarative;
                last_fact = false;
            } else {
                if last_fact {
                    return Err(fail(owner, name, "multiple facts in a row are not allowed"));
                }
                last_fact = true;
            }
            steps.push(member);
            owner = member.value;
        }
        if refs.is_empty() {
            return Err(fail(root, path, "does not reference any variables"));
        }
        if refs.last().copied() != Some(steps.len() - 1) {
            return Err(fail(owner, path, "does not end on a variable"));
        }
        if group.is_none() && steps.len() > 1 {
            parent = match steps[0].role {
                SourceMemberRole::Previous => SourceParent::Previous,
                SourceMemberRole::Next => SourceParent::Next,
                _ => SourceParent::Indirect,
            };
        }
        Ok(CompiledSourcePath {
            root,
            target,
            path: path.into(),
            steps,
            refs,
            group,
            parent,
            list_variable,
        })
    }
}
/// A compiled source resolves entities owning the referenced variables, not the
/// value of the final field. Terminal getters are therefore never called.
/// The plan can be reused across immutable snapshots and handles null links.
pub struct CompiledSourcePath<'r, 's> {
    root: PlanningType,
    target: PlanningVariableRef<'s>,
    path: String,
    steps: Vec<&'r Member<'s>>,
    refs: Vec<usize>,
    group: Option<usize>,
    parent: SourceParent,
    list_variable: Option<PlanningVariableRef<'s>>,
}
impl<'r, 's> CompiledSourcePath<'r, 's> {
    pub fn root_type(&self) -> PlanningType {
        self.root
    }
    pub fn parent(&self) -> SourceParent {
        self.parent
    }
    pub fn list_variable(&self) -> Option<PlanningVariableRef<'s>> {
        self.list_variable
    }
    pub fn target(&self) -> PlanningVariableRef<'s> {
        self.target
    }
    pub fn references(&self) -> impl ExactSizeIterator<Item = SourceReference<'_, 'r, 's>> {
        (0..self.refs.len()).map(|index| SourceReference {
            source: self,
            index,
        })
    }
    fn walk<'v>(
        &self,
        start: &'v dyn Any,
        expected: PlanningType,
        range: std::ops::Range<usize>,
    ) -> Result<Vec<&'v dyn Any>, SourcePathError> {
        if start.type_id() != expected.id() {
            return Err(SourcePathError {
                path: self.path.clone(),
                owner: expected,
                member: String::new(),
                reason: "wrong traversal root type".into(),
            });
        }
        let mut current = vec![start];
        for i in range {
            current = current
                .into_iter()
                .flat_map(|v| (self.steps[i].get)(v))
                .collect();
        }
        Ok(current)
    }
    pub fn value_entities<'v>(
        &self,
        root: &'v dyn Any,
    ) -> Result<Vec<&'v dyn Any>, SourcePathError> {
        self.walk(root, self.root, 0..self.steps.len() - 1)
    }
    /// Map actual variable/entity references to caller-owned stable keys for
    /// DependencyGraph, DependencyTopology or DependencyCache. Return None to
    /// omit a source (for example a built-in field used only for invalidation).
    /// Keys are deduplicated and sorted; duplicate group members remain visible
    /// through value_entities/references, but never duplicate graph edges.
    pub fn dependency_keys<K: Ord>(
        &self,
        root: &dyn Any,
        mut key: impl FnMut(PlanningVariableRef<'s>, &dyn Any) -> Option<K>,
    ) -> Result<Vec<K>, SourcePathError> {
        let mut keys = BTreeSet::new();
        for reference in self.references() {
            for entity in reference.entities(root)? {
                if let Some(k) = key(reference.variable(), entity) {
                    keys.insert(k);
                }
            }
        }
        Ok(keys.into_iter().collect())
    }
}
#[derive(Clone, Copy)]
pub struct SourceReference<'a, 'r, 's> {
    source: &'a CompiledSourcePath<'r, 's>,
    index: usize,
}
impl<'a, 'r, 's> SourceReference<'a, 'r, 's> {
    fn step(self) -> usize {
        self.source.refs[self.index]
    }
    pub fn variable(self) -> PlanningVariableRef<'s> {
        self.source.steps[self.step()]
            .variable
            .expect("compiled variable")
    }
    pub fn target(self) -> PlanningVariableRef<'s> {
        self.source.target
    }
    pub fn is_top_level(self) -> bool {
        self.index == 0
    }
    pub fn is_bottom_level(self) -> bool {
        self.index + 1 == self.source.refs.len()
    }
    pub fn on_root(self) -> bool {
        self.step() == 0
    }
    pub fn is_declarative(self) -> bool {
        self.source.steps[self.step()].role == SourceMemberRole::Declarative
    }
    pub fn downstream_declarative(self) -> Option<PlanningVariableRef<'s>> {
        self.source.steps[self.step()..]
            .iter()
            .find(|m| m.role == SourceMemberRole::Declarative)
            .and_then(|m| m.variable)
    }
    pub fn chain(self) -> Vec<&'r str> {
        let begin = self.source.group.map_or(0, |i| i + 1);
        self.source.steps[begin..self.step()]
            .iter()
            .map(|m| m.name.as_str())
            .collect()
    }
    pub fn entities(self, root: &dyn Any) -> Result<Vec<&dyn Any>, SourcePathError> {
        self.source.walk(root, self.source.root, 0..self.step())
    }
    pub fn target_entities_from_variable(
        self,
        entity: &dyn Any,
    ) -> Result<Vec<&dyn Any>, SourcePathError> {
        self.source.walk(
            entity,
            self.variable().entity().entity_type(),
            self.step()..self.source.steps.len() - 1,
        )
    }
}

mod structure;
pub use structure::{ShadowDirection, ShadowSourceEntity, ShadowStructure};
