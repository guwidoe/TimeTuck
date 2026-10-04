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
  facts. A side update visits only its old/new opposite matching buckets and caches
  the new key. Same-key replacements keep index membership unchanged and emit one
  replacement per surviving pair. Key changes emit old pair removals followed by
  new pair insertions.
  Pair identity is `(left_id, right_id)`. A constant key produces a Cartesian
  join; tuple keys express multiple equalities. Self-joins require delivering
  each source change to both sides; either order is valid. Inspection is ordered
  by left ID, right key, then right ID, independently of insertion order.
  `JoinNode::new` and graph `join_equal` retain exact equality bucket lookup.
  `JoinNode::comparing` and graph `join_compare` support Equal, LessThan,
  LessThanOrEqual, GreaterThan and GreaterThanOrEqual. Orientation is always
  left key versus right key; right-side updates reverse the index relation.
  Inequalities seek a bound in the persistent AVL index, then visit only matching
  ordered buckets. Index traversal costs O(log K + B), for K opposite distinct
  keys and B matching buckets; output still visits every matching occurrence and
  looks up each payload by ID. Dense joins can have quadratic output size.
  Keys use native Rust Ord; callers explicitly choose missing-value handling.
  Tuple ordering is lexicographic, not a conjunction of separate inequalities.
  Additional predicates use a downstream filter; multiple ordered conditions,
  interval indexing and Java event-order parity remain outside this operator.
- `PartitionedJoinNode` and graph `join_partitioned` first match an equality key,
  then compare an ordered key inside that equality partition. Projections return
  `(equality_key, ordered_key)`; tuple equality keys support multiple equalities.
  The ordered key still uses one native Ord relation, with explicit left/right
  orientation. Both inputs retain unmatched rows. Identity remains `(left_id,
  right_id)`; inspection orders equality key, left ID, right key and right ID.
  Each side has a persistent equality map containing the existing ordered indexes,
  plus an ID-to-partition lookup. Updates visit only old/new partitions and matching
  ranges, keeping memory linear in indexed facts; dense output remains quadratic.
  Empty side partitions are removed. Persistent candidate roots and events precede
  installation, so ordinary callback/clone/comparison panics leave this standalone
  node unchanged, including a partially prepared transfer between partitions.
  Graph checkpoints share these roots. Same-key replacements retain membership;
  retraction uses captured keys without invoking projections. No additional
  overhead is added to the existing equality-only or ordered-only join indexes.
- `OverlapJoinNode` and graph `join_overlapping` index the strict cross-endpoint
  predicate `left_start < right_end && right_start < left_end`. Touching endpoints
  never match. Endpoints are captured without normalization or validation: a
  zero-length or reversed interval can match a containing interval under these
  two comparisons. This is not mathematical intersection of empty sets.
  `join_overlapping_partitioned` and `OverlapJoinNode::partitioned` add equality
  partitions; projections return `(equality_key, start, end)` and tuple equality
  keys support multiple equalities. Each partition uses a persistent AVL tree
  ordered by `(start, ID)` with cached subtree maximum endpoints. Queries skip
  starts beyond the query end and subtrees whose maximum end is at/before the
  query start. Updates rebuild logarithmic paths; memory is linear in rows, and
  dense matching still produces quadratic pairs. No general multidimensional
  inequality index or output-sensitive query complexity guarantee is claimed.
  Pair identities, captured-key retractions, same-key replacement and candidate
  root installation follow the partitioned join contract. Inspection is ordered
  by left ID, right start, right ID. Graph transactions share persistent roots.
- `ExistsNode` supports indexed equality plus a pair predicate, in Exists or
  NotExists mode. It preserves left IDs/payloads and never multiplies them by
  right-match count. Bidirectional cached match IDs make retractions callback-free;
  right updates emit only membership transitions, and accepted left updates emit
  the current payload. Key buckets restrict pair predicates to relevant facts.
  Persistent candidate roots and output are prepared before installation, so
  ordinary callback/key/clone panics leave this standalone node unchanged.
  Memory is proportional to facts plus matching pairs; dense relations can require
  quadratic space. Empty right input retains every left row only in NotExists mode.
- `ExistsEqualNode` and graph `existence_equal` specialize pure equality
  existence/anti-existence. Constant keys express any right presence; tuple keys
  express multiple equalities. Persistent maps retain left rows and key memberships,
  right ID-to-key mappings, and one right count per key. Right payloads are discarded
  after projection. Memory is linear in input rows even for dense buckets. A right
  update visits left rows only when an old/new key becomes empty or nonempty;
  duplicate changes and same-key right replacements emit nothing. Visible left
  updates refresh payloads. Retractions use captured keys without callbacks.
  Outputs are ordered by left ID, including transfers between two keys. Candidate
  roots and output construction precede installation, as for the filtered operator.
  Pair predicates and self-exclusion continue to use `ExistsNode` explicitly.
- `FlatMapNode<I,T,U>` expands a changed input to a finite ordered sequence of
  payloads. Child IDs are `(input_id, zero_based_position)`, so duplicate payloads
  remain separate occurrences. Replacing an input first retracts positions past
  the new length, then replaces/inserts every new position. Reorder changes the
  payload at each position; IDs are not stable value identities. Empty expansion
  emits no rows. An absent input removal is a no-op; removal uses cached lengths
  without invoking the mapping. Mapping and iterator collection, output creation
  and persistent map preparation all precede installation, so ordinary panics
  preserve this standalone node. Only lengths are cached here; downstream nodes
  capture payloads. Memory is linear in input count, plus output events for the
  changed row. A mapping must be deterministic and return a finite iterator.
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
transactions. A call commits one node's input. Downstream nodes must treat a put
as replacement of the captured old payload, even without a preceding removal.
Intermediate downstream totals are not final graph results. Explicit
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

Fifteen adapted flatten/flattenLast methods across four arities preserve exact
ordinary scores and match multisets through removal. Uni and bi
flattenLastWithDuplicates remain pending; Java equal-object representative choice
is not claimed. Six original regressions check 2,000 graph edits (seed 20261109),
nested expansions, duplicate/nullable outputs, changing lengths/order, failed
mapping/iterator/scoring callbacks, structural-score errors, rollback/rebuild,
foreign ports and construction/search with independent objective checks.

## Typed graph transactions

`StreamGraphBuilder<S>` builds a fixed directed acyclic graph of typed sources,
filters, maps, one-to-many expansions, concatenations, complement, equality/existence joins, distinct, integer groups and named score terminals.
`concat(left, right)` returns `ConcatenatedStreamPort<L,R,T>`, whose occurrence
identity is `ConcatId<L,R>` with Left and Right variants. Payload types must match;
ID types may differ. Equal payloads remain independent occurrences, including when
both operands are the same port. Inspection orders left IDs before right IDs;
this does not imply event-delivery or callback order. Concatenation is a stateless
routing node: each input change becomes one branch-tagged output change with its
captured payload. It clones that ID/payload but stores no additional row cache.
Downstream caches and all affected input paths participate in normal transactions,
rebuild and saved-candidate installation. Each input is validated before registering
any node/edge. Native map closures explicitly pad or transform different tuple
arities into a common type; Option::None is a payload field, not a retraction.
Use distinct after concatenation when equal tuples should collapse. Nested
concatenations retain nested branch tags, so no global occurrence counter is needed.

Forty-four adapted concatenation methods preserve all initial/update match tuples,
multiplicities, padding, weighted group counts and totals across four arities.
Six original regressions check heterogeneous IDs, nullable payloads, same-port and
nested concatenation, foreign ports, failed transactions/rebuilds, construction/
search, and 3,000 seeded shared-source updates (20261110) against full tagged
multisets and group summaries. Existing performance workloads do not benchmark
concatenation, and no Java event trace or implicit node-sharing parity is claimed.

