# Owned XML factory execution

`XmlSolverFactory` owns an `XmlTerminationPlan` and a real `SolverFactory`.
It validates supported score texts and phase declarations before publishing the
factory. Each job compiles fresh borrowed expressions inside the executor's
scope, retaining no temporary references in stored configuration. The executor
installs its supplied phase binding slice in `SolverExtensions` and runs the
normal native solver. Managers can retain the factory after the wrapper drops;
the executable closure owns the XML plan.

Native template limits, job overrides and cancellation remain independent hard
caps. They do not replace XML expression children. Inspect the native template
through `factory().configuration()` and the additional XML declarations through
`termination_plan()`. This is the scoped extension profile, separate from the
scalar precedence profile of `NativeSolverConfigurationSource`.

The constructor consumes the existing basic/list sequence profile. Nested AND/OR
trees, XML work/feasibility limits, and independently inherited phase-local rate
trees execute through the scoped binder. Invalid descendants and unsupported
phase controls reject before publication. The [composition contract](xml-termination-composition.md)
describes solve/phase projection and independent safeguards. No XML class loading or JVM configuration ABI is
claimed. Original manager integration tests receive no upstream-method credit.

`new_with_controls` also lends a solve-wide `StatefulSearchTermination` to the
executor. Install that output in `SolverExtensions.stateful_termination` together
with the phase bindings. Root significant-improvement thresholds retain one
history across phase handoffs; each manager job starts fresh histories. The whole
root tree is evaluated in that scope, preserving AND/OR grouping. Native caps and
cancellation still apply independently. Phase thresholds use fresh phase histories.

The original `new` constructor retains its two-output executor and rejects root
thresholds which require solve-wide state. Both constructors recursively validate
all declarations before any executor call, including deferred threshold score
texts and missing unimproved windows. Input/scoring failures remain ordinary job
errors; a subsequent job can use the same manager.
