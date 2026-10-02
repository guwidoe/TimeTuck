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
not an incremental tuple graph. Separate persistent operators support explicit
delta propagation. `StreamGraphBuilder` connects these operators with automatic
routing, shared ports and graph transactions; `StreamScoreDirector` binds the
graph to finite basic-variable moves, as described below.

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
- `ExistsNode` supports indexed equality plus a pair predicate, in Exists or
  NotExists mode. It preserves left IDs/payloads and never multiplies them by
  right-match count. Bidirectional cached match IDs make retractions callback-free;
  right updates emit only membership transitions, and accepted left updates emit
  the current payload. Key buckets restrict pair predicates to relevant facts.
  Persistent candidate roots and output are prepared before installation, so
  ordinary callback/key/clone panics leave this standalone node unchanged.
  Memory is proportional to facts plus matching pairs; dense relations can require
  quadratic space. Empty right input retains every left row only in NotExists mode.
- `DistinctNode<I,T>` keeps one output per `T::cmp` equivalence class, identified
  by that class. Only equality/ordering of that ID is meaningful; current
  representative data comes from the payload. Equality and ordering must agree.
  The lowest input ID supplies
  its payload, so equal values with different non-key data remain well-defined.
  Replacing or removing that representative updates the surviving class; changing
  a nonrepresentative duplicate emits nothing. Last removal retracts the class.
  `occurrences` reports multiplicity. Input values are captured for removal by ID.
  Persistent candidates and output are prepared before installation; ordinary
  clone/ordering panics preserve the previous node. Storage is linear in facts
  and classes, with logarithmic map operations and constant-size checkpoints.
- `GroupNode` caches each input ID's mapped key and integer. An update retracts
  the captured old value and accumulates the new one in the affected groups only.
  It emits one final summary for a same-key replacement, old then new summaries
  for a key change, and a retraction when the last group member disappears.
  Equal values at different IDs retain separate multiplicities.
- `ScoreNode` caches one contribution per ID and updates its total by subtracting
  the old score and adding the new score. Retraction does not replay the callback.
  Invalid structural contributions return an error before changing that node.
  Current individual contributions remain inspectable.

Used on their own, these operators perform incremental updates without graph
transactions. A call commits one node's input. A join replacement may emit a removal and insertion of the same
pair ID; intermediate downstream totals are not final graph results. Explicit
connections must preserve emitted order and fan changes out to every consumer.

Callback/key projection and output row cloning occur before mutation in filter
and join nodes. Scoring callbacks and structural checks also precede mutation.
Failure there leaves that node unchanged, but previously updated nodes are not
rolled back. In filter/join/group/score nodes, a panic during mutation in user-defined
key/identity operations poisons the node; subsequent reads/updates panic instead
of exposing possibly inconsistent caches. `is_poisoned()` remains available. Keys/facts must have stable
ordering and immutable contents, and callbacks must be deterministic. Rebuild
the affected pipeline after a partially delivered failure. Allocation aborts are
not recoverable. These standalone operators make no graph-wide atomicity claim.

Do not hide persistent nodes inside existing `ConstraintNetwork` callbacks via
interior mutability: current solver move undo cannot restore that external state.
Use snapshot pipelines in those callbacks, or the separate transactional
`StreamScoreDirector` for persistent graph state.

## Typed graph transactions

`StreamGraphBuilder<S>` builds a fixed directed acyclic graph of typed sources,
filters, maps, equality/existence joins, distinct, integer groups and named score terminals.
`distinct(input)` returns `StreamPort<T,T>`: value-class identity and representative
payload. Include a stable fact ID in the value to distinguish facts with identical
attributes. Representative priority is native input-ID order, not a claim about
Java representative choice or snapshot-stream encounter order.
`existence(left, right, mode, left_key, right_key, predicate)` selects matching or
unmatched left rows. Constant keys allow predicate-only matching. A stable-ID
inequality predicate excludes self matches. Both sides of self-relations receive
source updates automatically. Explicit assigned filtering remains the caller's
responsibility on both input ports. Each connection points from an existing node to a new one, so cycles cannot be built.
Reusing a port shares that node and fans its outputs out to every consumer.
Equivalent closures are not automatically deduplicated. Handles from another
builder/graph and duplicate constraint names return errors before registration.
Owned IDs and payloads must be `Clone + 'static`; callbacks can borrow immutable
problem data for the graph lifetime. The graph is single-threaded.

