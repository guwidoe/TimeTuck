# Native constraint streams

`ConstraintStream<T>` is an eager, typed multiset relation. Rows may be facts,
stable IDs, references or tuples; ordinary Rust equality defines their identity.
Use explicit IDs to distinguish objects with otherwise equal attributes.
The library receives exactly the supplied rows. Callers explicitly filter
unassigned planning variables; there is no implicit Java-style fact discovery.

`ConstraintStream` operates on snapshots. Put a snapshot stream pipeline in a
`ConstraintNetwork` or `ListConstraintNetwork` callback with the complete variable
scope on which it depends. Affected callbacks rebuild their stream; unrelated
callbacks retain their cached contributions. This is scoped incremental scoring,
not an incremental tuple graph. Separate persistent operators now support explicit
delta propagation, as described below; automatic graph scheduling and solver
move binding remain pending.

## Relations and scoring

- Filter, map, flat-map and concat preserve duplicate occurrences and encounter
  order. Distinct retains the first occurrence of each equal row.
- Predicate joins scan all pairs. Equality joins index the right-hand rows by
  hash key, preserving left order and then right order within each match. Tuple
  keys support multiple equalities. Self matches and duplicate cross products
  are intentional. Additional range predicates can filter the resulting pairs.
- Exists/not-exists retain or reject each left occurrence without multiplying it
  by the number of right matches. They scan right rows until the outcome is known.
  Excluding the same entity requires a stable-ID predicate supplied by the caller.
- Unique pairs emit each pair of distinct input positions once (`i < j`). Equal
  values at different positions still pair; this is not planning-ID sorting.
- Group-by produces one `(key, rows)` per observed key in first-occurrence order.
  Group-collect produces integer summaries in the same key order. Empty input
  creates no keyed group; collecting the entire empty relation yields zeros and
  absent extrema. Key/value mappings run once per input row in encounter order.
- Score evaluates one ordinary contribution per row and adds it with the selected
  score type's arithmetic. Negative contributions penalize, positive ones reward.
  The empty score is zero. Nonzero structural contributions return an error with
  the offending row index before arithmetic can discard them. Structural state
  remains the planning model's responsibility. Rows themselves provide the match
  tuples; named justifications/indictments are not yet implemented.

Streams own their row storage. Borrowed facts must remain valid and callbacks must
be deterministic throughout a scoring session. Interior mutation of captured
facts can invalidate cached contributions, just as for existing score callbacks.
Encounter order does not depend on hash iteration order. Joins materialize their
outputs; a Cartesian join has quadratic output size. No memory or throughput
parity with Timefold's incremental engine is claimed.

## Retractable integer collectors

`IntCollector` captures integer values and multiplicities in an ordered multiset.
Insert adds one occurrence; remove retracts one and reports whether it existed.
An absent removal is a no-op. The summary includes native `usize` occurrence and
distinct counts, wrapping signed 64-bit sum, and optional minimum/maximum. Empty
results have zero counts/sum and `None` extrema. Count overflow panics before any
state modification. Inserts/removals cost logarithmic time in distinct values.

Unlike a closure capturing a mutable domain object, removal takes its previously
inserted integer. To update a mapped fact, remove its old mapped value and insert
its new value. The caller tracks that old value; collector tokens and mutable
object listeners are not provided. Independent equal occurrences are
indistinguishable, and each removal consumes only one of them.

## Persistent incremental operators

`TupleChange<I,T>` addresses an occurrence by stable identity. `Some(value)`
inserts or replaces it; `None` removes it, with an absent removal a no-op. IDs
distinguish equal-valued rows. `map` transforms a present payload and preserves
its identity; it does not invoke the mapping on a retraction. Persist the nodes
and explicitly deliver every output change to downstream nodes in emitted order.
Read the pipeline's final result only after all outputs have been delivered.

- `FilterNode` evaluates only a changed row, remembers rejected rows, and emits
  insert/update/removal transitions. Removal uses the cached decision.
