//! Owned employee datasets consumed by a real native constraint solver.
use crate::*;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    time::SystemTime,
};
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScheduleEmployee {
    pub id: String,
    pub skills: BTreeSet<String>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmployeeShift {
    pub id: String,
    pub required_skill: String,
    pub start: i64,
    pub end: i64,
    pub employee: Option<String>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeEmployeeSchedule {
    pub employees: Vec<ScheduleEmployee>,
    pub shifts: Vec<EmployeeShift>,
}
pub const SHIFT_END_BEFORE_START: &str = "SHIFT_END_BEFORE_START";
pub const NATIVE_SHIFT_END_MESSAGE: &str = "Shift end must not precede shift start.";
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmployeeShiftEndBeforeStartIssue {
    pub shift_id: String,
    pub start: i64,
    pub end: i64,
}
impl EmployeeShiftEndBeforeStartIssue {
    pub fn code(&self) -> &'static str {
        SHIFT_END_BEFORE_START
    }
    fn service_issue(&self) -> ServiceValidationIssue {
        ServiceValidationIssue::new(
            self.code(),
            ValidationSeverity::Error,
            vec![
                ("shiftId".into(), json!(self.shift_id)),
                ("startTime".into(), json!(self.start)),
                ("endTime".into(), json!(self.end)),
            ],
        )
        .expect("fixed unique payload keys")
        .with_legacy_message(NATIVE_SHIFT_END_MESSAGE)
    }
}
#[derive(Clone, Debug)]
pub struct EmployeeScheduleValidation {
    pub issues: Vec<EmployeeShiftEndBeforeStartIssue>,
    result: ServiceValidationResult,
}
impl EmployeeScheduleValidation {
    pub fn status(&self) -> ServiceValidationStatus {
        self.result.status
    }
    pub fn to_json(&self) -> Value {
        self.result.to_json()
    }
    pub fn service_result(&self) -> &ServiceValidationResult {
        &self.result
    }
}
impl NativeEmployeeSchedule {
    pub fn assignment_penalty(&self) -> usize {
        self.shifts.iter().filter(|s| s.employee.is_none()).count()
    }
    pub fn constraint_score(&self) -> Result<HardMediumSoftScore, String> {
        let problem = self.model()?;
        let network = self.network();
        let director = ScoreDirector::new(&problem, &network, problem.initial_assignment())
            .map_err(|e| e.to_string())?;
        Ok(director.score())
    }
    pub fn score(&self) -> HardMediumSoftScore {
        HardMediumSoftScore::new(0, -(self.assignment_penalty() as i64), 0)
    }
    pub fn validation(&self) -> EmployeeScheduleValidation {
        let issues = self
            .shifts
            .iter()
            .filter(|s| s.end < s.start)
            .map(|s| EmployeeShiftEndBeforeStartIssue {
                shift_id: s.id.clone(),
                start: s.start,
                end: s.end,
            })
            .collect::<Vec<_>>();
        let mut builder = ServiceValidationBuilder::new();
        for issue in &issues {
            builder = builder.add_issue(issue.service_issue());
        }
        EmployeeScheduleValidation {
            issues,
            result: builder.build(),
        }
    }
    fn json(&self) -> Value {
        json!({"employees":self.employees.iter().map(|e|json!({"id":e.id,"skills":e.skills})).collect::<Vec<_>>(),"shifts":self.shifts.iter().map(|s|json!({"id":s.id,"skill":s.required_skill,"start":s.start,"end":s.end,"employee":s.employee})).collect::<Vec<_>>()})
    }
    fn decode(value: &Value) -> Result<Self, String> {
        let string = |v: &Value| {
            v.as_str()
                .map(str::to_owned)
                .ok_or_else(|| "string field required".to_string())
        };
        let employees = value["employees"]
            .as_array()
            .ok_or("employees required")?
            .iter()
            .map(|e| {
                Ok(ScheduleEmployee {
                    id: string(&e["id"])?,
                    skills: e["skills"]
                        .as_array()
                        .ok_or("skills required")?
                        .iter()
                        .map(&string)
                        .collect::<Result<_, _>>()?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let shifts = value["shifts"]
            .as_array()
            .ok_or("shifts required")?
            .iter()
            .map(|s| {
                Ok(EmployeeShift {
                    id: string(&s["id"])?,
                    required_skill: string(&s["skill"])?,
                    start: s["start"].as_i64().ok_or("start required")?,
                    end: s["end"].as_i64().ok_or("end required")?,
                    employee: if s["employee"].is_null() {
                        None
                    } else {
                        Some(string(&s["employee"])?)
                    },
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(Self { employees, shifts })
    }
    fn model(&self) -> Result<FiniteProblem<usize>, String> {
        let mut employees = BTreeSet::new();
        let mut shifts = BTreeSet::new();
        for e in &self.employees {
            if !employees.insert(&e.id) {
                return Err("duplicate employee id".into());
            }
        }
        let mut vars = Vec::new();
        for s in &self.shifts {
            if !shifts.insert(&s.id) {
                return Err("duplicate shift id".into());
            }
            let domain = self
                .employees
                .iter()
                .enumerate()
                .filter(|(_, e)| e.skills.contains(&s.required_skill))
                .map(|(i, _)| i)
                .collect();
            let mut v = Variable::new(domain);
            if let Some(id) = &s.employee {
                v = v.with_initial(
                    self.employees
                        .iter()
                        .position(|e| &e.id == id)
                        .ok_or("unknown assigned employee")?,
                );
            }
            vars.push(v);
        }
        FiniteProblem::new(vars).map_err(|e| e.to_string())
    }
    fn network(&self) -> ConstraintNetwork<'static, usize, HardMediumSoftScore> {
        let mut n = ConstraintNetwork::new(self.shifts.len());
        for slot in 0..self.shifts.len() {
            n.add_constraint(format!("assignShiftToEmployee:{slot}"), vec![slot], |v| {
                HardMediumSoftScore::new(0, -i64::from(v[0].is_none()), 0)
            })
            .expect("valid slot");
        }
        n
    }
    fn assigned(&self, a: &Assignment<usize>) -> Self {
        let mut out = self.clone();
        for (s, v) in out.shifts.iter_mut().zip(a.values()) {
            s.employee = v.map(|i| out.employees[i].id.clone());
        }
        out
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EmployeeDatasetStatus {
    Computed,
    SolvingStarted,
    SolvingActive,
    Completed,
    Failed,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EmployeeResourceEventKind {
    Computed,
    Best,
    Initialized,
    Final,
}
#[derive(Clone, Debug)]
pub struct EmployeeResourceEvent {
    pub id: String,
    pub kind: EmployeeResourceEventKind,
    pub producer: Option<String>,
    pub solve_requested: bool,
    pub model: NativeEmployeeSchedule,
    pub score: HardMediumSoftScore,
    pub weights: BTreeMap<String, HardMediumSoftScore>,
}
#[derive(Clone, Debug)]
pub struct EmployeeScheduleResponse {
    pub id: String,
    pub status: EmployeeDatasetStatus,
    pub model: NativeEmployeeSchedule,
    pub score: HardMediumSoftScore,
    pub validation: EmployeeScheduleValidation,
    pub weights: BTreeMap<String, HardMediumSoftScore>,
    pub error: Option<String>,
}
impl EmployeeScheduleResponse {
    pub fn employee_count(&self) -> usize {
        self.model.employees.len()
    }
    pub fn shift_count(&self) -> usize {
        self.model.shifts.len()
    }
    pub fn assigned_shifts(&self) -> usize {
        self.model.shifts.len() - self.model.assignment_penalty()
    }
}
#[derive(Default)]
pub struct EmployeeResourceCommandQueue(VecDeque<WorkerDatasetEvent>);
impl WorkerDatasetEventSink for EmployeeResourceCommandQueue {
    fn emit(&mut self, event: WorkerDatasetEvent) -> Result<(), String> {
        self.0.push_back(event);
        Ok(())
    }
}
type EmployeeFacade = WorkerDatasetFacade<
    InMemoryWorkerDatasetStore,
    EmployeeResourceCommandQueue,
    fn() -> SystemTime,
>;
/// Synchronous owned command consumer; publishing alone never completes solving.
/// Native skill-domain filtering is explicit policy. Only assignedness constraint
/// behavior is sourced from the provider tests; other production rules are unknown.
/// Events publish initial score, actual initialization, and coalesced phase bests,
/// never a fixture event table. HTTP/CDI/SSE transport is outside this API.
pub struct EmployeeScheduleResource {
    facade: EmployeeFacade,
    records: BTreeMap<String, EmployeeScheduleResponse>,
    events: Vec<EmployeeResourceEvent>,
    watches: crate::employee_schedule_watching::EmployeeWatchHub,
}
impl Default for EmployeeScheduleResource {
    fn default() -> Self {
        Self::new()
    }
}
impl EmployeeScheduleResource {
    pub fn new() -> Self {
        Self {
            facade: WorkerDatasetFacade::new(
                InMemoryWorkerDatasetStore::default(),
                EmployeeResourceCommandQueue::default(),
                SystemTime::now as fn() -> SystemTime,
            ),
            records: BTreeMap::new(),
            events: Vec::new(),
            watches: crate::employee_schedule_watching::EmployeeWatchHub::default(),
        }
    }
    pub fn subscribe(
        &self,
        id: &str,
        capacity: usize,
    ) -> Result<crate::EmployeeSubscription, crate::EmployeeWatchError> {
        let record = self
            .records
            .get(id)
            .ok_or(crate::EmployeeWatchError::NotFound)?;
        self.watches.subscribe(record, capacity)
    }
    pub fn active_subscription_count(&self) -> usize {
        self.watches.count()
    }
    pub fn events(&self) -> &[EmployeeResourceEvent] {
        &self.events
    }
    pub fn get(&self, id: &str) -> Result<EmployeeScheduleResponse, String> {
        self.records
            .get(id)
            .cloned()
            .ok_or_else(|| "dataset not found".into())
    }
    pub fn issue_types(&self) -> Vec<ServiceValidationIssueType> {
        vec![ServiceValidationIssueType {
            code: SHIFT_END_BEFORE_START.into(),
            severity: ValidationSeverity::Error,
            message: NATIVE_SHIFT_END_MESSAGE.into(),
        }]
    }
    pub fn issue_type(&self, code: &str) -> Option<ServiceValidationIssueType> {
        self.issue_types().into_iter().find(|t| t.code == code)
    }
    pub fn post(
        &mut self,
        schedule: &NativeEmployeeSchedule,
        solve_requested: bool,
    ) -> Result<EmployeeScheduleResponse, String> {
        let metadata = self
            .facade
            .create_dataset(
                "Employee schedule".into(),
                BTreeSet::new(),
                schedule.json(),
                json!({}),
                solve_requested,
            )
            .map_err(|e| e.to_string())?;
        self.consume()?;
        self.get(&metadata.id)
    }
    pub fn solve_existing(&mut self, id: &str) -> Result<EmployeeScheduleResponse, String> {
        self.facade.solve_dataset(id).map_err(|e| e.to_string())?;
        self.consume()?;
        self.get(id)
    }
    fn event(
        &mut self,
        record: &EmployeeScheduleResponse,
        kind: EmployeeResourceEventKind,
        producer: Option<String>,
        solve_requested: bool,
    ) {
        self.watches.publish(record);
        self.events.push(EmployeeResourceEvent {
            id: record.id.clone(),
            kind,
            producer,
            solve_requested,
            model: record.model.clone(),
            score: record.score,
            weights: record.weights.clone(),
        });
    }
    fn consume(&mut self) -> Result<(), String> {
        while let Some(command) = self.facade.sink_mut().0.pop_front() {
            match command {
                WorkerDatasetEvent::ValidateCompute { id, solve } => {
                    let dataset = self.facade.get_dataset(&id).map_err(|e| e.to_string())?;
                    let model = NativeEmployeeSchedule::decode(&dataset.input)?;
                    let record = EmployeeScheduleResponse {
                        id: id.clone(),
                        status: EmployeeDatasetStatus::Computed,
                        score: model.score(),
                        validation: model.validation(),
                        model,
                        weights: BTreeMap::from([(
                            "assignShiftToEmployee".into(),
                            HardMediumSoftScore::new(0, 1, 0),
                        )]),
                        error: None,
                    };
                    self.facade
                        .store_mut()
                        .set_output(&id, record.model.json())
                        .map_err(|e| e.to_string())?;
                    self.event(&record, EmployeeResourceEventKind::Computed, None, solve);
                    self.records.insert(id.clone(), record);
                    if solve {
                        self.run(&id)?;
                    }
                }
                WorkerDatasetEvent::SolveStart { id } => self.run(&id)?,
                _ => {}
            }
        }
        Ok(())
    }
    fn run(&mut self, id: &str) -> Result<(), String> {
        let mut record = self.get(id)?;
        if record.validation.status() == ServiceValidationStatus::Errors {
            return Err("dataset validation failed".into());
        }
        record.status = EmployeeDatasetStatus::SolvingStarted;
        self.records.insert(id.into(), record.clone());
        self.event(
            &record,
            EmployeeResourceEventKind::Best,
            Some("Solving started".into()),
            true,
        );
        let result = (|| {
            let problem = record.model.model()?;
            let network = record.model.network();
            let model = record.model.clone();
            let mut phase_bests = BTreeMap::new();
            let mut initialized = Vec::new();
            let result = solve_with_events(
                &problem,
                &network,
                SolverConfig::<HardMediumSoftScore>::default(),
                |e| match e {
                    SolverEvent::Best(update) => {
                        phase_bests.insert(
                            update.phase,
                            (update.kind, model.assigned(update.assignment), update.score),
                        );
                    }
                    SolverEvent::FirstInitialized {
                        initialization,
                        assignment,
                        score,
                        ..
                    } => {
                        if let Some(score) = score {
                            initialized.push((initialization, model.assigned(assignment), score));
                        }
                    }
                },
            )
            .map_err(|e| e.to_string())?;
            Ok::<_, String>((result, phase_bests, initialized))
        })();
        match result {
            Err(error) => {
                record.status = EmployeeDatasetStatus::Failed;
                record.error = Some(error.clone());
                self.watches.publish(&record);
                self.records.insert(id.into(), record);
                Err(error)
            }
            Ok((result, phase_bests, initialized)) => {
                record.status = EmployeeDatasetStatus::SolvingActive;
                for (i, model, score) in initialized {
                    let mut snapshot = record.clone();
                    snapshot.model = model;
                    snapshot.score = score;
                    self.event(
                        &snapshot,
                        EmployeeResourceEventKind::Initialized,
                        Some(stage_name(i.kind, i.phase)),
                        true,
                    );
                }
                let mut last = record.score;
                for (phase, (kind, model, score)) in phase_bests {
                    if score > last {
                        let mut snapshot = record.clone();
                        snapshot.model = model;
                        snapshot.score = score;
                        self.event(
                            &snapshot,
                            EmployeeResourceEventKind::Best,
                            Some(stage_name(kind, phase)),
                            true,
                        );
                        last = score;
                    }
                }
                let Some((assignment, score)) = result.solution() else {
                    let error = "solver produced no solution".to_string();
                    record.status = EmployeeDatasetStatus::Failed;
                    record.error = Some(error.clone());
                    self.watches.publish(&record);
                    self.records.insert(id.into(), record);
                    return Err(error);
                };
                record.model = record.model.assigned(assignment);
                record.score = score;
                record.status = EmployeeDatasetStatus::Completed;
                self.facade
                    .store_mut()
                    .set_output(id, record.model.json())
                    .map_err(|e| e.to_string())?;
                self.event(&record, EmployeeResourceEventKind::Final, None, true);
                self.records.insert(id.into(), record);
                Ok(())
            }
        }
    }
}
fn stage_name(kind: SolverPhaseKind, index: usize) -> String {
    if kind == SolverPhaseKind::Construction {
        format!("Construction Heuristic ({index})")
    } else {
        format!("{kind:?} ({index})")
    }
}