Source changes use stable tuple IDs. A source put replaces its prior value; a
retraction removes it. Maps retain identity, join IDs are ordered ID pairs, and
group IDs are keys. Group payloads contain both key and `IntSummary`. Empty keyed
groups disappear. Sources start empty; no input facts are discovered implicitly.
`constraint_matches` exposes each terminal's exact typed tuples and ordinary
scores. Constraint totals are named and sorted; the graph total excludes planning
structure. Nonzero structural contributions are rejected.

A transaction owns an exclusive graph borrow and drains each input change through
an iterative FIFO queue before returning. Node output order is preserved through
fan-out, including both inputs of a shared self-join and paths of different
lengths that later rejoin. Callbacks may see intermediate tuples during delivery;
only the fully drained result is a completed candidate. A multi-input transaction
can inspect results between its writes, commit all writes or undo them together.

Before the first update to each affected node, the transaction saves that node's
persistent map roots and scalar state, including indexes, summaries and scores.
Further updates reuse that checkpoint. Sources, filter inputs, both join indexes
and their ID buckets, group inputs/collectors, score contributions and match
records all use shared AVL trees. Integer collector multiplicities use the same
storage. Cloning a map shares its root without cloning keys, values or rows;
updates allocate only search/rebalance paths, sharing unchanged branches.

Dropping restores owned roots and scalar state with no predicate, key, scoring,
cloning or comparison callbacks. Commit keeps current roots with no callback
replay. Checkpoint/candidate-state storage scales with affected nodes rather than
all cached rows. Updating a tree entry takes logarithmic path work; joins still
emit every affected match, and source changes still clone payloads during tuple
propagation. Model validation, assignment snapshots and structural counting retain
their existing costs. No constant-time move or runtime-speedup claim follows.

`source_rows` and `constraint_matches` now return read-only `SnapshotMap` views,
replacing their former `BTreeMap` return types. They support ascending iteration,
keys/values, lookup, indexing, length and extrema. Cloning a view creates a stable
snapshot that can outlive subsequent graph updates without cloning payloads.
Snapshots keep their referenced branches alive until released. Mutation stays
private to the solver. Atomic reference counts preserve `Send`/`Sync` when key and
value types permit them; graph scheduling remains single-threaded. No unsafe code
or additional dependencies are used.

Any returned error or ordinary caught panic invalidates the transaction. Further
updates, graph inspection, score reads and commit reject it; dropping restores
the pre-transaction graph, including operators poisoned during mutation. Callbacks
must be deterministic; their external side effects cannot be rolled back. Facts
and key comparison identity must be immutable. Allocation aborts, forgotten guards
and panicking destructors are outside ordinary unwind guarantees.

## Basic-variable move binding

`StreamScoreDirector` binds one initially empty source of `(variable_index,
Option<V>)` payloads, identified by the same variable index, to a `FiniteProblem`.
It loads every variable, including unassigned ones; constraints explicitly filter
unassigned rows as needed. Other sources may already hold immutable problem facts.
The director exposes read-only graph/assignment inspection so callers cannot
update graph facts independently of the planning assignment.

Applying a change, swap or composite first uses the existing model transaction
and validates the final assignment's domains and pins. It then sends each touched
variable's final value once to a graph transaction. Intermediate composite writes
are not delivered. Dropping or ordinary unwinding restores both assignment and
graph. Commit retains both without callback replay. Model validation errors occur
before graph callbacks. The returned score adds the model's negative count of
unassigned required variables; optional nulls add no structural penalty.

This director supports custom search loops and exact enumeration adapters.
`greedy_stream_construction`, `stream_local_search` and `stream_local_search_from`
consume a graph with an empty planning source. They use the same internal search
algorithms as the scoped-network entry points: construction early-pick policies,
first/best accepted selection, strict/hill/late/tabu/annealing acceptance, optional
rejected fallback, seeded variable order, budgets and best-solution retention.
Explicit initial assignments are validated even at zero budget. A zero evaluation
budget or impossible model performs no new scoring; source ownership/emptiness
and acceptance configuration still validate. Other preloaded fact sources remain
fixed. Each phase initializes its own director and counts that initial score once.

