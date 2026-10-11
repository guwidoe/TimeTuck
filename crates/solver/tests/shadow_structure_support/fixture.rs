// Adapted classification fixture declarations; Apache-2.0. See provenance.
#[derive(Default)]
pub struct Value {
    pub id: &'static str,
    pub previous: Option<Box<Value>>,
    pub next: Option<Box<Value>>,
    pub group: Vec<GroupMember>,
    pub owner: Option<Box<Owner>>,
    pub leader: Option<Box<Leader>>,
    pub number: i32,
    pub code_sum: String,
}
pub enum GroupMember { This, Other(Box<Value>) }
#[derive(Default)]
pub struct Owner { pub values: Vec<Value>, pub number: i32 }
#[derive(Default)]
pub struct Leader { pub value: i32 }
pub struct Base;
pub struct Solution;
#[derive(Clone, Copy)]
pub enum Fixture { Simple, Extended, Concurrent, Follower, MultiEntity, MultiDirectional, ListElement }
pub fn value(id: &'static str) -> Value { Value { id, ..Default::default() } }
pub fn schema(f: Fixture) -> PlanningSchema {
    let mut fields = vec![
        PlanningVariableDefinition::shadow::<Value>("previous"),
        PlanningVariableDefinition::shadow::<Value>("previousValue"),
        PlanningVariableDefinition::shadow::<Value>("nextValue"),
        PlanningVariableDefinition::shadow::<Owner>("entity"),
    ];
    for name in ["startTime", "endTime", "serviceReadyTime", "serviceStartTime", "serviceFinishTime", "value"] {
        fields.push(PlanningVariableDefinition::shadow::<i32>(name));
    }
    fields.push(PlanningVariableDefinition::shadow::<String>("codeSum"));
    let mut owner = vec![PlanningVariableDefinition::list::<Value>("values", true)];
    if matches!(f, Fixture::MultiEntity) {
        owner.push(PlanningVariableDefinition::basic::<i32>("delay", false));
        owner.push(PlanningVariableDefinition::shadow::<i32>("readyTime"));
    }
    if matches!(f, Fixture::ListElement) { owner.push(PlanningVariableDefinition::shadow::<i32>("lastEndTime")); }
    PlanningSchema::new::<Solution>(vec![
        PlanningEntityDefinition::of::<Value>(fields),
        PlanningEntityDefinition::of::<Owner>(owner),
        PlanningEntityDefinition::of::<Leader>(vec![PlanningVariableDefinition::basic::<i32>("value", false)]),
        PlanningEntityDefinition::of::<Base>(vec![]),
    ]).unwrap()
}
pub fn access(s: &PlanningSchema, f: Fixture) -> SourcePathRegistry<'_> {
    let mut a = SourcePathRegistry::new(s);
    for name in ["previous", "previousValue"] {
        a.one(name, SourceMemberRole::Previous, |v: &Value| v.previous.as_deref()).unwrap();
    }
    a.one("nextValue", SourceMemberRole::Next, |v: &Value| v.next.as_deref()).unwrap();
    a.one("entity", SourceMemberRole::Variable, |v: &Value| v.owner.as_deref()).unwrap();
    a.one("leader", SourceMemberRole::Fact, |v: &Value| v.leader.as_deref()).unwrap();
    for name in ["concurrentValueGroup", "dependencies"] {
        a.many(name, SourceMemberRole::Fact, |v: &Value| v.group.iter().map(|member| match member {
            GroupMember::This => v,
            GroupMember::Other(other) => other.as_ref(),
        }).collect()).unwrap();
    }
    for name in ["startTime", "endTime", "serviceReadyTime", "serviceStartTime", "serviceFinishTime", "value"] {
        a.one(name, SourceMemberRole::Declarative, |v: &Value| Some(&v.number)).unwrap();
    }
    a.one("codeSum", SourceMemberRole::Declarative, |v: &Value| Some(&v.code_sum)).unwrap();
    a.many("values", SourceMemberRole::Variable, |o: &Owner| o.values.iter().collect()).unwrap();
    if matches!(f, Fixture::MultiEntity) {
        a.one("delay", SourceMemberRole::Variable, |o: &Owner| Some(&o.number)).unwrap();
        a.one("readyTime", SourceMemberRole::Declarative, |o: &Owner| Some(&o.number)).unwrap();
    }
    if matches!(f, Fixture::ListElement) {
        a.one("lastEndTime", SourceMemberRole::Declarative, |o: &Owner| Some(&o.number)).unwrap();
    }
    a.one("value", SourceMemberRole::Variable, |l: &Leader| Some(&l.value)).unwrap();
    a
}
pub fn paths<'r, 's>(a: &'r SourcePathRegistry<'s>, f: Fixture) -> (Vec<CompiledSourcePath<'r, 's>>, Vec<CompiledSourcePath<'r, 's>>) {
    let specs: &[(&str, &str)] = match f {
        Fixture::Simple => &[("startTime", "entity"), ("startTime", "previous.endTime"), ("endTime", "startTime")],
        Fixture::Extended => &[("codeSum", "previous.codeSum")],
        Fixture::Concurrent => &[("serviceReadyTime", "previousValue.serviceFinishTime"), ("serviceReadyTime", "entity"),
            ("serviceStartTime", "serviceReadyTime"), ("serviceStartTime", "concurrentValueGroup[].serviceReadyTime"),
            ("serviceFinishTime", "serviceStartTime")],
        Fixture::Follower => &[("value", "leader.value")],
        Fixture::MultiEntity => &[("startTime", "dependencies[].endTime"), ("startTime", "previousValue.endTime"),
            ("startTime", "entity.readyTime"), ("endTime", "startTime")],
        Fixture::MultiDirectional => &[("serviceReadyTime", "previousValue.serviceFinishTime"), ("serviceReadyTime", "entity"),
            ("serviceStartTime", "serviceReadyTime"), ("serviceStartTime", "concurrentValueGroup[].nextValue"),
            ("serviceFinishTime", "serviceStartTime")],
        Fixture::ListElement => &[("startTime", "entity"), ("startTime", "previous"), ("startTime", "previous.endTime"), ("endTime", "startTime")],
    };
    let values = specs.iter().map(|(target, source)| a.compile::<Value>(target, source).unwrap()).collect();
    let owners = match f {
        Fixture::MultiEntity => vec![a.compile::<Owner>("readyTime", "delay").unwrap()],
        Fixture::ListElement => vec![a.compile::<Owner>("lastEndTime", "values[].endTime").unwrap()],
        _ => vec![],
    };
    (values, owners)
}