`complement(input, universe, input_key, universe_key, padding)` retains every input
occurrence and appends one padded row for each universe occurrence with no matching
input key. It returns the same tagged port shape as concat: Left(input_id) and
Right(universe_id). The first input for a key hides padding; only removal of the
last restores it. Duplicate universe keys have independent occurrence IDs. Input
keys absent from the universe remain unchanged. Coverage is determined before
padding; the mapper is responsible for any desired relationship between padded
payload fields and the universe key. All keys must have stable ordering.

This builder composition uses count-based equality NotExists, map and concat.
Its retained state is linear in input and universe occurrences, including dense
key buckets. It does not add a second transactional engine. Universe insert/update/
removal and input changes participate in ordinary graph rollback, reconstruction
and saved candidate installation. Both ports validate before topology registration.
Universe facts are explicit and can come from a shared source or another graph
pipeline. Assigned filtering is also explicit; there is no Java class discovery.
Use maps to express one-to-one tuple expansion or custom padding values.

Padding is called for emitted missing rows and must be valid for every universe
payload. As in other graph compositions, differently routed branches can expose
transient missing rows within one source update, even when the completed result
is covered. Callbacks must therefore be deterministic and without external side
effects; callback counts/order are not a Java compatibility guarantee.

Four adapted complement methods preserve all scores/matches, appended numeric
indexes and custom padding. Six adapted expandTo methods preserve all retained/
appended facts, sums and code strings through removal; documented fixed fixture
properties replace nondeterministic source values. Six original regressions check
3,000 transactions (seed 20261111) with one to four writes against full independent
complement multisets, nullable/duplicate/changing keys, shared-source transients,
foreign ports, failures/rebuild recovery and construction/search. Existing timing
workloads do not benchmark complement or establish Java event-trace equivalence.

`flat_map(input, mapping)` returns `StreamPort<(I,usize),U>` for any finite
`IntoIterator<Item=U>` mapping. Closures can retain a tuple prefix while expanding
its last element, or append new elements to the original tuple. Payloads are
emitted as supplied; equal objects are not replaced with a canonical instance.
Use explicit `distinct` for set semantics. Checkpoints share expansion-length
roots, including saved search candidates. Full rebuild replays source rows and
reconstructs lengths; no mapping is needed for rollback or candidate installation.
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
and their ID buckets, group inputs/collectors and terminal match records all use
shared AVL trees. Each graph terminal stores its payload and cached contribution
in one map, plus a scalar total; its checkpoint retains both. Standalone ScoreNode
continues to store contributions without payloads. Integer collector multiplicities
use the same storage. Cloning a map shares its root without cloning keys, values or rows;
updates copy shared search/rebalance paths and reuse uniquely owned nodes.
All key comparisons finish before mutation, using a bounded stack path. This keeps
comparison panics atomic even when an entire tree is uniquely owned. Rotations
and metadata updates invoke no user comparisons or key/value clones. Existing-key
replacement retains the original key object and allocates only the new value plus
any shared path nodes.

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

Mixed basic/list binding and search now share the graph transaction machinery;
see `docs/mixed-planning-contract.md`. Additional collector support is described
in the sections below.
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
finished result. `replace(old,new)` updates one captured occurrence; its default
uses an isolated remove/insert candidate. `Default` must be empty, `Clone` must isolate subsequent mutation,
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
on a key change, delegates same-key changes to `replace`, and emits changes for
affected groups. It prepares candidate
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


## Ordered-list move binding and search

`ListStreamSources<V>` registers two initially empty sources. `entities` maps each
entity index to `(index, Vec<V>)`, including empty lists. `values` maps each stable
global range index to `ListValueRow`: range index, value identity, optional
`ListPosition` (owner/index), previous value and next value. Range indices need not
equal value IDs. Every registered value remains present even when unassigned;
constraints explicitly filter `position.is_some()` to select assigned values.
Other graph sources can be preloaded with immutable facts.

`ListStreamScoreDirector` owns the assignment and graph. Initialization validates
the model, ownership and emptiness of both sources before callbacks. Applying a
move first validates the final composite through the native list transaction.
It delivers each touched entity list once, then each value belonging to those
lists before or after the move once. This conservative invalidation covers moved,
unassigned and neighboring values, including no-op list edits. Intermediate
entity/value joins may occur during delivery; the fully drained candidate exposes
consistent final rows. Source projection, callbacks and model errors or ordinary
panics roll back both graph and assignment. Commit performs no replay. Other
sources cannot be changed through the bound director.

The score includes minus the number of missing required values as structural
state; optional unassignment contributes no structural penalty. Read-only graph
inspection exposes ordinary named totals and typed matches. Full graph rebuild
diagnostics verify cached terminal identities/contributions and restore the
current graph after checking. The model and source projections are covered
separately by independent regression/reference comparisons.

`greedy_list_stream_construction` and `list_stream_local_search_from` share the
existing list algorithms through a private scoring interface. Early picks, lazy
neighborhoods, all five acceptance policies, entity ordering/tabu, fallback,
budgets and best-solution retention keep their scoped behavior. Each phase loads
its own sources and counts the initial scoring once. Terminal weighting calls are
counted separately; diagnostics and preloading immutable facts are excluded.
Selected candidates save assignment and affected graph roots and install without
replaying callbacks. Failures after saving an earlier candidate leave the base
unchanged. Full model snapshots and linear shadow lookups remain; no constant-time
move, thread safety or new Java performance claim follows.

Four adapted upstream methods check assigned/included-unassigned matching in both
required and optional models. Nine original integration tests cover 1,200 seeded
composites (20261024), span unassignment, cached joins/groups, unrelated callback
counts, immutable facts, validation at zero budget, ranges/pins, empty/impossible
models, 160 clone-failure budgets, source/assignment rollback and diagnostic work
accounting. Twenty-four construction configurations and 100 search configurations
agree with separate full-scoring list networks, including scores, assignments,
termination and candidate/step counts. One unit regression checks scorer errors,
scorer panics and acceptor panics after an earlier winner was saved. One doctest
and stream_list_routes demonstrate public API use.

The existing original Java list-move comparison now emits native cached stream
shadow rows, independently compared with Python and Java across candidate, undo
and committed states. It also checks native entity rows and a position score
inside the Rust adapter. No Java/Rust position-score comparison is claimed.
The existing required-list-solving benchmarks still use full scoring, and the
basic-variable stream benchmarks remain basic-variable workloads.


Four original replacement regressions check same-key updates on both join sides,
changed-key removals/insertions, 2,000 self-join updates against nested enumeration
(seed 20261025), downstream filtering/grouping, scorer errors/panics, transaction
undo, retained snapshots and zero/wrapping contributions. They add no upstream
method credit. The native profiling command in docs/dashboard.md measures node
work on an isolated instrumented source copy; use uninstrumented benchmarks for
runtime comparisons.


## Maps of collectors

`MapCollector<K,V,C>` is a reversible collector of mapped `(key, value)` pairs.
Each key owns an independent `C`, so it composes with `SetCollector`, `IntCollector`,
`CountCollector`, `PairCollector` or another `MapCollector`. `finish()` returns an
ordered immutable `SnapshotMap<K,C::Output>`. Key identity follows `Ord`; keys and
stored values must obey the same stable/immutable contracts as other collectors.

A separate occurrence count controls each key's lifetime. Zero sums, cancelling
values and empty inner results retain a key while occurrences remain; removing
the last occurrence removes that key. Removal expects a captured mapped value,
and missing-value detection follows the inner collector (CountCollector only
knows the number of occurrences). Absent keys return false without mutation.
The owning CollectNode already captures values and uses them on replacement/undo.

Only the changed key's inner collector is cloned and finished. Persistent group
and output maps share roots; finishing a map does not refinish its inner results.
Both roots are installed together after candidate work, preserving the previous
state on ordinary collector, clone or ordering panics. Custom collectors retain
the existing isolation/determinism requirements. Nested maps can be used directly
in global/keyed graph aggregation, transactional scoring, construction and search.