Selection scans candidates from one unchanged base. When a better eligible
candidate appears, it saves the assignment and the current roots/scalars of
affected graph nodes before dropping the candidate transaction. Installing the
winner moves that saved state back without rerunning callbacks. Failures during a later
candidate, acceptance or snapshot leave the base unchanged. Candidate snapshots
share unchanged graph storage; assignment cloning remains linear. The result's
`constraint_evaluations` counts terminal scoring callbacks,
including intermediate tuples during propagation, not predicate/key callbacks.
Retractions reuse cached scores. Loading immutable facts before calling a search
entry point is preprocessing and is excluded from phase counters.

`assert_scores` runs a full graph rebuild from copied source rows with empty
operator caches. It checks individual terminal match IDs/scores, cached
contributions, constraint totals and the graph total, so opposing errors cannot
hide behind a correct sum. It restores the current graph after the diagnostic,
including on errors or ordinary callback panic. Diagnostic callbacks are excluded
from reported search work. `StreamGraph::assert_consistent` and the director's
method expose the same check; a diagnostic failure on an active graph transaction
invalidates that transaction. This verifies scores, not arbitrary payload equality.

List-variable stream sources and additional collectors (lists, maps, averages and
exact decimals) remain pending.
Run `cargo run --release --example stream_coloring` for an original
24-variable construction/search example with independent result verification.

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
recorded separately. Two more adapted methods preserve named mirrored joins and
two-key equality self-joins with automatic routing, checking every tuple, score
and constraint name before and after updates. Descriptor, custom justification
and indictment APIs remain pending.

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

Twelve original graph/move regressions additionally check shared-node callbacks,
unrelated branches, multi-source rollback after errors and panics, 44 clone-failure
injection positions, foreign handles, duplicate names, a 2,000-node deep graph,
wrapping arithmetic, 1,000 three-write transactions against independent grouped
recomputation and 600 updates through reconverging paths. Basic-variable binding
checks all 81 partial scheduling assignments, final composite validation, domains,
pins, optional nulls, complete rollback after later-variable failures and all 16
complete assignments. Bounded first-improvement search with changes and swaps
reaches the independently enumerated zero-score optimum; changes alone have a
local optimum on this case. Two API doctests demonstrate graph and move undo.

Six original search-integration methods additionally compare 24 construction
policy/budget configurations and 200 local-search configurations with independent
full scoring. The latter exercise all five acceptance policies, both selection
modes, fallback, seeded/original order and five evaluation budgets. Phase handoff,
callback accounting, diagnostics, optional values, heterogeneous domains, pins,
zero limits, empty/impossible models, preloaded fact scores and validation before
callbacks are checked. A unit regression verifies that scoring errors, scorer
panics and acceptor panics after saving a winner preserve the prior state.

Three storage unit tests compare 20,000 sorted/reverse/generated edits with
`BTreeMap`, checking every AVL ordering, height, balance and size invariant and
retained snapshots. They bound newly retained path nodes on 8,192-key maps without
requiring key/value `Clone`, and inject comparison panics at seven search depths.
Two original integration methods verify identical changed-row payload clone costs
at 16 and 4,096 source rows, held source/match snapshot isolation, rollback/commit,
duplicate collector retraction/extrema and preserved thread traits. One API
doctest demonstrates holding a snapshot across committed graph updates.

Original public-API Java stream comparisons now cover 128 cases and 2,899 scoring
states, including partial assignments, duplicate/self edges, signed group sums,
commits and rollback. Community `SolutionManager.update()` scores each state;
native persistent transactions update the same join/group constraints. Python
independently checks both ordinary scores and native exact conflict/group matches.
Java match analysis is Enterprise-only in this release and is not used.

Separate matched local-search timing uses incremental streams in both engines,
with four hill/tabu workloads and 64 small correctness cases with independently
enumerated optima. Rust graph construction and Java per-solve setup are timed;
Java's reusable factory is prepared before timing. Each sample records final
quality and terminal weighting callbacks alongside elapsed time. Original
comparisons grant no additional upstream method credit. See `docs/dashboard.md`
for the measurement conditions and limits.

Thirteen adapted existence methods preserve initial and incremental scores,
exact left match identities/payloads, per-match contributions and constraint names.
They cover no joiners, filtering-only, equality plus optional filters, self
exclusion, explicit assigned filtering on both sides, and existence after a
distinct-count group. The latter preserves all three tested nonempty states; it
does not establish global empty-aggregation semantics. Unsupported-class
descriptor exceptions remain pending. Provenance includes the test fixtures and
assertion helper, with no additional fixture method credit.

