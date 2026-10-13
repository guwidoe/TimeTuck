//! Native subscriptions and route dispatch; no HTTP/CDI/SSE wire ABI claim.
use crate::{
    EmployeeDatasetStatus, EmployeeScheduleResource, EmployeeScheduleResponse, HardMediumSoftScore,
    NativeEmployeeSchedule,
};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    collections::{BTreeMap, VecDeque},
    rc::{Rc, Weak},
};
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmployeeWatchMetadata {
    pub id: String,
    pub status: EmployeeDatasetStatus,
    pub score: HardMediumSoftScore,
    pub error: Option<String>,
}
impl From<&EmployeeScheduleResponse> for EmployeeWatchMetadata {
    fn from(r: &EmployeeScheduleResponse) -> Self {
        Self {
            id: r.id.clone(),
            status: r.status,
            score: r.score,
            error: r.error.clone(),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EmployeeWatchError {
    NotFound,
    Gone,
    InvalidCapacity,
    SubscriberLimit,
    Overflow,
}
impl EmployeeWatchError {
    pub fn status_code(&self) -> u16 {
        match self {
            Self::NotFound => 404,
            Self::Gone => 410,
            Self::InvalidCapacity => 400,
            Self::SubscriberLimit => 429,
            Self::Overflow => 503,
        }
    }
}
struct WatchQueue {
    capacity: usize,
    items: VecDeque<EmployeeWatchMetadata>,
    closed: bool,
    error: Option<EmployeeWatchError>,
}
struct WatchSlot {
    id: String,
    queue: Rc<RefCell<WatchQueue>>,
}
#[derive(Default)]
struct WatchState {
    serial: usize,
    slots: BTreeMap<usize, WatchSlot>,
}
#[derive(Default)]
pub(crate) struct EmployeeWatchHub(Rc<RefCell<WatchState>>);
impl Drop for EmployeeWatchHub {
    fn drop(&mut self) {
        for slot in self.0.borrow_mut().slots.values() {
            slot.queue.borrow_mut().closed = true;
        }
    }
}
impl EmployeeWatchHub {
    pub(crate) fn subscribe(
        &self,
        record: &EmployeeScheduleResponse,
        capacity: usize,
    ) -> Result<EmployeeSubscription, EmployeeWatchError> {
        if capacity == 0 || capacity > 4096 {
            return Err(EmployeeWatchError::InvalidCapacity);
        }
        if matches!(
            record.status,
            EmployeeDatasetStatus::Completed | EmployeeDatasetStatus::Failed
        ) {
            return Err(EmployeeWatchError::Gone);
        }
        let mut hub = self.0.borrow_mut();
        if hub.slots.len() >= 256 {
            return Err(EmployeeWatchError::SubscriberLimit);
        }
        hub.serial = hub
            .serial
            .checked_add(1)
            .ok_or(EmployeeWatchError::SubscriberLimit)?;
        let serial = hub.serial;
        let queue = Rc::new(RefCell::new(WatchQueue {
            capacity,
            items: VecDeque::from([EmployeeWatchMetadata::from(record)]),
            closed: false,
            error: None,
        }));
        hub.slots.insert(
            serial,
            WatchSlot {
                id: record.id.clone(),
                queue: queue.clone(),
            },
        );
        Ok(EmployeeSubscription {
            hub: Rc::downgrade(&self.0),
            serial,
            queue,
        })
    }
    pub(crate) fn publish(&self, record: &EmployeeScheduleResponse) {
        let mut hub = self.0.borrow_mut();
        if hub.slots.is_empty() {
            return;
        }
        let terminal = matches!(
            record.status,
            EmployeeDatasetStatus::Completed | EmployeeDatasetStatus::Failed
        );
        hub.slots.retain(|_, slot| {
            if slot.id != record.id {
                return true;
            }
            let mut q = slot.queue.borrow_mut();
            if q.items.len() == q.capacity {
                q.error = Some(EmployeeWatchError::Overflow);
                q.closed = true;
                return false;
            }
            q.items.push_back(EmployeeWatchMetadata::from(record));
            q.closed = terminal;
            !terminal
        });
    }
    pub(crate) fn count(&self) -> usize {
        self.0.borrow().slots.len()
    }
}
pub struct EmployeeSubscription {
    hub: Weak<RefCell<WatchState>>,
    serial: usize,
    queue: Rc<RefCell<WatchQueue>>,
}
impl EmployeeSubscription {
    pub fn next_event(&mut self) -> Result<Option<EmployeeWatchMetadata>, EmployeeWatchError> {
        let mut q = self.queue.borrow_mut();
        if let Some(e) = q.error.take() {
            return Err(e);
        }
        Ok(q.items.pop_front())
    }
    pub fn is_closed(&self) -> bool {
        self.queue.borrow().closed
    }
    pub fn pending_len(&self) -> usize {
        self.queue.borrow().items.len()
    }
    pub fn cancel(&mut self) {
        if let Some(hub) = self.hub.upgrade() {
            hub.borrow_mut().slots.remove(&self.serial);
        }
        let mut q = self.queue.borrow_mut();
        q.items.clear();
        q.error = None;
        q.closed = true;
    }
}
impl Drop for EmployeeSubscription {
    fn drop(&mut self) {
        self.cancel();
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EmployeeRouteOperation {
    Schedule,
    SolveDataset,
    FromInput,
    FromPatch,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmployeeRouteDefinition {
    pub operation: EmployeeRouteOperation,
    pub operation_id: &'static str,
    pub path: String,
    pub priority_parameter: &'static str,
}
#[derive(Clone, Debug)]
pub enum EmployeeRouteCommand {
    Schedule(NativeEmployeeSchedule),
    SolveDataset(String),
    FromInput(NativeEmployeeSchedule),
    FromPatch {
        id: String,
        assignments: BTreeMap<String, Option<String>>,
    },
}
impl EmployeeRouteCommand {
    fn operation(&self) -> EmployeeRouteOperation {
        match self {
            Self::Schedule(_) => EmployeeRouteOperation::Schedule,
            Self::SolveDataset(_) => EmployeeRouteOperation::SolveDataset,
            Self::FromInput(_) => EmployeeRouteOperation::FromInput,
            Self::FromPatch { .. } => EmployeeRouteOperation::FromPatch,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmployeeRouteReceipt {
    pub sequence: u64,
    pub priority: i32,
    pub status_code: u16,
}
struct Pending {
    receipt: EmployeeRouteReceipt,
    command: EmployeeRouteCommand,
}
pub struct EmployeeRouteDispatcher {
    routes: Vec<EmployeeRouteDefinition>,
    pending: Vec<Pending>,
    serial: u64,
    capacity: usize,
}
impl EmployeeRouteDispatcher {
    pub fn new(version: &str, capacity: usize) -> Result<Self, String> {
        if version.is_empty() || version.contains('/') || capacity == 0 {
            return Err(
                "Version must be a nonempty path segment and queue capacity positive".into(),
            );
        }
        Ok(Self {
            routes: [
                (EmployeeRouteOperation::Schedule, "schedule", "/schedules"),
                (
                    EmployeeRouteOperation::SolveDataset,
                    "solveDataset",
                    "/schedules/{id}",
                ),
                (
                    EmployeeRouteOperation::FromInput,
                    "fromInput",
                    "/schedules/from-input",
                ),
                (
                    EmployeeRouteOperation::FromPatch,
                    "fromPatch",
                    "/schedules/{id}/from-patch",
                ),
            ]
            .into_iter()
            .map(|(operation, operation_id, path)| EmployeeRouteDefinition {
                operation,
                operation_id,
                path: format!("/{version}{path}"),
                priority_parameter: "priority",
            })
            .collect(),
            pending: vec![],
            serial: 0,
            capacity,
        })
    }
    pub fn routes(&self) -> &[EmployeeRouteDefinition] {
        &self.routes
    }
    pub fn openapi(&self) -> Value {
        let mut paths = serde_json::Map::new();
        for route in &self.routes {
            paths.insert(route.path.clone(),json!({"post":{"operationId":route.operation_id,"parameters":[{"name":route.priority_parameter,"in":"query","schema":{"type":"integer","format":"int32"}}]}}));
        }
        json!({"openapi":"3.0.3","paths":paths})
    }
    pub fn enqueue(
        &mut self,
        method: &str,
        path: &str,
        query: &BTreeMap<String, String>,
        command: EmployeeRouteCommand,
    ) -> Result<EmployeeRouteReceipt, String> {
        if method != "POST" {
            return Err("Route not found".into());
        }
        let route = self
            .routes
            .iter()
            .find(|r| r.operation == command.operation())
            .ok_or("Route not found")?;
        let expected = match &command {
            EmployeeRouteCommand::SolveDataset(id) | EmployeeRouteCommand::FromPatch { id, .. } => {
                if id.is_empty() || id.contains('/') {
                    return Err("Invalid dataset route id".into());
                }
                route.path.replace("{id}", id)
            }
            _ => route.path.clone(),
        };
        if path != expected {
            return Err("Route does not match command".into());
        }
        let priority = query
            .get(route.priority_parameter)
            .map(|v| {
                v.parse::<i32>()
                    .map_err(|_| "Priority must be int32".to_string())
            })
            .transpose()?
            .unwrap_or(0);
        if self.pending.len() == self.capacity {
            return Err("Route queue capacity exceeded".into());
        }
        self.serial = self.serial.checked_add(1).ok_or("Sequence overflow")?;
        let receipt = EmployeeRouteReceipt {
            sequence: self.serial,
            priority,
            status_code: 202,
        };
        self.pending.push(Pending {
            receipt: receipt.clone(),
            command,
        });
        Ok(receipt)
    }
    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }
    pub fn execute_next(
        &mut self,
        resource: &mut EmployeeScheduleResource,
    ) -> Option<(
        EmployeeRouteReceipt,
        Result<EmployeeScheduleResponse, String>,
    )> {
        let index = self
            .pending
            .iter()
            .enumerate()
            .max_by_key(|(_, p)| (p.receipt.priority, std::cmp::Reverse(p.receipt.sequence)))
            .map(|(i, _)| i)?;
        let p = self.pending.remove(index);
        let result = match p.command {
            EmployeeRouteCommand::Schedule(model) | EmployeeRouteCommand::FromInput(model) => {
                resource.post(&model, true)
            }
            EmployeeRouteCommand::SolveDataset(id) => resource.solve_existing(&id),
            EmployeeRouteCommand::FromPatch { id, assignments } => (|| {
                let mut model = resource.get(&id)?.model;
                for (shift, employee) in assignments {
                    if employee.as_ref().is_some_and(|e| {
                        !model.employees.iter().any(|candidate| candidate.id == *e)
                    }) {
                        return Err("Unknown patch employee".into());
                    }
                    model
                        .shifts
                        .iter_mut()
                        .find(|s| s.id == shift)
                        .ok_or("Unknown patch shift")?
                        .employee = employee;
                }
                resource.post(&model, true)
            })(),
        };
        Some((p.receipt, result))
    }
}
