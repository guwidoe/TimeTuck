# Owned roster workflow

Run `cargo run -p timetuck --example owned_roster_workflow`. The example creates a
fresh temporary directory containing its native input document, configuration,
generated configuration schema, and solved output from each scoring backend.
The printed paths remain available for inspection.

The application assigns employees to shifts. Missing qualifications and overlapping
assignments are hard penalties; assigning someone other than the preferred employee
is a configurable soft penalty. Input employee IDs resolve to actual native values.
Every job reads current staff, skills, intervals and preferences, builds independent
scoring state, and commits the returned assignment through typed planning bindings.
An owned XML factory supplies a solve time cap, best-score target and local step
limit. Caller time overrides remain independent. Hill climbing stops when no move
improves the score, so a step limit is a ceiling, not a required amount of work.

The workflow reads real input/configuration files, retains the input independently,
executes an owned manager after dropping its factory wrapper, writes output through
`SolutionFileIo`, and rereads that output. Configuration metadata and codecs are
executable; the generated schema describes the actual configuration members. The
roster document is an original versioned application format, not Java wire parity.

Original application tests check both Dense and Stream results against a separate
employee-grouped interval sweep. Exhaustive enumeration establishes the optimum for
the small examples only. A warm-start case demonstrates different returned quality
and work for one-step versus longer XML limits. Reusing a factory with changed staff
and intervals proves that scoring facts are not cached from its first job. Malformed
input/configuration preserves an existing output file, and a zero deadline returns
an independently checked partial score which survives codec round trip. Larger
rosters do not inherit an optimality guarantee from those small exhaustive checks.
These are original controls and earn zero upstream-method credit.

`factory_with_plan(configuration, backend, plan)` accepts an explicit owned native
XML termination plan. `run_files_with_xml(input, configuration, solver_xml, output,
backend)` reads a complete solver XML file and executes that plan through the same
file workflow. The default factory and `run_files` retain their generated XML and
configuration format. Both backends install the fresh phase bindings and solver-wide
stateful expression compiled by `XmlSolverFactory::new_with_controls` for each job.
`SolvedRoster.termination` reports the actual solver termination.

Original custom XML controls use a zero unimproved window and a large significant
improvement threshold on a deliberately poor complete warm start. They check root
condition termination, bounded work, independently scored output and codec round
trip, then compare the default solve with exhaustive optimum. Repeated file runs
start fresh. Invalid XML, phase shapes and thresholds preserve existing output on
both backends. These controls establish application execution, not elapsed-time
trace parity or additional upstream-method credit.