Twenty-five adapted methods from the uni/bi/tri/quad InnerConstraintCollectors
classes preserve map, sorted-map, merged-sum and update assertions, plus the bi
map duplicate-value method. Native integer summary sums replace the tested Java
Integer::sum result; these small-value tests make no Java int-overflow or arbitrary
merge-function compatibility claim. Global empty-stream semantics remain unchanged.

Five original regressions check 4,500 graph edits against independent per-key
multiset statistics (seed 20261026), retained nested snapshots, zero-sum keys,
last-key removal, missing values, wrapping sums, Send/Sync, callback rollback,
inner collector failures, 300 clone/ordering failure budgets and map-based
construction/local-search quality against a mathematical bound. These original
checks add no upstream method credit. Existing Java timing workloads do not
exercise MapCollector.


## Collected lists

`ListCollector<I,V>` collects unique stable occurrence IDs and mapped values.
Its immutable `ListSnapshot<I,V>` orders occurrences by ascending ID, retaining
equal values at different IDs. Duplicate IDs are rejected before mutation;
removal uses the captured ID (without comparing V) and returns false if absent.
The owning CollectNode handles replacement as old captured removal followed by
new insertion. This preserves relative order for an unchanged ID within a group.

`StreamGraphBuilder::collect_list` and `group_list` supply input tuple IDs
automatically, including composite IDs from joins. Group changes move an
occurrence between lists. Empty groups retract, and an empty global input emits
no row. Standalone empty list finish remains an empty snapshot. These operators
use the existing transactional aggregation machinery and integrate with basic/list
scoring and search; generic collectors otherwise retain their existing mappings.

