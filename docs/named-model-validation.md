# Named model validation and discovered scoring

The optional `register_global_scoring` catalog stores the actual `PlanningType`
of each callback input and its executable factory builder. Model-owned recipes
retain precedence. Without model-owned recipes, missing/ambiguous explicit scoring
selection is diagnosed against the global catalog before callback execution.
The selected callback input must match the actual registered schema type. All
registered models and scoring recipes must be used; unused global entries are
reported before any builder. Valid selections build actual typed factories only
after every preflight check. A builder failure publishes no partial bank; builder
side effects themselves are not rolled back.

`apply_xml_selection` consumes solutionClass and one scoreDirectorFactory class
selection through existing SolverXmlConfiguration. These strings are explicit
native registration keys, not Java class loading. Multiple scoring selections
reject before either key changes. This method interprets selection only; other
XML configuration remains the responsibility of existing typed adapters.

The new source cases adapt7 unspecified and11 multiple-solver validation methods.
They retain all solver/model/scoring diagnostic identities, missing/ambiguous/
unused conditions, two solver requests where specified, property score targets
and the actual normal/easy/incremental TEST XML selection fixtures. Native typed
Result errors replace Spring cause wrappers. Diagnostic words describe registered
models/entities/recipes rather than pretending to find annotations/interfaces
on a Java classpath. Source dummy calculator null becomes an executed native
nullable callback followed by a job error; no score is fabricated. Native input
types replace Java's erased Object calculator boundary. No inferred model type
comes from a string name. The supplementary other model executes an independent
native scoring objective; it does not claim source declarative/Gizmo codegen parity.

Originals execute heterogeneous real factories/managers/jobs, independently scored
assignments, actual selected dummy callback failure and callback silence for
missing/type/duplicate/XML errors. Existing named operational cases and fixtures
are unchanged and included in affected checks. Immutable Java record/entity
rejection, class loading, dependency injection, YAML/benchmark autoconfiguration,
daemon lifecycle and other infrastructure-only methods remain excluded. This is
native configuration behavior, not Spring integration.
