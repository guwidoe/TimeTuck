//! Synchronous lifecycle notifications, separate from scored best updates.
use crate::SolverPhaseKind;

/// Notifications from one solver invocation. Phase/step indices are zero based;
/// attempted steps are numbered independently within each entered phase. Exact
/// and custom phases do not produce step events (their work is evaluations and
/// commands). Repair attempts may end without completing an accepted solver step.
///
/// Callbacks run inline; shared solve deadlines include their time. Returning a normal error
/// closes open steps/phases and the solve. Panics propagate without calling more
/// lifecycle callbacks during unwind. Validation failures before solve start emit
/// nothing. A phase score diagnostic never starts another lifecycle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SolverLifecycleEvent {
    SolveStarted,
    SolveEnded,
    PhaseStarted {
        phase: usize,
        kind: SolverPhaseKind,
    },
    PhaseEnded {
        phase: usize,
        kind: SolverPhaseKind,
    },
    StepStarted {
        phase: usize,
        kind: SolverPhaseKind,
        step: u64,
    },
    StepEnded {
        phase: usize,
        kind: SolverPhaseKind,
        step: u64,
        completed: bool,
    },
}
/// Borrowed listener for one invocation; set `SolverExtensions::lifecycle` to attach.
pub type SolverLifecycleCallback<'a> = dyn FnMut(SolverLifecycleEvent) + 'a;
#[derive(Default)]
pub(crate) struct Lifecycle<'a> {
    callback: Option<&'a mut SolverLifecycleCallback<'a>>,
    started: bool,
    phase: Option<(usize, SolverPhaseKind)>,
    step: u64,
    step_open: bool,
}
impl<'a> Lifecycle<'a> {
    pub(crate) fn new(callback: Option<&'a mut SolverLifecycleCallback<'a>>) -> Self {
        Self {
            callback,
            ..Self::default()
        }
    }
    pub(crate) fn enabled(&self) -> bool {
        self.callback.is_some()
    }
    fn emit(&mut self, event: SolverLifecycleEvent) {
        if let Some(callback) = &mut self.callback {
            callback(event);
        }
    }
    pub(crate) fn start(&mut self) {
        if !self.enabled() {
            return;
        }
        self.started = true;
        self.emit(SolverLifecycleEvent::SolveStarted);
    }
    pub(crate) fn start_phase(&mut self, phase: usize, kind: SolverPhaseKind) {
        if !self.enabled() {
            return;
        }
        self.phase = Some((phase, kind));
        self.step = 0;
        self.emit(SolverLifecycleEvent::PhaseStarted { phase, kind });
    }
    pub(crate) fn start_step(&mut self) {
        let Some((phase, kind)) = self.phase else {
            return;
        };
        // A prior early exit is closed before starting another attempt.
        self.end_step(false);
        self.step_open = true;
        self.emit(SolverLifecycleEvent::StepStarted {
            phase,
            kind,
            step: self.step,
        });
    }
    pub(crate) fn end_step(&mut self, completed: bool) {
        if !self.step_open {
            return;
        }
        self.step_open = false;
        let (phase, kind) = self.phase.expect("step phase");
        let step = self.step;
        self.step = self.step.saturating_add(1);
        self.emit(SolverLifecycleEvent::StepEnded {
            phase,
            kind,
            step,
            completed,
        });
    }
    pub(crate) fn end_phase(&mut self) {
        self.end_step(false);
        if let Some((phase, kind)) = self.phase.take() {
            self.emit(SolverLifecycleEvent::PhaseEnded { phase, kind });
        }
    }
}
impl Drop for Lifecycle<'_> {
    fn drop(&mut self) {
        if self.started && !std::thread::panicking() {
            self.end_phase();
            self.started = false;
            self.emit(SolverLifecycleEvent::SolveEnded);
        }
    }
}