The ordering is a native contract, independent of event delivery order. It also
makes order-sensitive score callbacks consistent with full source rebuilds.
Timefold's public collection API does not guarantee general list iteration order
([collection documentation](https://docs.timefold.ai/timefold-solver/latest/constraints-and-score/score-calculation)).
No Java encounter-order parity is claimed. The adapted tests assign IDs in their
original insertion order and preserve every exact list assertion, including
replacement position and duplicate removal.

Snapshots share persistent roots. Updates copy logarithmic tree paths; numeric
get/index uses cached subtree sizes in logarithmic time. Iteration is linear in
list length. len/is_empty, iteration, optional get and indexing are read-only;
indexing panics out of bounds. Equality, comparison, hashing and debug output
use the ordered values, ignoring occurrence IDs. Lists can therefore be used as
value keys in downstream distinct/grouping. Materializing a Vec or computing a
score over the entire list remains linear; these APIs imply no solver speedup.
Snapshots and collectors preserve Send/Sync when their stored types do.

Twelve adapted collector methods cover list insertion/removal and list/ArrayList
replacement across all four tuple arities. Native snapshots replace Java mutable
containers; custom Java collection factories are not an API compatibility target.
Six original regressions check 8,000 occurrence edits and all numeric ranks
(seed 20261027), 3,000 grouped/global graph writes (20261028), nonmonotonic IDs,
order-sensitive full rebuilds, rollback, callback/trait panics, nested maps,
retained snapshots, value equality/order/hash, downstream distinct and bounded
construction/search quality. The existing Java timing workloads do not exercise
collected lists.


## Integer averages and replacement semantics

`AverageCollector` collects signed i64 values and finishes to `Option<f64>`.
Empty means None; nonempty means convert the cached sum and count to f64 and
then divide. Results are binary floating point, not exact decimal/rational or
duration averages. The native count is usize. Integer input multiplicities allow
missing-value removal/replacement to return false without changing state.

The pinned Community public API probe establishes different arithmetic rules
for each operation: insertion checks sum + value, removal checks sum - value,
and replacement checks new - old before adding that difference to the sum with
wrapping arithmetic. For example, replacing -1 with 1 in [i64::MAX, -1] produces
a wrapped sum of i64::MIN. Conversely, replacing i64::MIN with i64::MAX is rejected
even for a singleton, because the difference overflows. No production code was
read or translated to establish these boundaries.

Native try_insert / try_remove / try_replace return AverageOverflow and preserve
state on rejection. The ReversibleCollector methods panic on these overflows;
graph transactions invalidate and restore checkpoints when dropped. After a
wrapping replacement, a full rebuild may reject an intermediate insertion sum.
Even a previously representable total can encounter an overflowing prefix when
replayed in source-ID order. Keep every required intermediate sum and replacement
difference representable when relying on full-rebuild diagnostics. These are
pinned arithmetic boundaries, not an unbounded mathematical-average guarantee.

CollectNode now delegates same-key replacements to the collector and keeps the
group's occurrence count unchanged. PairCollector delegates both components;
MapCollector delegates within the same map key and uses captured removal/insertion
when a key changes. This is necessary to distinguish replacement from removal
followed by insertion. Candidate collector state and finished outputs still
install together after successful work. Existing collectors inherit the isolated
default replacement unless they need a specialized rule.

Eight adapted average methods preserve all initial, duplicate, replacement and
removal results across four tuple arities. Five original regressions cover numeric
boundaries, missing values, nested replacement, overflow rollback, 2,000 grouped
updates (seed 20261030) and construction/local search. The new original standalone
AverageCollectorOracle and native average_oracle compare 137 sequences with 3,936
observations against independent Python rules, including exact IEEE-754 result
bits and expected rejections (seed 20261029). Each case stops at its first overflow;
Java post-error container state is not reused or claimed to be transactional.
These checks are separate from upstream credit and from stream lifecycle checks.
They run with differential_streams.py and appear as supplemental dashboard evidence.


## Signed duration aggregation

SignedDuration is an immutable, ordered i128 nanosecond value restricted to a
normalized i64 seconds component and 0..999999999 subsecond nanoseconds.
from_seconds is exact; from_nanos rejects values outside that range. Negative
values use floor seconds and a nonnegative fractional part (-1ns is -1s plus
999999999ns). This representation follows the public
[Java Duration contract](https://docs.oracle.com/en/java/javase/21/docs/api/java.base/java/time/Duration.html).
No parsing, calendar arithmetic or general java.time API compatibility is claimed.

DurationSumCollector and DurationAverageCollector retain persistent captured
multiplicities. Clone shares roots; missing removal/replacement returns false.
Sum finish returns zero for empty state; average finish returns None. Generic
keyed/global graph collection, map/pair composition, transactions, construction
and search accept both collectors without new graph node types.

Public Community probes establish that duration replacement subtracts the old
value before inserting the new value. Arithmetic checks normalized seconds
intermediates, even when carrying nanoseconds would yield a representable final
value. For example, adding -1ns to MIN+1ns rejects, while subtracting 1ns from
MIN+1ns yields MIN. Subtraction of a MIN-seconds value handles its unrepresentable
negated seconds through two representable additions. These boundaries are
verified against the pinned public API, without reading production code.

Duration averages first narrow the entire sum to i64 nanoseconds and only then
divide by count, truncating fractional nanoseconds toward zero. A total of
9223372036854775808ns therefore rejects at finish even with two inputs whose
average fits i64. This differs from unbounded mathematical duration averaging.
try_finish returns the error without mutation. Update try methods preserve state
on arithmetic rejection, but a successful update may leave an average whose
finish rejects. Trait finish/update methods panic on errors; graph transactions
invalidate and restore cached roots/model on drop. No Java post-error rollback
behavior is inferred. Full-rebuild diagnostics may reject different normalized
intermediate sums when source-ID replay changes insertion order; use values with
representable intermediates when requiring reconstruction to succeed.

Sixteen adapted methods preserve all assertions across four tuple arities.
Six original regressions add 5,000 arithmetic pairs (seed 20261101), 3,000 graph
updates with independent sum/average checks (seed 20261102), snapshots, failed
updates/finish, nested composition and construction/search quality. Original
DurationCollectorOracle and duration_oracle adapters compare 288 sequences and
7,650 observations against Python (seed 20261031), including 142 expected errors
with their update/finish stage. Each sequence stops at its first error. The
stream comparison saves this evidence separately; it adds no upstream credit or
duration timing claim. Existing search benchmarks do not exercise durations.


## Arbitrary-precision integer and decimal collection

BigIntSumCollector and BigIntAverageCollector accept the re-exported num_bigint
BigInt. Sum is exact and empty is zero. Average returns Option<Decimal>, rounded
half-even at scale zero. DecimalSumCollector and DecimalAverageCollector accept
Decimal, an exact coefficient * 10^(-scale) value with i32 scale. Decimal sums
start at scale zero; addition and subtraction retain the larger scale. Averages
round the coefficient at that cached scale, without any floating-point conversion.
The collector behavior is established through the pinned Community public API.
The representation follows the public
[BigDecimal value model](https://docs.oracle.com/en/java/javase/21/docs/api/java.base/java/math/BigDecimal.html).
BigInt storage/arithmetic uses [num-bigint 0.5.1](https://docs.rs/num-bigint/0.5.1/num_bigint/struct.BigInt.html),
not a Timefold implementation. Cargo.lock records the complete dependency graph.

Decimal::new takes a coefficient and scale; accessors expose both. Equality and
hashing include scale. Native Ord compares numeric values, then scale, so equality
and ordering stay consistent: 1.0 and 1.00 are distinct keys. numeric_cmp compares
value alone. It compares coefficient digits/exponents without padding across an
extreme scale gap. Display emits a lossless coefficient/exponent representation,
not Java formatting. There is no general decimal parser or score implementation.
Add/subtract align scales exactly, and divide_count rounds half-even without
changing scale (None for divisor zero). Arbitrary precision is resource-limited;
alignment of distant nonzero scales can require correspondingly large allocations.
No fixed digit bound is imposed. Comparing extreme scales needs only stored digits.

Collectors retain persistent captured multiplicities. Missing remove/replace is
false without mutation; replacement inherits isolated removal then insertion.
Snapshots share map roots, but numeric sum/input clones cost their digit count.
Decimal captures distinguish numerically equal values at different scales.
Map/pair composition, generic graph collection, transactions and solver callbacks
work with these values. Existing score types remain fixed-width integers; callbacks
may project an exact aggregate to a supported score with explicit range checking.
This does not implement arbitrary-precision score types.

Decimal precision is historical: adding then removing 0.00 from [7,0] changes the
average from 4 (scale zero) to 3.50 (scale two). Empty standalone collectors retain
the cached scale; empty average is None, while sum is scaled zero. Graph groups
are discarded at their last occurrence and a future group begins from Default.
Changing input scales can therefore affect the numeric result of future averages.
For history-independent objectives and full-rebuild diagnostics, normalize all
inputs to a fixed scale. A dedicated original regression confirms that a mixed-scale
history produces CorruptedScore on reconstruction and restores the prior cached
state. No Java graph-lifecycle equivalence is claimed by the standalone probes.

Thirty-two adapted methods preserve every upstream result assertion. Eight original
regressions exercise 1,001-digit integers, signed ties, missing captures, retained
snapshots, maps/pairs, panic rollback, numeric order, 5,000 arithmetic/comparison
pairs (seed 20261104), 2,000 graph changes (seed 20261105), rebuild diagnostics and
construction/search quality. BigNumberCollectorOracle and big_number_oracle
compare 304 original sequences / 16,298 observations against Python (seed 20261103).
Python recomputes exact totals from captures and historical scale rather than
incrementally updating a cached sum. Outputs include decimal coefficients and
scales; numeric equality alone cannot hide a representation mismatch. These checks
add no upstream coverage credit and do not benchmark arbitrary-precision arithmetic.

## Ordered comparison evidence

Twenty-seven adapted methods preserve five scalar equality/comparison truth tables,
20 mapped numeric joiner methods across bi/tri/quad/penta arities, and two index
orientation methods with exact memberships and sizes. The mapped fixtures retain
BigInt sums and scale-zero Decimal projections for the small source values; checked
i64 conversion is sufficient for those inputs and does not claim general Java
longValue overflow/truncation compatibility. Option replaces the two scalar null
equality assertions. Internal index selection/treeification tests remain pending.

Eight original regressions separately cover extreme keys, duplicate occurrences,
same-key payload replacement, callback-free retraction, 6,000 bidirectional edits
against full cross products, 3,000 shared-source transactions with commit/undo,
rollback after callback panics and structural-score errors, failed rebuild recovery,
foreign ports, group-sum rekeying and construction/local search. Sparse lookups in
both orientations visit fewer than 512 key comparisons among 4,096 opposite keys.
An internal bounded-map regression exhausts included/excluded/unbounded bound
combinations on old/new snapshots. Existing Java/Rust timing workloads exercise
equality joins; they do not measure ordered-join performance.

## Equality plus ordered comparison evidence

Ten adapted EqualsAndComparisonIndexerTest methods preserve all empty, insertion,
match membership, size, ordinary iterator presence/exhaustion and boundary checks
for gender equality with stored age <= query age. Native stable occurrence IDs,
query rows and peekable/Option iterators replace Java index handles and iterator
plumbing. Repeated-removal exception and random neighborhood iterator methods
remain pending; native absent retraction is intentionally a no-op.

Seven original regressions check 6,000 bidirectional edits (seed 20261114), 3,000
shared-source transactions (seed 20261115), exact matches through commit/undo and
rebuild, sparse queries across 4,096 ordered keys plus 4,096 unrelated partitions,
key clone/ordering failure sweeps, graph callback errors/panics, foreign ports,
grouped tuple equality keys and construction/local search. An internal churn test
checks partition and membership cleanup. Current Java/Rust timing workloads do
not exercise this partitioned operator; no new performance parity claim follows.

## Direct ordered and overlap join Community comparisons

Original public-API comparisons now exercise all five relations (equal, less than,
less than or equal, greater than, greater than or equal) without equality
partitions, with one equality key, and with two equality keys. Overlap comparisons
add unpartitioned and equality-partitioned intervals, including touching, zero-length,
reversed and extreme endpoints. Across 153 seeded cases and 10,155 states, Java Community SolutionManager.update and native
incremental graphs agree with independent Python pair enumeration on match counts
and weighted scores. Exact native pair IDs and both captured row payloads are
checked separately. Cases include duplicate/extreme ordered keys, payload-only and
key replacements, signed/zero weights, independent left/right edits, absent and
last removals, trials and undo; all cases finish empty.

The Java adapter uses public Joiners and score updates with problem facts and one
assigned dummy planning entity. It rebuilds a score session per observation; it
does not verify Java incremental notification traces or expose Community match
analysis. The native adapter checks full graph rebuild consistency after every
committed or undone operation. No new upstream method credit or timing claim.
Inputs and both outputs are retained under ordered_join_checks in the downloadable
stream differential report. Seed 20261116; scripts/ordered_join_checks.py.


## Equality-only existence counts

Four additional AbstractBiConstraintStreamTest methods preserve all original
initial/removal scores and matches for Exists/NotExists with zero or one equality
joiner. Ordered ID self-joins produce unique pairs; counts retain each accepted
pair once, regardless of right multiplicity. Stable IDs and explicit facts replace
Java fact discovery and reference identity. These tests do not claim filtering,
unknown-class diagnostics or Java notification-trace parity.

Original regressions independently enumerate 6,000 bidirectional edits and check
2,000 shared-source graph transactions; they cover duplicate counts, payload/key
changes, first/last removal, exact two-key transfer events, callback/clone/ordering
failures, terminal errors, tuple/optional keys and foreign ports. A dense case
with 16,384 left and 16,384 right occurrences completes without storing 268 million
pair links; counted key comparisons bound duplicate right work independently of
wall-clock timing. Complement now uses this specialization; its existing source
adaptations and rollback/solver regressions run against the new composition.

Original Java Community comparisons cover Exists/NotExists with zero, one and two
equality keys. Python nested enumeration checks both scores and native left
identities/payloads through edits, duplicate removal, commit and undo. Java uses
public ifExists/ifNotExists via score recalculation. This evidence does not add
method credit or measure Java incremental notifications, match analysis or timing.


## Snapshot allocation profiling and copy-on-write updates

The matched stream workload at baseline commit 6fd8b99 showed substantial allocator
work in Callgrind. Snapshot maps previously rebuilt every AVL path, even after
transaction-local updates made nodes uniquely owned. Arc copy-on-write now reuses
those nodes. Shared branches remain immutable and snapshots retain Send/Sync.
A stack search plan bounds depth by twice the usize bit width (a conservative AVL
height bound); all comparisons finish before any mutation. No unsafe code, user
key/value Clone requirement or runtime dependency was added.

The new ownership regression checks reuse through replacement, insertion/removal
and single/double rotations. The comparison-panic regression now checks uniquely
owned and shared trees, exact root identity and every retained entry. Existing
20,000-update AVL/reference-map tests, bounded ranges, payload sharing, graph
snapshots, collector states, saved search candidates and rollback tests remain
part of the full gate. This optimization adds no upstream method credit.

Local Valgrind 3.22.0 profiles used the original 32-variable hill workload with
50 steps, no warmup and one solve. Diagnostic builds used release optimization
plus debug information. Total allocated volume fell from 71,705,255 to 55,254,648
bytes; allocation blocks fell from 1,027,292 to 724,323 (29.5%). Peak live heap
was essentially unchanged: 172,661 versus 172,446 bytes. Callgrind instruction
references fell from 370,789,666 to 341,311,713. These whole-process diagnostic
counts are not elapsed-time or general memory-usage guarantees.

Separately, uninstrumented before/after binaries ran three fresh processes per binary and
workload, three warmups and five samples per process, alternating order. Every
sample was independently rescored; final assignments, scores and terminal callback
counts were identical before/after. Final measured medians on this shared machine:

| Workload | Before ms | After ms |
| --- | ---: | ---: |
| Hill climbing, 32 variables | 71.201 | 63.927 |
| Hill climbing, 96 variables | 280.967 | 249.524 |
| Tabu, 32 variables | 67.638 | 68.745 |
| Tabu, 96 variables | 251.331 | 217.973 |

Three medians improved; the 32-variable tabu median was about 1.6% slower. Small
wall-clock differences are not conclusive on this shared machine. This comparison
is against the prior Rust binary, separate from refreshed Java/Rust dashboard
benchmarks. Neither comparison establishes a general solver speedup or parity.
Raw samples, frozen inputs, before binaries, profile outputs and the local runner
remain in work/profiling/. The profiler was extracted there without changing
system packages. No original Timefold implementation was inspected or translated.


## Conditional collection

`ConditionalCollector<C>` delegates selected `Some(value)` occurrences to any
ReversibleCollector and captures rejected inputs as None. Rejected inputs remain
members of their CollectNode group. A nonempty group whose inputs all fail the
predicate therefore emits the delegate's empty result; only removing the last
input retracts the group. `Some(None)` can represent a selected nullable payload
when C accepts Option values, separately from a rejected outer None.

`group_collect_conditionally(input, key, predicate, mapping)` evaluates the key and
predicate on each put, invoking the value mapping only when selected. The global
`collect_conditionally` counterpart uses a unit group key. Decisions and mapped
values are captured by input ID, so retraction and rollback do not reevaluate
callbacks. Replacements can cross selection or group boundaries. Active-to-active
same-key replacement delegates C::replace, retaining specialized arithmetic
semantics such as AverageCollector's checked replacement difference. Other
transitions insert/remove one captured value. The wrapper counts rejected
occurrences to reject missing None removals, and isolates delegate mutations before
installation. It inherits C's requirements for independent clones and immutable
outputs; it cannot undo arbitrary shared interior state in custom collectors.

This uses the existing aggregation/transaction engine. Group counts include all
inputs, while the delegate counts only selected ones. Conditional collectors can
be nested in PairCollector or wrap a pair; filtering a stream before grouping
has different group-presence semantics. No conditional benchmark is claimed.

Eight adapted conditionally/conditionallyUpdate methods across all four arities
preserve every empty, duplicate, insertion, retraction and replacement assertion.
Native captured Options and stable IDs replace predicates/retractors/update slots;
zero-valued trailing tuple fields and the source-specific min/max choice are
preserved. Six original integration methods cover nullable selected values,
missing occurrences, arithmetic failure and replacement delegation, callback
laziness, captured removals, foreign ports, failed transactions, nested collectors,
construction and bounded local search, plus 6,000 seeded graph edits with independent
multiset summaries and commit/undo. Two public API doctests demonstrate the API.

A separate original Community public-API comparison checks 34 cases / 2,139 states
(seed 20261204) for selected global counts, keyed sums and global/group presence.
Java SolutionManager.update recalculates each state; Rust updates incremental
transactions; Python independently reconstructs scores from all source rows.
Rejected-only groups, duplicate values, replacements, key transfers, removals and
trial undo agree. No Java match-ID, incremental-notification or timing claim follows,
and these original cases add no upstream method credit. The evidence is included
in stream-differential.json and the dashboard's comparison details.

## Collection membership joins

`MembershipJoinNode` and graph helpers `join_containing`, `join_contained_in`,
`join_containing_any_of` and `join_containing_any_of_partitioned` implement native
set membership. Each input occurrence retains its captured distinct projected
keys. Optional equality partitions can use tuple keys for several equalities.
A scalar is projected as a singleton. Empty collections match nothing but remain
retained input rows; Option::None is an ordinary explicit key. The caller decides
how a missing collection maps to a collection of keys.

Each side has a persistent inverted index from partition and key to occurrence
IDs. Queries union matching buckets, so multiple shared keys and duplicate members
produce exactly one pair. Pair IDs are (left ID, right ID). Updates emit lost-pair
retractions followed by new/surviving puts in opposite-ID order. A surviving pair
gets one put even when its shared keys change. Projections run once per put and
never on retraction; same-key-set replacements retain index membership and update
payloads. Snapshot roots and captured key sets are shared. State is linear in rows
plus distinct per-row keys; transient match sets and dense outputs can be large.
Ord/Eq must be consistent and stable, and clones must not share mutable logical
state. Ordinary projection, comparison or clone panics preserve standalone roots;
graph transactions also restore changes if downstream processing fails. Neither
mechanism rolls back arbitrary external callback side effects.

Eight complete source methods cover size, forEach and remove-last/reuse in the
three collection indexer test classes. Native left-row emptiness expresses the
reuse assertion; Java entry handles, duplicate-handle semantics, isRemovable-only
methods, combined joiner index tests and random iterators remain pending. Eight
original integration methods check 3,600 bidirectional edits, 1,800 shared-source
transactions, exact payloads/pair IDs, callbacks, every injected key-clone/comparison
failure during a partition transfer, downstream errors/panics, rebuilds, foreign
ports, sparse queries and basic construction/search including saved prefixes.
An internal 2,000-replacement churn test checks bucket cleanup. These are correctness
and work-bound checks, not a membership performance benchmark.

The public Community comparison deliberately exposes a compatibility limit:
54 original cases / 4,420 states agree across Java, Rust and Python for distinct,
non-null collection members, all three relations and zero/one/two equality keys
(seed 20261207). Another 54 native cases / 4,420 states include duplicate/null
members and are checked against Python only. Java recalculates each score with
SolutionManager.update; Rust applies persistent incremental transactions. Python
also checks exact native pair IDs and original payloads, including member order
and duplicates. No Java match-ID or incremental notification/timing claim follows.

Eighteen fixed boundary probes characterize the pinned public API. The duplicate
probe has left members [7,7], right members [7], scalar 7 on both sides, equal
partition keys and weights 2/3. Java yields two matches and score (2,12) for
containing and any-of at all three equality arities; Rust yields (1,6).
Contained-in agrees at (1,6). The null probe has singleton [null] collections,
null scalars, the same partitions and weights. Unpartitioned Java containing and
contained-in return (0,0), while Rust returns (1,6); the other seven modes agree.
Thus eight of eighteen final probe states differ. These exact Java outputs are
validated and retained separately, never counted as passing compatibility cases.
The native contract consistently deduplicates pairs and permits explicit null
keys. These public-join differences do not invalidate the eight narrower adapted
indexer methods, but do rule out blanket membership-join compatibility. The
report and dashboard expose the distinction; no upstream production implementation
was inspected to explain or reproduce the Java behavior.


## Indexed single-valued references

`StreamGraphBuilder::follow_reference(left, right, reference)` follows an optional
right-row ID from each left payload. Its output preserves the left identity and
contains `(left, Option<right>)`. Every left row remains present: an absent reference
or missing target produces `None`; a present target whose payload is optional can
produce `Some(None)`. Inserting/removing a target updates existing dependents.
Changing a reference removes the old inverse membership before installing the new
one. Target events visit only current dependents, in left-ID order. Repeated target
puts refresh their dependents; retracting an absent target emits nothing.

Compose references and maps to express derived fields such as `entity.group.owner`.
This is an explicit native graph, not reflection or an annotation processor.
Topology is acyclic even if payload IDs refer to themselves or each other; each
operator follows exactly one edge, with no recursive cycle/fixed-point evaluation.
Downstream callbacks may observe intermediate updates during composite moves, as
with other stream operators; completed candidates and commits are consistent.

Persistent left rows, targets and reverse memberships participate in graph
checkpoints and saved-candidate installation. Drop/unwind restores them without
replaying callbacks. The standalone `ReferenceNode` stages each update before
installation, including mapping/clone failures. Stable immutable identity and pure
callbacks follow the existing graph contract. Bound basic/list/mixed sources can
feed these operators; verified integrations include basic construction/local search,
composite basic moves and list previous-element snapshots. This adds no dedicated
reference performance or general declarative-shadow compatibility claim.


## Explicit dependency evaluation

`DependencyGraph<K>` separates immutable fixed dependencies from a list of
`(variable, dependency)` edges supplied for each evaluation. Declare every key
up front. Unknown keys and duplicate declarations are errors; repeated edges are
deduplicated. Fixed cycles are rejected at construction with exactly their cycle
members in key order. Acyclic inputs can be evaluated with a callback receiving
only their immediate prerequisites, in key order, and immutable captured facts.

`DerivedValue<V>` has independent `value: Option<V>` and `inconsistent: bool`
fields. Returning None does not mark a value inconsistent. Dynamic cycles and
all values depending on them receive None with inconsistency set and never invoke
the evaluator callback. Other ready variables evaluate in deterministic key order.
A later evaluation without the cycle recovers. The algorithm does not recursively
solve cyclic equations; dependency traversal uses explicit stacks, including fixed
cycle discovery. Output values need not implement Clone.

The plan is immutable and reusable after errors or callback unwind. Every dynamic
edge is validated before any callback runs. Callbacks must be pure: external side
effects are not undone. Each call recomputes the schedule and returns a new map.
Inside scoped or stream score callbacks this supports task times derived from fixed
dependencies and current list predecessors. Existing score directors cache and
restore resulting contributions on trial/undo; they do not store these projections
as mutable fields on user entities. Incremental recomputation and dependency-specific
performance claims are outside this implementation.

## Shared reference suppliers

`follow_shared_reference(left, right, reference, supplier)` derives one value per
occupied reference group, using its smallest left tuple ID as representative.
A target update invokes the supplier once per occupied target; an unreferenced
target invokes none. Every left tuple receives `(left, supplied_value)` under its
original identity. A missing target is passed to the supplier as `None`; nullable
supplier outputs can themselves be `Option` values. Alignment is a caller promise
that all members intend the same derived value. Membership changes recompute the
affected groups, even when their supplied values compare equal.

This composes persistent ordered group collections and reference indexes. During
routing, outputs may retract/reinsert or temporarily reflect intermediate paths;
the full derived state is available after each source update completes. Scoring
callbacks must be pure. Graph rollback and saved-candidate installation restore
all caches without re-invoking suppliers, but do not undo external callback side
effects. Key/payload identity must remain stable. Graph topology stays acyclic;
cyclic payload references are finite one-hop lookups, not recursive evaluation.
There is no automatic discovery of alignment keys, no guarantee of Java's general
representative choice, and no coalescing across separate source updates.
Keys are ordinary Rust values: using `Option` as a key would group `None` values
together. Java's special rule that null alignment keys each compute separately is
not inferred; callers must supply distinct keys for such unaligned rows.

Three attributed follower methods cover solving, value changes and exact shared
supplier counts. Six original regressions additionally compare generated updates
with full scans and construction/local search with direct scoring, including
rollback, unwind, representative replacement, missing targets and same-source
references. Generated Java follower evidence is recorded in the next section;
no dedicated performance comparison is claimed.

## Dynamic follower conformance and generated comparisons

Native basic-variable reference graphs now have all three complete dynamic
follower method adaptations, plus the existing fixed/shared follower adaptations.
A generated public-API comparison in `follower_checks.py` covers 128 fixed/shared
and dynamic models through 2,373 initial/trial/undo/committed states and 3,714
primitive operations. Java maintains annotated shadows with public MoveTester;
Rust maintains graph projections. Independent Python calculations verify all
references, derived values, ordinary scores and completeness flags. Java scores
are recomputed for each observation; there is no timing or incremental Java score
claim, and these generated cases grant no source method credit.

The public runtime exposed an interface-source distinction: in these fixtures,
`forEach(HasValue.class)` includes rows with null genuine variables. Native fixture
scoring therefore groups all leader and follower values, including nulls, rather
than applying the usual concrete-entity assignment filter. The previous static
fixture's partial scores are corrected and verified directly against Java. Native
explicit source filtering remains under the caller's control; no automatic Java
class/interface discovery is implemented. Shared cases also check missing fixed
references for values and scores only; no Java null-key supplier counts are claimed.

## Collection-derived follower values

A reusable attributed test/example fixture combines indexed collection membership,
conditional ordered collection, and reference projections for fixed collections of
leader values. The ordered key is `(UTF-16 code units, first reference position,
value identity)`, so minima retain Java String order and the first equal-code
identity. Duplicate references remain semantically harmless to this minimum;
all-null groups retain a None result. Every follower remains in the interface
score, including null rows. Inputs and code strings are fixed for a graph instance;
leader values update transactionally and cached minima participate in undo and
saved search candidates. No new production operator was needed.

Two complete upstream methods cover changes and solving; two original native
regressions cover Unicode/tie handling, undo and search/full-score agreement.
Another 128 Java/Rust/Python collection-follower cases observe 2,368 states across
3,724 operations, including repeated references, equal-code identities, empty
codes, all-null states and composite temporary moves. Java updates annotated
shadows using public moves and recomputes each observed ordinary score. No direct
Java timing, supplier-count, PlanningId, code/dependency mutation or empty/null
collection compatibility follows from these comparisons.

## Ordered list-element projections

An attributed test/example fixture uses per-owner maps to scan list elements in
order from a fixed start time, retaining each element's start/end and the owner's
last end. Empty owners retain their fixed start. Reference projections connect
all value rows, including unassigned rows, to their owner's cached chain. Only
changed owners rescan; updates do not use a prefix-difference optimization.
Java integer addition is modeled with wrapping i32 arithmetic. Compared generated
inputs use nonnegative starts and positive small durations, without overflow.

A second fixture maps optional genuine durations to padded durations, joins list
membership, and groups nullable sums. A null member yields None; an empty owner
yields Some(0). Basic changes therefore update owner aggregates without a list
change. Both projections participate in ordinary graph transactions, undo and
saved search candidates; no new production operator was required. Six upstream
methods, three original Rust regressions and 128 separate public Java/Rust/Python
cases establish the scoped behavior. Mixed comparisons observe values only;
ordered Java scores use a detached full-update solution to leave move-driven
shadow observations untouched. No incremental Java scoring/timing claim follows.

## Explicitly aligned dependency evaluation

DependencyGraph.evaluate_aligned forms transitive classes from explicit key pairs.
Each class uses its smallest key, unions fixed and dynamic dependencies, and calls
one supplier with sorted members and canonical distinct inputs. Its result is
cloned to all class members. Dependencies inside a collapsed class create cycles;
cycles and downstream classes produce inconsistent absent values without callback
calls. Validation precedes every callback. The original evaluate method still
supports non-Clone values. Both APIs rebuild their schedules each evaluation;
this is not an incremental dependency cache or automatic alignment inference.

A concurrent-schedule fixture aligns start variables while ready/finish variables
remain distinct. An inconsistent activity can retain an independent ready time;
only cycle-dependent fields become absent. Shared group starts can reach unassigned
members when other members are assigned. The stream fixture projects a complete
list snapshot through an owned dependency cache. Its separate full projection
remains a reference evaluator. Temporary duplicate
ownership while owner rows arrive is normalized privately; the validated final
assignment has unique ownership and receives the complete projection. Candidate
installation and undo preserve every final cached derived field and score.

Seven adapted methods and original exhaustive/transaction tests establish native
behavior. Generated comparisons retain 107/128 Java agreements and 21 runtime
error cases, while all 128 native cases match independent Python. Errors are shown
separately and not counted as passes; an unchanged-fixture probe reproduces the
partial-group null-finish/scoring boundary. Java scores come from detached full
updates, leaving observed move-driven shadows untouched. No Java timing claim.

## Persistent dependency topology

`DependencyTopology<K>` keeps a fixed set of domain-variable keys and supports
atomic batches of edge removals/additions in the same node-to-dependency direction
as `DependencyGraph`. Query each node's sorted strongly connected component,
component rank, dependencies and cycle membership. Dependencies have lower ranks
than their consumers unless both are in one component. Rank numbers for unrelated
components are only a deterministic implementation choice.

Updates return the sorted keys whose cycle membership toggled. A downstream node
can have an inconsistent derived value without being cyclic; splitting one cyclic
component into two cyclic components can therefore return no changed keys. This
return value alone is not a dirty-value set. Every changed incoming dependency
also requires reevaluating the consumer. Batches validate every endpoint first;
removals apply before additions. Duplicate additions and absent removals are no-ops.
Clone snapshots may be retained for trials. Components and ranks currently rebuild
after actual edge changes. Derived values are maintained separately by
`DependencyCache`, described below.

## Incremental derived values

`DependencyCache` builds initial values from a `DependencyTopology`, then accepts
atomic removals/additions and explicit dirty keys. Suppliers receive sorted
node-to-dependency inputs. Changed values schedule direct consumers in topological
order; equal results stop propagation. Cycle members and downstream inconsistent
values skip suppliers. A consistent unavailable value may still be consumed by a
supplier. Dirty every key reading a changed external fact or changed formula.
The cache cannot infer changes to captured data.

Snapshots share topology and persistent value roots, even for non-Clone values.
Value-only updates avoid scanning all keys; changed edges rebuild components and
reverse indexes. Metadata failure or ordinary supplier/equality unwinding leaves
the cache untouched. External side effects and panicking destructors retain normal
Rust limitations.

For stream scoring, `derive_dependencies` accepts a fixed key universe, a row-to-
dependency mapping and a supplier. Output rows retain `(source_row, DerivedValue)`.
Missing targets are consistent None; source retraction removes its incoming edges
and output, then propagates any value change. Cyclic rows remain in the output with
an inconsistent absent value. Source metadata updates emit even when derived values
are equal, while dependent rows only emit when their derived value changes. The
node owns/checkpoints both rows and cache, so trial undo and saved candidates restore
state without replaying suppliers. A plain stateless map cannot provide this cache
rollback guarantee.

## Owned projection caches

`map_cached(input, initial, project)` preserves input identities and stores one
owned cache per identity. The callback may update that cache to avoid recomputation,
but its result must equal projecting the current input into a fresh initial cache.
State must not give outputs history-dependent meaning. Cache Clone must produce
independent snapshots; sharing immutable storage is allowed, shared interior
mutation is not. Only the affected cache is cloned on update; transaction checkpoints
share the persistent state-map root. Cloning a user cache may itself be expensive.

Retraction discards that identity's cache without callbacks, and reinsertion starts
fresh. Undo, downstream failures and saved candidates restore caches and outputs.
Diagnostic rebuilds clear cache state before replaying current source rows. The
existing stateless map retains its original behavior.

Concurrent scheduling now projects complete normalized list snapshots through
map_cached and CachedSchedule. Dirty ready keys and predecessor edge changes are
applied together, then every complete projected schedule is scored against the
independent group-closure reference. Complete snapshots avoid asserting schedules
assembled from partially updated phase rows. List snapshot construction and
edge-change topology rebuilding still visit the full relevant structure; cached
suppliers do not imply constant-time moves or measured speed parity with Java.


## Indexed dependency scheduling

DependencyCache uses fixed internal indices for reverse links, dirty roots,
component-rank lookups and the ready queue. Indices follow sorted declared keys,
so supplier inputs, deterministic callback order and returned changed keys retain
their key-based meaning. Public snapshots remain SnapshotMap<K, DerivedValue<V>>;
indices never escape as user keys. Supplied dirty keys resolve to declared topology
keys. Scheduling consumers no longer clones their keys or repeatedly searches the
key-to-index map. Edge changes still rebuild topology and reverse links.

A 1,024-consumer regression uses shuffled string keys and checks full callback
order, equality cutoffs, negligible key cloning during scheduling, and atomic
rollback when cloning a final changed key panics. Existing exhaustive graph,
stream transaction, saved-candidate and public concurrent checks remain required.

## Consecutive sequences

`SequenceCollector<I,P,V,M>` consumes captured `(logical_id,index,value)` triples
and implements `ReversibleCollector`, so it works with global/keyed, conditional
and composed aggregation. Repeated IDs retain occurrence counts but appear once
in the result; distinct IDs at the same index remain distinct. Repeated IDs must
have equal captured index and value. Remove all occurrences before changing that
identity; a slot can instead replace its old identity with a different one.
Removal looks up the captured ID and ignores the passed index/value. Conflicting
duplicates panic atomically; graph transactions restore their checkpoint on drop.

`SequenceMetric<P>` defines zero, positive maximum gap, endpoint distance and
addition. Adjacent ordered points connect when their distance is in
`[zero,maximum_gap]`. Sequence length is endpoint distance plus maximum gap;
break length is endpoint distance alone. Metrics must be deterministic with
immutable state and arithmetic consistent with index ordering. Integer metrics
use i64 positions and exact i128 distances, with a const u64 step (default 1).
Custom metrics can return SignedDuration, as the adapted one-day UTC fixture does.

`SequenceChain` owns an immutable snapshot of ordered items and full sequence and
break metadata, including endpoint items, counts, lengths, first/last flags and
neighbor break indices. Equal-index items use ascending native logical IDs. Java
identity-hash ties, live views and mutable object-field aliasing are not reproduced.
Values and callback captures must remain immutable, as with other stream rows.
A standalone empty collector finishes to an empty chain; an empty global stream
has no aggregate row. Clone shares persistent state; mutations stage both indexes.
Finish currently visits all distinct items and looks up each captured value; it
does not maintain chain boundaries incrementally. The standalone benchmark in
`docs/dashboard.md` measures replacement/inverse updates plus full observations;
it does not isolate insertion/removal cost or measure graph rollback.

The separate public comparison checks 64 original cases/4,689 states through
add, replacement, removal and temporary changes restored by inverse slot updates.
Independent Python checks every item and metadata field; Java tie order may vary
but endpoint identities must match its observed items. One additional boundary
is explicitly divergent: Java's break from int MIN to MAX has length -1, while
Rust returns exact 4,294,967,295. It earns no ordinary agreement credit. Native
regressions additionally exercise i64 endpoints, transaction undo/rebuild, failure
recovery and actual construction/search against independent full scoring.

## Connected ranges

`ConnectedRangeCollector<I,P,V,M>` consumes captured `(id,start,end,value)`
occurrences. Endpoints must satisfy start <= end. Repeated IDs require equal
captured fields until all occurrences have been removed; independently changing
occurrences can use separate IDs. Retraction uses only captured identity, even
if the caller now supplies changed endpoints. Every occurrence remains in the
result, including duplicates, and contributes to overlap on nonempty segments.
All generic global/keyed, conditional and composed aggregation APIs are available.

Touching and overlapping closed endpoint spans form components. Overlap counts
measure their half-open interiors: touching endpoints do not double-count, and
zero-length intervals contribute no overlap. Empty-only components have
`None` minimum and maximum overlap; otherwise each bound is `Some(usize)`.
`has_overlap` means maximum overlap exceeds one. Component length and gap length
come from a pure `RangeMetric`; `IntegerRangeMetric` subtracts i64 endpoints
exactly into i128. Custom metrics can use SignedDuration. Reversed ranges are
rejected atomically. No Java int-overflow behavior is reproduced.

ConnectedRangeChain owns immutable components and gaps. Component items sort by
start/end/ID, repeating each occurrence; complete native values, including IDs,
participate in equality/hashing. Saved snapshots and hashes remain unchanged.
Java live views, identity tie order, mutable object aliases and live-object hash
mutation are not reproduced. Standalone empty collection yields an empty chain;
empty stream groups have no aggregate row. Clones share persistent captured maps;
finish sorts distinct ranges, expands occurrences and sweeps endpoint events.
It rebuilds components/overlap metadata rather than maintaining boundaries
incrementally. No dedicated connected-range performance claim exists yet.

Public Java/native/Python comparisons retain all 197 original cases and 11,583
states. Native fields match independent connectivity/midpoint counts everywhere.
Java agrees on all 64 distinct-identity positive-length cases, and 65/197 cases
overall after mapping its empty-component MAX/MIN sentinels to native absence.
The other 132 cases differ; none gets agreement credit. Shared-object duplicate
removal can leave a Java occurrence behind, empty-range overlap flags can disagree
with maximum counts, and integer lengths can overflow. Inputs and full differences
are retained. All original shared-identity cases were kept after adding the distinct
cohort to isolate the cause. There are no Java runtime-error cases in this run.

## Load balancing

`LoadBalanceCollector<K>` consumes captured `(key, load: i64, initial_load: i64)`
occurrences. Each active key totals its initial load once plus every occurrence's
load. Signed and zero loads are valid. Zero-load keys remain until their final
occurrence is removed. Only present keys participate; insert zero-load baseline
rows for idle resources that should influence fairness. All generic keyed/global,
conditional and composed collection operators accept this collector.

Simultaneously active occurrences of a key must have equal initial loads.
Inconsistent inserts/replacements panic atomically; a changed initial load is
allowed after all old occurrences are removed. This gives history-independent
rebuild/undo. A public Java probe instead retains the first initial value while
any occurrence remains. Retraction matches the exact captured triple, with
multiplicity. Missing removal/replacement returns false without mutation.

Native totals, total sum and squared moments use BigInt. Unfairness equals
`sqrt(sum((load - mean)^2))`, rounded once to six significant digits, nearest with
ties to even. Zero returns Decimal::ZERO; other results strip trailing zeros.
Integer square root and exact squared-midpoint comparisons avoid binary floats.
Java's observed numeric format uses six significant digits, although LoadBalance
Javadoc says six decimal places. General Java decimal-scale representation is not
preserved: comparisons normalize trailing zeros. Native full-width arithmetic
intentionally avoids Java overflow and cancellation. Immutable LoadBalance
snapshots expose loads and unfairness; Java live-map aliases are not reproduced.

Updates maintain exact moments and persistent capture/key maps; finish shares the
load-map root and computes the rounded square root. There is no dedicated timing
claim. An empty standalone collector produces an empty map and zero; an empty
stream group produces no aggregate row. Inverse updates are exercised in public
comparisons; native graph regressions additionally check rollback and rebuild.

## Calendar-period sums

`CalendarPeriod` holds independent signed i32 year, month and day fields. Constructors,
component access, equality/hash, checked addition/subtraction and display preserve
those fields. `12 months` and `1 year` remain distinct. This follows the public
[Java Period arithmetic contract](https://docs.oracle.com/en/java/javase/21/docs/api/java.base/java/time/Period.html#plus(java.time.temporal.TemporalAmount)).
There is no automatic normalization, conversion to elapsed seconds or chronological
ordering. Applying amounts to calendar dates, parsing and time-zone handling remain
outside this value type. Display uses P with nonzero signed fields; zero is P0D.

`PeriodSumCollector` records every captured occurrence, including zero, in persistent
maps and caches a checked component-wise sum. Empty standalone finish is ZERO;
empty stream groups emit no row. All generic global/keyed, conditional and composed
collection operators accept it. `try_insert`, `try_remove` and `try_replace` return
PeriodOverflow without changing state. Missing removal/replacement returns false.
Trait methods panic on arithmetic error; graph transactions restore their checkpoint.
Clone shares occurrence roots, while outputs are owned Copy values.

Replacement subtracts the old amount then adds the new one. Both intermediates must
fit i32 in every field, even when the final mathematical sum is representable or the
replacement value equals the old one. A public Java same-reference probe confirms
this: day amounts -1,MAX,1 total MAX; replacing -1 with itself throws on subtraction.
The comments in source update tests calling repeated replacement a shortcut are not
generalized beyond their bounded assertions. Native preserves its state on error;
Java post-error state is not assumed. Subtracting MIN from MIN succeeds and gives zero.
Keep all arithmetic prefixes representable for history-independent rebuild checking.

Original comparison retains 227 cases/8,771 observations, all agreeing with Java and
independent Python on components or expected overflow. There are 120 expected
rejections, each ending that case's observed prefix; the unexecuted operations remain
in its stored input. Successful observations also match independent full sums from
active captures. Inverse-update trials compare standalone collectors; separate native
tests exercise actual graph undo/rebuild and solver candidates. No timing claim.

## Key-projected extrema

MinByCollector<I,K,V> and MaxByCollector<I,K,V> accept captured `(id,key,payload)`
occurrences. Keys and IDs implement Ord; payloads only need Clone and PartialEq.
An owned `Option<Extremum<I,K,V>>` returns the selected identity, ordering key and
payload. Standalone emptiness is None; an empty grouped stream emits no group row.
Both directions choose the smallest active ID among tied keys. Use Reverse, tuples
or an Ord newtype to define other key orderings. Comparisons must remain pure,
consistent and immutable while keys are stored. No chronological ordering is
invented for calendar values or other non-Ord payloads.

Map and capture keys when inserting/updating source rows. Retraction uses the stored
ID and captured fields, so no projection runs again on removal or finish. Duplicate
IDs must carry equal keys/payloads while active; the last removal deletes the ID.
Use distinct occurrence IDs for independently mutable rows. Removal and replacement
of a missing ID return false. Conflicting insertions/replacements panic atomically;
replacement stages removal and insertion before installation. Persistent indexes
share roots across clone/checkpoint and return immutable selected snapshots. Finish
looks up endpoint keys and lowest IDs without scanning all values.

Pinned public ConstraintCollectors.min/max comparable-property Javadocs permit any
tied value and allow that value to vary with incremental history. Native tie IDs
are deliberately deterministic for rollback/reconstruction. Comparison evidence
checks that Java's selected identity is active, its payload matches its capture,
and its key is extreme; differing valid tied IDs are explicitly retained. No Java
ABI, exact tie history, live aliasing, callback-order or dedicated timing claim.