Seven original existence regressions cover 6,000 indexed edits against nested
enumeration (seed 20261018), 2,000 self-relation graph transactions with commit/undo
(seed 20261019), duplicate match suppression, changed-bucket predicate counts,
callback-free removal, payload updates, whole-graph rollback after predicate panic
and invalid score, output clone panic, and 160 ordering-panic injection budgets.
Construction and local search both reach a directly verified zero-score assignment
using existence constraints and full-rebuild diagnostics. One public doctest
demonstrates NotExists transitions. Existing Java stream timing workloads use
join/group scoring; they do not benchmark the new existence node.

Twenty adapted uni/bi/tri/quad methods preserve no-op distinct, mapped duplicate
and nonduplicate streams, and mapped distinct with/without duplicates. Every
initial and first-entity-removal score and match assertion is retained. Cartesian
self-joins plus stable-ID ordering reproduce the tested unique pairs; third-fact
self-exclusion and indexed group joins retain the tri/quad query structures.
The tri nonduplicate mapping deliberately uses only the first two groups, exactly
as the source test specifies. Tagged identities and sorted sets preserve tuple
and group-set equality. Fixture and assertion-helper provenance is recorded.

Seven original regressions additionally cover 6,000 generated updates with equal
keys/different payloads (seed 20261020), 1,000 three-write graph transactions with
commit/undo and retained snapshots (seed 20261021), representative replacement,
nullable tuple values, 180 combined clone/ordering failure budgets, terminal
errors/panics, and construction/search minimizing distinct assigned values.
Nested distinct nodes are checked against independent lowest-ID representatives
and full graph rebuilds. One API doctest demonstrates duplicate multiplicity and
retraction. Existing Java timing workloads do not measure this new node.


## Composable incremental collectors

`ReversibleCollector<V>` defines captured-value insertion/removal and an immutable
finished result. `Default` must be empty, `Clone` must isolate subsequent mutation,
and methods must be deterministic. `finish` must not mutate state. Returning false
when removing a captured occurrence is an invariant failure. Custom collectors
must honor these contracts; arbitrary shared mutable interior state cannot be
rolled back automatically.

`IntCollector` implements the trait with its existing integer summary.
`CountCollector` counts occurrences; `SetCollector<V>` retains ordered unique keys
with duplicate multiplicities, returning a persistent `SnapshotMap<V, ()>`.
Use stable fact IDs for set membership. `PairCollector<A,B>` collects independently
mapped fields together and can be nested for more fields. Built-in collectors
share persistent roots; custom collector cloning may cost more.

`CollectNode` captures each input's key and mapped value, retracts the old value
on replacement, and emits changes for affected groups. It prepares candidate
state and finished outputs before installation, preserving the previous state
on ordinary collector/clone/ordering panics. The graph builder's
`group_collect_with` exposes keyed aggregation; `collect` uses a unit key for a
global group. Both participate in graph rollback, full rebuild diagnostics and
basic-variable construction/local search.

Only nonempty groups emit rows. In particular, the global stream starts without
a row and retracts its row when the last occurrence disappears. A standalone
collector's empty finish (zero count/sum, empty set, absent extrema) does not imply
an empty stream emits a match. The eager snapshot `ConstraintStream::collect`
returns a summary directly and retains its existing empty-summary behavior.
`AggregationBoundaryOracle.java` verifies empty / one zero-weight fact / removed
last fact through Community full score updates with constant presence penalties.
It runs with `scripts/differential_streams.py`; no Java incremental timing or
additional upstream-method credit is claimed for this probe.

Sixteen adapted `groupBy_0Mapping{1..4}Collector` methods preserve both states of
all four arities, query multiplicity, count/distinct/min/max/set tuple projections,
per-match scores, names and totals. Fixture/helper provenance is recorded.
Seven original regressions cover 5,000 captured-value/key edits (seed 20261022),
1,000 three-write graph transactions (seed 20261023), nested projections and held
snapshots, global empty boundaries, custom collector failures, 180 trait-panic
budgets, terminal errors, construction and search. The existing Java timing
workloads use the specialized integer group node; they do not benchmark the new
generic collector node.
