# Native owned model solver sessions

`ModelSolverSession` joins executable `PlanningSolutionBinding` storage with an
owned native factory and manager. Jobs retain an `Arc` input and clone its model
before opening live ranges. The caller supplies the actual native solve callback;
its effective sequence includes job cancellation and overrides. The callback
returns a real assignment, score and logical work. The session validates the
assignment against the live working model, commits through its entity setters,
sets the actual score and publishes that independent working copy.

Model `Clone` must create independent mutable storage. Binding projections must
remain pure and address the same storage with stable membership. These are
existing explicit Rust model contracts, not Java cloning/reflection guarantees.
A native stopped solve may publish a partial assignment with its actual score;
a result without a score rejects before committing. Native callback errors remain
inside `ModelSessionError::Solve`; binding errors retain their own type. Panics
from score predicates are caught by the existing manager, which retains their
message and unregisters the job. This adapter does not catch or replace that cause.

Five full operational Community methods are adapted using their complete TEST
models, counts, value ranges and constraints:

- The failed-solve method submits two separate two-entity jobs whose only value
  is `fail`. Actual score predicates panic only after that value is assigned.
  Both jobs preserve the cause message and unregister. A third two-entity job
  with `v1` and `v2` succeeds through the same manager with actual zero score.
- The ordinary and extended solve methods use ordered duplicate-value pairs,
  with two values and two entities. The extended model projects parent storage
  through the inherited binding and retains `Extra Data`.
- The interface entity method uses two boxed native entity trait objects,
  integer range `0,1,2` and the source sum-of-values penalty. All output objects
  differ in identity from the retained input objects.
- The concrete-provider method uses the three generated values and two entities.
  A real abstract Rust provider trait and concrete implementation feed an actual
  typed executable scoring-catalog constructor. Abstract contracts are not
  constructible recipes. The concrete stream penalty retains match factor one.

Every adapted case executes both prepared Dense and Stream backends. Original
controls independently rescore duplicate and integer objectives, vary live ranges,
retain unchanged inputs, verify score-factory resource destruction on success and
failure, cancel a queued job and an active job during actual scoring, and then
reuse its manager and problem ID for a successful solve. The resource counters
observe resources owned by `PreparedScoreFactory`; they are not simulated flags.

Only these five methods override their original framework exclusion. Quarkus
singleton injection/class discovery, Spring multimodule entity/package scanning,
REST property rendering, Java exceptions/ABI, annotations and framework loading
remain excluded. The Spring multimodule method was inspected but receives no
credit: its distinguishing behavior is package scanning, not an additional native
solver operation. No production implementation or Enterprise source was read.