- `JoinNode` retains both inputs and ordered key indexes, including unmatched
  facts. A side update visits only its old/new opposite matching buckets, emits
  old pair removals followed by new pair insertions, and caches the new key.
  Pair identity is `(left_id, right_id)`. A constant key produces a Cartesian
  join; tuple keys express multiple equalities. Self-joins require delivering
  each source change to both sides; either order is valid. Inspection is ordered
  by left ID then right ID, independently of insertion order.
- `GroupNode` caches each input ID's mapped key and integer. An update retracts
  the captured old value and accumulates the new one in the affected groups only.
  It emits one final summary for a same-key replacement, old then new summaries
  for a key change, and a retraction when the last group member disappears.
  Equal values at different IDs retain separate multiplicities.
- `ScoreNode` caches one contribution per ID and updates its total by subtracting
  the old score and adding the new score. Retraction does not replay the callback.
  Invalid structural contributions return an error before changing that node.
  Current individual contributions remain inspectable.

These operators perform actual incremental updates, but do not yet provide an
automatic heterogeneous DAG, shared-node scheduling, graph-wide transactions,
incremental existence/distinct nodes or solver-move binding. A call commits one
node's input. A join replacement may emit a removal and insertion of the same
pair ID; intermediate downstream totals are not final graph results. Explicit
connections must preserve emitted order and fan changes out to every consumer.

Callback/key projection and output row cloning occur before mutation in filter
and join nodes. Scoring callbacks and structural checks also precede mutation.
Failure there leaves that node unchanged, but previously updated nodes are not
rolled back. During mutation, a panic in user-defined key/identity operations
poisons the node; subsequent reads/updates panic instead of exposing possibly
inconsistent caches. `is_poisoned()` remains available. Keys/facts must have stable
ordering and immutable contents, and callbacks must be deterministic. Rebuild
the affected pipeline after a partially delivered failure. Allocation aborts are
not recoverable. There is no graph-wide panic or error atomicity claim.

Do not hide persistent nodes inside existing `ConstraintNetwork` callbacks via
interior mutability: current solver move undo cannot restore that external state.
Use the snapshot pipeline there until a transactional graph binding exists.

## Evidence and limitations

Five adapted methods from pinned `InnerUniConstraintCollectorsTest` preserve all
seven assertions each, including duplicate insertion/retraction and empty
results. Exact provenance and native representation changes are recorded in
`compat/provenance.json`. This grants five upstream collector-method passes.
Seven further methods from `AbstractUniConstraintStreamTest` preserve every
initial and incremental score/match assertion for three filters, two joins and
two grouping cases. Stable native identities and explicit event delivery replace
Java object identity and notification plumbing. Per-match scores, exact match
sets and applicable collector counts are checked. Fixture/helper adaptations are
recorded separately. Descriptor, automatic graph, custom justification and
indictment APIs remain pending.

Twelve original regression methods additionally check:

- 4,096 seeded collector edits against independently rebuilt multiset summaries,
  including signed integer boundaries, overflow and absent removals.
- 256 indexed-join cases against nested enumeration, with deliberate hash
  collisions, duplicates and empty sides, plus chained joins and tuple keys.
- Bag semantics, grouping order, existence multiplicity, empty groups, distinct
  values, structural rejection and weighted multilevel scores.
- All 81 partial assignments of a four-job/two-machine scheduling problem against
  independent loops, plus every composite move's rollback and commit.
- Exact enumeration of all 16 complete assignments, greedy construction and
  local improvement from an overloaded assignment to independently checked
  `0hard/0soft`. All constraints are nonpositive, so zero is also an upper bound.

Nine original incremental regressions additionally check 1,500 self-join updates
with alternating side order and 2,000 composed filter/join/group/score updates
against independent full recomputation (seeds 20261010 and 20261011). They check
captured-value retractions, empty group removal, wrapping totals, contribution
errors, callback counts restricted to changed rows, pre-mutation callback panics,
and poisoned-node rejection after panicking user key operations.

The existing Java/Rust comparison and benchmark sections cover earlier score,
move, exact and heuristic workloads. They do not measure these stream operations.
Public-API Java stream comparisons and matched stream timing are future work.
