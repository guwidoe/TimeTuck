# Executable planning composition

`PlanningEntityBinding` registers callable genuine member getters and setters.
`inherit` projects contained parent storage, preserving its range references and
rejecting a repeated genuine member name before any callback executes. Generic
trait getters/setters can be registered directly. `PlanningSolutionBinding`
composes actual entity collections, including collections inherited through a
parent solution projection. Its schema uses concrete Rust `TypeId`s; repeated
collections of the same type must agree on their genuine members.

Opening reads the current owned solution and produces the existing `FiniteProblem`.
Committing first validates the whole assignment's ranges and all read/write
collection slot counts, then writes through registered setters. Collection
membership must remain stable during a solve; the API does not retain object
handles or detect replacement by a different entity at the same index. Getters,
range callbacks, and immutable/mutable projections must remain pure. Paired
read/write projections must refer to the same storage. Equal-length projections
into different storage cannot be detected. Setters must not change another
collection's membership. Setter panics are not rolled back. This is an optional
assembly boundary; ordinary scoring and move hot paths are unchanged.

The Community inheritance adaptations use real manager jobs and prepared score
directors, then write scores and assignments into owned composed solutions. Java
annotations and scans are replaced with explicit typed registrations; concrete
base/child storage is replaced with composition and interfaces with actual Rust
traits. Testdata value objects use their unique fixture codes as native values.
Object/raw entity collections become typed native collections of their actual
fixture inhabitants. Nullable singleton entity fields in the XML fixture are
absent in this opening and therefore contribute no variables. The XML factory
registry resolves the full solution/entity/provider declaration and consumes its
best-score limit. No Spring integration or JVM ABI is claimed.

Eleven annotation/classpath and single-level/mixed inheritance rejection tests
remain excluded. Native multi-level composition is valid; imposing JVM scanner
restrictions on it would not demonstrate those source behaviors.
