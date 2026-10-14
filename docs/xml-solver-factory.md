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

The constructor consumes the existing basic/list sequence profile. Unsupported
combinations retain the XML binder's errors, including solve-wide AND with
phase-local diminished returns. No XML class loading or JVM configuration ABI is
claimed. Original manager integration tests receive no upstream-method credit.
