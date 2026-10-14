# Named native model factories

`NamedModelFactoryRegistry` supplies the missing cross-name registration and
selection layer. Existing `PreparedScoreFactory` already prepares one scoring
recipe for one model; `SolverFactory` already owns defaults, per-job overrides
and cancellation. The XML registry selects a signature within one input/result
family. The new registry composes these capabilities instead of implementing a
second scoring engine, manager or XML parser.

Register each owned input type with its real `PlanningSchema` and concrete named
`NamedModelScoring` builders. A builder returns the existing typed `SolverFactory`;
its input `PlanningType` must equal the schema's solution type. It consumes the
selected native sequence and environment profile and must construct its actual
scoring/director recipe. Names identify registrations; they never establish Rust
type identity. Different owned model and result types can coexist in one bank.

Preparation resolves named model and scoring choices, rejects missing or ambiguous
choices, unused registrations, duplicate names, absent genuine entity schemas,
and common invalid configuration before any builder executes. A sole registered
model or recipe can be selected implicitly. Every registered model and scoring
recipe must be used by at least one configuration; register only the recipes
intended for this bank. Concrete builders perform their remaining model/scoring
validation. Failed preparation publishes no partial factory bank; callback side
effects are not rolled back. Job creation happens only after successful preparation.

Typed lookup returns actual factories, whose managers solve independent owned
problems. Factory defaults are immutable; job-specific time overrides and
cancellation use the existing manager implementation. The bank itself is a
single-threaded registration object with Rc schemas, while each retrieved factory
has the existing Send/Sync solve callback contract. This registry does not launch
jobs or share mutable scoring state between models.

Five operational Community TEST methods are adapted: named solve, per-job time
override, no-XML configuration, explicit XML configuration, and phase-only XML
termination. TEST resource declarations pass through the existing validated XML
parser; their supplied global/local time fields reach actual native configuration.
Two real named managers execute the two-entity v1/v2 fixture. Gradients are read
from the actual effective job limits, including default 30 seconds overridden to
2 seconds. The source negative initialized HardSoft scope is retained and its
negative ordinary score projected below the actual native SimpleScore goal before
observing the native time component. No Java termination-priority equivalence is
claimed for observations outside that controlled scope.

Six original controls exercise heterogeneous basic/list models, concrete dense
and stream prepared directors, whole-set preflight without callbacks, failed
factory publication, typed lookup, independent cancellation and sparse overrides.
No Spring scans, beans, classloader resources, YAML configuration, framework
injection, daemon service or Enterprise support is claimed. Infrastructure-only
methods in the source class remain excluded. Defaults and solver kernels are
unchanged; this optional registration layer makes no performance claim.

The native XML parser accepts only `schemaLocation` and
`noNamespaceSchemaLocation` in the exact XML Schema-instance namespace. These
external hints are ignored in the executable element tree and never fetched;
source-backed serialization may retain them, while rebuilt XML may omit them.
Ordinary unknown attributes, other namespaced attributes and foreign element
namespaces remain rejected. The copied TEST resources remain unchanged. Two
original controls retain and execute global/phase limits from hinted XML and
reject wrong namespace, unqualified hints and unrelated attributes. This uses
application-selected native schema checks, not external XSD validation.
The hint boundary follows the [W3C XML Schema location convention](https://www.w3.org/TR/xmlschema-1/#schema-loc).
