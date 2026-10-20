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
  remains the planning model's responsibility. Snapshot rows provide match tuples;
  retained graph and scoped directors additionally expose named justifications and
  indictments as described in the solver contract.

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
ordinary scores and match multisets through removal. The explicit canonical
expansion operator described below retains first equivalence-key representatives
for duplicate outputs; ordinary flat-map keeps its existing identity behavior.
Six original regressions check 2,000 graph edits (seed 20261109),
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
`constraint_matches` exposes each tracked terminal's exact typed tuples and ordinary
scores. Constraint totals are named and sorted; the graph total excludes planning
structure. Nonzero structural contributions are rejected.

The explicit `score_only`, `score_only_weighted` and
`score_only_weighted_decimal` builders instead retain each occurrence's ID and
contribution score without its tuple payload. Full input source and upstream
operator rows remain retained. Ordinary `score` and weighted builders retain
their existing payload/inspection behavior. Score-only totals, occurrence-count
metrics (including zero contributions), rollback, saved candidates, consistency
diagnostics and live weight/activity replay remain available. Scoring callbacks
still execute and are counted for each delivered put, including replacements;
retractions use retained contributions. Existing scheduling, batch coalescing,
disabled branches and diagnostic/replay callback rules apply.

`constraint_matches` and tuple-based explanations for score-only handles return
`GraphError::ConstraintMatchesUnavailable` with the constraint name, including
empty/disabled constraints. The terminal omits payload Clone/storage/Drop
operations, so their user-defined side effects and panics differ when opting in;
source/upstream cloning remains unchanged. The new public error variant must be
handled by external exhaustive matches. This capability alone establishes no
speed or memory reduction for a whole graph or solve.

A transaction owns an exclusive graph borrow and drains each input change through
an iterative FIFO queue before returning. Node output order is preserved through
fan-out, including both inputs of a shared self-join and paths of different
lengths that later rejoin. Callbacks may see intermediate tuples during delivery;
only the fully drained result is a completed candidate. A multi-input transaction
can inspect results between its writes, commit all writes or undo them together.

Before the first update to each affected node, the transaction normally saves that
node's persistent map roots and scalar state, including indexes, summaries and scores.
Sources, filters, terminal matches, datasets and indexed batched joins use
reversible row journals as described below. Generic saved candidate snapshots
share roots; bounded planning nodes instead copy the limited slots described below.
Further updates reuse that checkpoint. Generic sources and filter inputs, both join
indexes and their ID buckets, group inputs/collectors and terminal match records
use shared AVL trees. Bounded planning storage is described below. Each tracked graph
terminal stores its payload and cached contribution
in one map, plus a scalar total; its checkpoint retains both. Standalone ScoreNode
continues to store contributions without payloads. Integer collector multiplicities
use the same storage. Cloning a map shares its root without cloning keys, values or rows;
updates copy shared search/rebalance paths and reuse uniquely owned nodes.
All key comparisons finish before mutation, using a bounded stack path. This keeps
comparison panics atomic even when an entire tree is uniquely owned. Rotations
and metadata updates invoke no user comparisons or key/value clones. Existing-key
replacement retains the original key object and allocates only the new value plus
any shared path nodes.

An ordinary join receiving a protected basic or mixed planning source can use
private indexed row slots when the source and join side both contain exactly the
contiguous `usize` IDs `0..count`, with at most 256 rows. Each side qualifies
independently. Private identity metadata follows ordinary map/filter operators;
joins, grouping, concatenation and expansion stop it at their changed output
namespace. Ordinary facts, larger inputs and batched joins retain map storage. Join membership indexes
still use maps; key projections, payload clones and emitted tuple order retain
their existing behavior. Trial frames retain the first original Arc per changed
slot, merge originals on nested commit and release intermediate replacements.
Full candidate snapshots copy at most 256 row slots and their key handles per
eligible side, sharing the underlying keys and payloads. Undo lookup and nested
merge are bounded linear scans of touched slots, not constant-time
operations. Restoring a saved candidate inside an outer trial preserves that
trial's originals. This private representation adds no public ID restriction.

An ordinary graph filter can use the same private row slots when its initial
input cache contains every protected planning ID `0..count`, with at most 256
rows. Rejected rows still belong to that cache: their predicate result is retained.
An initially sparse downstream filter keeps its map and can later accept unseen
IDs. Ordinary facts, larger inputs and standalone `FilterNode` instances also keep
maps. Bounds follow only the existing identity-preserving map/filter path and stop
at operators that change the occurrence namespace.

Binding preserves the original key/payload owners and iteration order. Updates
keep the existing predicate and output-clone order; retraction and rollback do not
rerun predicates. Full snapshots copy at most 256 shared key and payload handles;
trial frames retain first-touched originals and merge nested commits. Restoring
full snapshots inside an outer trial preserves its undo. Full snapshot payloads
are boxed separately so frequent journal tokens do not allocate the larger full
snapshot representation. This is a checkpoint-cost tradeoff, not constant-time
snapshotting or a general memory-reduction claim.

A four-fork pinned release comparison observed a 10.48% lower large hill/swap
median, improving 7.01–15.02% in every fork with exact assignments, scores and
measured callback counts. The Dense control also fell 3.55%, so the entire timing
difference cannot be attributed to this change. Large changes improved overall
but one fork was slower; Mixed Stream varied. These results do not establish a
general speedup or Java parity. Actual binaries, inputs and source hashes are in
`work/round238-bound-filter/{artifact-receipt,paired/results}.json`.

Dropping restores owned roots and scalar state with no predicate, key, scoring,
cloning or comparison callbacks. Commit keeps current roots with no callback
replay. Generic root checkpoint/candidate-state storage scales with affected nodes
rather than all cached rows; bounded slot snapshots have the costs described above.
Updating a tree entry takes logarithmic path work; joins still
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

Ownership-consuming basic local-search calls (`stream_local_search`,
`stream_local_search_from`, and their direct provider/fallible-provider forms)
may select private copied source and contribution storage. Permission requires
an initially empty graph, known ordinary nodes, exact certified builtin IDs,
keys and payloads, a builtin fixed integer score, and no activity or batching
mode. Every eligibility check precedes activation; populated, custom and unknown
graphs retain their original storage. The public director constructor, retained
search, monitored manager/service routes and public graph transactions do not
select this permission. The consuming call never returns or lends its graph to
providers, termination hooks or solution observers.

Inside that scope, private terminal history retains IDs and contribution scores
by value, without retaining tuple payload owners. Every delivered predicate,
projection, mapping and scoring callback still runs in the original event order;
terminal payload Clone/discard occurs at the original post-check publication
phase. Each logical write keeps its own preimage; nested rollback, saved current
candidates, full snapshots, checked arithmetic, metrics and diagnostic replay
remain supported. Diagnostics compare terminal IDs, scores and totals and return
detached evidence. Public pre-call empty views remain empty; externally held
historical row snapshots remain independent. No callback purity is inferred from
lack of Drop, and no public match/snapshot ownership promise is relaxed on a
retained graph. Unbounded contribution IDs use sparse numeric pages; retired
candidate pages are pruned after journal completion instead of accumulating.

Within that same private consuming scope, an exact builtin bounded join may
copy captured payloads and journal preimages after the original complete ordinal
binding. Canonical input IDs and membership indexes keep their original owners;
unbound/pair-ID/custom/retained paths keep shared payload storage. Full/current
captures and first-preimage nested rollback remain supported. Copies trade wider
private slots for fewer recurring payload allocations.

In both retained and consuming graphs, fixed primitive equality joins may visit
matching bucket bits directly after the original single key lookup and full
output reservation. Bit order retains
canonical input IDs and the existing retract/put, payload Clone/Drop/panic and
poisoned-install phases. Custom key comparisons and ordered joins retain their
previous traversal. This does not imply callback purity or event coalescing.

Within the strict private consuming scope, an equal ID-to-score contribution
may omit only its final store and undo entry. Every callback, structural check,
old/new score clone, checked adjustment, final ID/payload Clone and discard phase
still runs. Present zero and absence remain distinct. Public retained terminals
continue to publish fresh owners even for equal rows; full/current/nested state,
match counts, metrics and diagnostics remain supported.

Exact primitive graph group input/key identities can construct erased outputs
through a final continuation after both output identities and input publication
are staged and poison clears. Payload-key clones and Event creation keep their
old-then-new order. Remaining local rows are exact builtin values; custom keys or
identities use original array erasure. No callback, event, summary field or scoring
work is suppressed, and the Outputs/Event layouts remain unchanged.

The exact private consuming score backend retains hot dense pages and stores
other full numeric positions directly in a standard HashMap. Observations, copies,
clear and installation enumerate globally sorted positions; every write still
records its preimage. Full-range IDs, present zero, current/full/nested restore and
the exact builtin constructor gates remain. Sparse map capacity may remain after
peak use. Ordinary public row owners and version history are unchanged.

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
and constraint name before and after updates. Native custom justifications and
indictments are specified in the solver contract; Java reflection/API layout is
outside the native scope.

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

Set replacement, when the old value occurs more than once, prepares count changes
and any new membership insertion before publishing either map. Exclusively borrowed paths keep the
lookup topology stable until a value replacement followed by insertion commits;
unique nodes can remain unique, while shared roots still copy on write. Removing
the last occurrence and structural fallback keep isolated snapshot staging.
It preserves multiplicities, representatives, missing-old-before-new lookup and
the former default sequence of ordering and payload clone operations, including
equal replacement. Ordinary comparison/clone/count-overflow failures leave the
prior collector and finished snapshots intact. Public types and bounds are unchanged.

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

Native Eq-only changed-value discovery uses a touched-row scan for registries of
at least 16 values when current touched rows contain at most half the registry.
It resolves current positions, includes removed values and restores registry
order before delivering source updates. Dense/small and indexed paths retain
their existing behavior. Temporary storage follows the edit footprint; no Hash
or Ord bound, new model constructor or reduced metadata contract is introduced.
Internal equality comparisons and frozen-original lookup schedules may differ;
identity must remain stable as required by the model. Ordinary panic rollback
still covers the assignment and graph. This is not a promise about Eq/Clone hook
traces or a general solver speed guarantee.

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
immutable state. The selected distance arithmetic also determines connectivity.
The default IntegerSequenceMetric supports i32/i64 positions and exact i128
distances, with a const u64 step (default 1).
Custom metrics can return SignedDuration, as the adapted one-day UTC fixture does.
WrappingIntegerSequenceMetric<STEP> explicitly selects i32 positions and wrapping
i32 subtraction/addition. STEP is a positive i32; construction rejects zero and
negative steps. Negative wrapped distances form breaks under the existing
nonnegative adjacency rule. Breaks and sequence lengths can be negative after
overflow. Exact metrics remain the default. Custom STEP behavior is native; the
Community comparison uses the original default step of one.

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

The separate public comparison selects the explicit wrapping32 metric for the
same 64 original cases and MIN/MAX boundary. It checks add, replacement, removal
and temporary changes restored by inverse slot updates. Independent Python checks
every item and metadata field; Java tie order may vary but endpoint identities
must match its observed items. The full default exact i64 controls also execute
and remain separately retained: the MIN/MAX break is exactly 4,294,967,295 there,
while the selected wrapping metric yields Java's -1. The selectable metric does
not change ordinary default behavior, stable native logical-ID ties or snapshots.
Native regressions additionally exercise i64 endpoints, explicit i32 policies,
transaction undo/rebuild, failure recovery and actual construction/search against
independent full scoring. No upstream-method or performance credit is added.

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
come from a pure `RangeMetric`; the default `IntegerRangeMetric` subtracts i32/i64
endpoints exactly into i128. `WrappingIntegerRangeMetric` explicitly selects i32
endpoints and wrapping i32 subtraction for component/gap lengths. Ordered endpoints,
connectivity, overlap counts, logical identity and snapshots retain the same rules;
only lengths can become negative after overflow. Custom metrics can use SignedDuration.
Reversed ranges are rejected atomically under either metric.

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
states with both arithmetic policies. The explicit wrapping32 comparison checks
modular component/gap lengths; default exact i64 outputs and the complete original
Java difference cohort are separately retained. Java agrees on all 64 distinct-
identity positive-length cases. Empty-component MAX/MIN sentinels map to native
absence. Shared-object duplicate removal can leave a Java occurrence behind and
empty-range overlap flags can disagree with maximum counts; neither discrepancy
is hidden or treated as an agreement. All original shared-identity cases remain.
The selectable arithmetic does not reproduce Java's history-dependent flags,
live-object behavior or identity-hash order. No upstream-method or timing credit.

A separate fresh qualification covers 1,892 distinct-identity histories and
16,660 states. Native results match the independent snapshot policy throughout;
Java differs in 1,444 observations, exclusively in the overlap flag. Twelve
family/current-item combinations produce different Java flags depending on their
history. For example, adding `[0,3]` and `[1,1]` reports overlap; adding then removing
`[2,2]` leaves the same two items and overlap bounds of one, but clears that flag.
An independent flag based only on current component endpoints cannot reproduce
both results. The current native policy remains unchanged. The inspected
connected-range guide section states neither an empty-interval exclusion nor
this history rule.
The [recorded witnesses](../compat/observations/connected-range-history.json)
preserve both runtimes' actual outputs and their artifact identities. This
qualification adds no upstream-method credit and does not refresh the older
197-case report or the complete stream comparison job.

On 2026-10-20 the user explicitly chose “Keep consistent native behavior” for
completion. Snapshot-consistent overlap flags are therefore the native target;
the observed Java history-dependent flags remain documented incompatibilities.
This decision adds no Java agreement passes and does not alter the retained
case counts, original-method inventory or historical execution evidence.

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

### Explicit wrapping load-balance moments

`WrappingLoadBalanceCollector<K>` is an additional arithmetic policy; the ordinary
`LoadBalanceCollector` retains its exact BigInt totals and moments. Captured
triples, multiplicities, consistent initial loads, atomic replacement and immutable
snapshots retain the same membership contracts. The new snapshot exposes signed
i64 per-key loads. Totals L, their sum S and squared moment Q use two's-complement
wrapping i64 arithmetic. Default unfairness is the six-significant-digit, half-even rounded
square root of `(n*Q - S.wrapping_mul(S))/n`, widening the numerator to BigInt
before multiplying/subtracting. Empty input and a zero numerator return zero.

Both `try_finish()` and the reversible collector's `finish()` return a Result.
With the default root policy, a negative widened numerator yields
`WrappingLoadBalanceError::NegativeRadicand`;
captures remain intact and subsequent removals can recover. A graph consumer can
score this result, reject it, or explicitly panic to trigger transaction rollback.
This is a defined native arithmetic error, not a claim about Java NaN behavior.
Occurrence counts remain checked usize. Keys require stable lawful ordering and
identity-preserving clones; panicking destructors and allocation aborts are unsupported.

`WrappingLoadBalanceRootPolicy::Exact` remains the default widened rational
root policy. `with_root_policy(WrappingLoadBalanceRootPolicy::Binary64)` explicitly
changes observation arithmetic without changing captures, wrapped moments or
previously returned snapshots; clones retain the selection. The Binary64 policy
converts Q, S.wrapping_mul(S), and unsigned key count separately to binary64,
divides the latter two, subtracts from Q, and takes a binary64 square root.
The exact numeric value of that finite binary root is then rounded to six
significant decimal digits with half-even ties. Counts above 2^53 lose precision
as part of the selected conversion policy. Empty input and either zero sign
return canonical zero. Negative radicands return the existing typed error.
Bounded i64 moments and supported usize widths imply finite public radicands.

The complete comparison executes all three native modes on identical original
inputs. Exact-default and wrapping-exact-root controls retain complete output,
results and differences separately. All 101 cases/8,290 states agree with actual
Java under the selected Binary64 policy and its independent integer/Fraction
model. This establishes the measured cohort, not general Java formula,
conversion-order, decimal-scale, nonfinite, live-map or history equivalence.
No additional upstream-method or performance credit follows from this policy.

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

## Finishing collector results

CollectorFinisher<Input> defines a pure, deterministic transformation of an owned
result into another Clone result. FinishingCollector<C,F> wraps an inner collector
and applies that transformation only at finish, including empty standalone results.
An empty stream group still emits no row. The policy is a type, with no Clone or
Default requirement on F; its function must not mutate shared state or depend on
call history. For transformations needing runtime closure captures, use a graph map.

Combine arbitrary captured input fields using nested PairCollector, then flatten or
derive a result with FinishingCollector. It preserves each delegate's own replace
method, including conditional transitions and specialized checked arithmetic. It
stages inner insert/remove/replace calls before installation; false removals or
replacements and ordinary panics leave the wrapper's prior inner state intact.
A finisher panic cannot mutate captured state under the pure-finisher contract.
Graph transaction failures still invalidate the transaction and restore its checkpoint
on drop. from_collector accepts an existing collector; into_inner returns it. Clone
shares inner roots when supported by that collector. No general O(1) custom-collector
clone, callback count/order, live aliasing or Java ABI guarantee is made.

Public comparisons cover two-/three-/four-result composition and a transformed
count. All 132 cases and 11,300 states match independently rebuilt minima, maxima,
counts and exact average bits, including duplicate inputs and inverse updates.
Inputs keep integer sums/differences representable. Dedicated composition timings
and generic Java callback-failure/recovery behavior are outside this increment.

## Checked integer sum and collection updates

SumCollector returns a checked signed-i64 sum, with zero for an empty standalone
collector. It is separate from IntCollector/IntSummary, whose sum remains wrapping.
Insert checks total+value; removal checks total-value. Replacement checks new-old
first, then total+difference. Both intermediates must be representable. An equal
replacement has zero difference; replacing MIN with MAX rejects even for a single
occurrence because the difference overflows. SumOverflow from try_insert/remove/
replace preserves state; trait methods panic for normal graph rollback. Missing
remove/replace returns false. Captured zero/duplicate values retain multiplicity,
clone shares persistent roots, and finish reads the maintained total. Count overflow
uses the sum collector's checked panic. Keep all insertion prefixes
representable when requiring independent reconstruction to succeed.

The implementation keeps private primitive frequencies, checked count and total;
finish reads only the total, without constructing extrema summaries. One bounded
pending replacement describes the logical frequencies over a persistent base.
After each operation's arithmetic and membership preflight, replacing that pending
new value composes the two frequency deltas; returning to the original value
cancels them. Other successful mutations materialize the pending primitive edit.
Missing, equal and overflowing operations leave the representation unchanged.
In particular, `0 -> MIN` succeeds but `MIN -> 0` still fails its difference
check before cancellation. Valid individually checked chains may have an
unrepresentable net difference; materialization does not recheck that difference.

Membership includes the pending delta and every duplicate/zero capture. Equality
compares logical frequencies across pending/materialized histories, cloning and
materializing temporary roots only when the pending transforms differ. External
clones share persistent roots and copy the bounded pending field; later mutations
preserve old snapshots. The collector grows by that field; unrelated edits can
pay deferred tree work and equality can allocate temporary paths. No general
latency or memory improvement is claimed. The no-pending removal path keeps its
ordinary direct lookup/removal. Public bounds and checked arithmetic are unchanged.
Private derived Debug field shape changes; no stable text format is promised.

Primitive integer frequencies use a private persistent AVL tree whose i64 key
and usize count live directly inside each shared node. No separately shared key
or count allocations are needed; cloning still shares the root and updates copy
only shared search/rebalance paths. This specialization accepts no user callbacks
and does not change generic SnapshotMap storage or public collector bounds.
A unique old class and absent new class reuse the removed node allocation.
Structural removal/insertion rebalance normally and preserve duplicate successor
counts. Collector snapshots isolate edits, including transaction rollback;
private frequency trees do not use generic-map journals. Duplicate classes,
existing destinations and inline promotion/demotion keep the ordinary count
operations. Promotion copies counts directly, including large multiplicities;
it does not replay occurrences. Checked replacement retains its original
difference/addition preflight.
Wrapping IntCollector replacement validates membership and count arithmetic before
changing frequencies, then applies wrapping subtraction/addition without a staging
clone. Missing captures, equal values, immutable snapshots and rollback retain
ordinary behavior. No generic user callback is introduced between the primitive
structural edits; public bounds and generic map operations are unchanged.

Public black-box probes, not implementation inspection, establish these arithmetic
rules. The bounded ordinary collector source methods now use SumCollector for sum
updates. Existing wrapping summaries remain useful for explicitly modular arithmetic.
MapCollector with FinishingCollector can expose each key's merged integer result;
using the IntSummary sum cast to i32 implements Integer::sum modulo 2^32 for captured
i32 values. Zero-valued keys persist until their final occurrence is removed.
This is an additive merge policy, not a general arbitrary binary merger API.

Set/map/sum comparison retains 270 original cases and 19,484 observations with
independent full multiset reconstruction. All states or expected rejection markers
agree with public Java collectors, including 39 expected overflow rejections. Both
runtimes stop at their first error; those rejections are not successful updates and
no Java post-error state is claimed. Set comparisons normalize iteration order; native sorted-set iteration is checked
separately by the adapted tests, and map comparisons retain sorted key order. Null-key/value semantics,
general merge functions and dedicated timings are not covered by this comparison.

## Aggregate update/observation benchmark

`python3 scripts/benchmark_aggregates.py` compares SumCollector, SetCollector and
MapCollector with an i32-sum finishing policy against public Community collectors.
Workloads contain 64/512 captures. Sorted sets have 32/256 distinct paired values;
merged maps have 8/64 initial keys with eight captures per key. Each selected capture
is replaced with a value/new key and restored by an inverse update. Inputs keep
all arithmetic intermediates representable. Independent Python reconstructs each
full result; both runtimes assert every timed trial/restore and separately committed
state, and consume batch checksums. Native snapshots and Java live results are
materialized into integer arrays. Native captured-value and Java handle APIs retain
their own bookkeeping costs.

Timing includes mutation, inverse mutation, finish, full observation allocation and
copying, assertions and checksums. Initialization, parsing, process startup and
output encoding are outside timing. Warmup batches precede measured samples in
fresh processes with alternating runtime order; all raw records remain in
work/aggregate-performance.json. Ratios compare median batch-average pair times.
No latency percentiles, statistical significance, isolated CPU, JMH, memory, generic
merger, graph rollback or overall solver-speed claim is made. All 96 original
validation cases and every timed observation must pass or the run aborts.

## Value-only snapshot replacement

An instruction profile of the original incremental-stream search workload at
commit 6934f43 identified persistent-map update work alongside allocation costs.
Existing-key updates now follow the prevalidated search path iteratively, copying
only shared nodes and replacing the value. Tree shape, height and subtree size
cannot change in this operation, so it requires no balance checks or metadata
recalculation on the return path. Insertions and removals retain AVL maintenance.
Original equal-key representatives, payload sharing, Send/Sync and snapshot
isolation remain unchanged. No unsafe code or dependency was added.

The exact-root panic regression now injects comparison failures during existing-key
replacement in both shared and uniquely owned maps, as well as insertion and
removal. Existing tests check 20,000 edits against BTreeMap, every AVL invariant,
retained snapshots, non-Clone payload sharing, bounded ranges and node reuse.
This optimization adds no upstream method coverage.

Callgrind 3.22.0 compared ordinary release binaries on four original stream-search
workloads, each with 50 steps, no warmup and one solve per process. Whole-process
instruction references were:

| Workload | Before | After | Reduction |
| --- | ---: | ---: | ---: |
| Hill climbing, 32 variables | 343,027,178 | 320,222,926 | 6.65% |
| Hill climbing, 96 variables | 1,274,376,313 | 1,186,628,115 | 6.89% |
| Tabu, 32 variables | 345,614,190 | 322,710,362 | 6.63% |
| Tabu, 96 variables | 1,279,191,407 | 1,191,443,209 | 6.86% |

Every output was independently rescored, and final assignments, scores and terminal
callback counts were identical before/after. These instruction counts are separate
from the Java/Rust dashboard timings; they do not establish elapsed-time speedup,
allocation reduction or general solver performance. Raw inputs, binaries, profiles,
checksums and runner are in work/profiling/value-replacement/. The dashboard uses
fresh uninstrumented comparisons, with shared-host timing limitations unchanged.

## Incremental unique pairs

`UniquePairNode` and graph `unique_pairs`/`unique_pairs_equal` compare distinct
occurrences once, oriented by stable native ID (`smaller, larger`). Unlike the
eager snapshot API's position order, graph orientation survives insertion order,
updates and trial rollback. Equal payloads at distinct IDs still pair; no self or
reversed pair is generated. Input filtering, including exclusion of unassigned
facts, remains explicit. Tuple keys express several equality requirements; a
following filter can express asymmetric predicates on the ID-oriented payloads.

Each source change reaches the graph node once. Persistent rows and equality
buckets require linear retained index state; no pair adjacency table is stored.
An update projects its new key once and visits only old/new bucket peers. A
same-key update emits one put for every surviving pair, refreshing payloads and
subsequent predicates. A key change retracts old pairs before putting new ones;
retraction uses the captured key, never rerunning the projection. Within each
set, peer ID determines output order. Dense output and downstream match storage
can still be quadratic. Inspection enumerates pairs and is not a cached view.

Standalone operations prepare outputs and candidate roots before installation,
so ordinary projection, comparison and clone panics preserve prior state. The
normal graph journal covers downstream callback failure, rollback, accepted
moves, shared branches and diagnostic rebuilds. IDs/keys must be immutable with
consistent ordering; callbacks must be deterministic without external effects.
No timing or Java internal event-order equivalence is claimed.

## Flat multiple-collector groups

Native tuples of two, three or four `ReversibleCollector` implementations collect
corresponding flat input fields and return flat output tuples. Use these directly
with `group_collect_with`, `collect`, conditional collectors or finishing adapters;
compound tuple group keys are independent of collector count. Nested tuples remain
valid for larger shapes. Each mutation clones the tuple, invokes each component
in field order and installs all components together. Missing removal/replacement
or ordinary callback/clone/arithmetic panic leaves the prior tuple intact, assuming
delegates obey the collector contract. Specialized `replace` is preserved, including
checked intermediate sum arithmetic. Composition adds no input Clone requirement.
Pure finish runs each delegate; graph transactions cover downstream failures.

`DistinctCountCollector<V>` retains persistent value multiplicities and returns
native usize distinct count in constant time. Empty finish is zero; final duplicate
removal releases its key. Clone shares roots with isolated later mutation. Stable
Ord defines distinctness; explicit Option values can represent a counted null.
As with all captured collectors, direct removals must identify an actual captured
occurrence. Empty graph groups disappear; standalone empty results do not invent
an output row. No new timing measurement accompanies this addition.

## Reusable transform definitions

`SharedFilter`, `SharedMap` and `SharedFlatMap` define reusable callback identity.
Applying a definition or its clone to the same parent in the same builder returns
the same graph node. Fresh definitions, different parents and different builders
remain separate, even for callbacks with equal bodies/results. Ordinary builder
filter/map/flat_map calls retain their existing behavior. This is explicit reusable
identity, not automatic Rust closure equivalence or Java lambda discovery.

Definitions invoke no user callbacks during construction/application. At runtime,
a shared node processes each incoming change once and fans its output to every
consumer. Each consumer still contributes its own score. Existing journals handle
shared-node rollback, callback failures, diagnostic rebuilds and duplicate expanded
occurrences. An expansion collects a finite IntoIterator into owned positional
outputs; prefix retention is expressed in the mapping. Retractions do not rerun
mappings. Functions must be deterministic with no external side effects.

Clones share callback and registration storage. Graph nodes retain the callback,
so definitions may be dropped after construction. Registrations hold weak graph
identity and node numbers, not graph state; dead identities are pruned on the next
application. Lookup scans the definition's active registrations. A foreign input
port is rejected before registration lookup/creation. No global registry or
cross-thread graph sharing is introduced. Callback-count tests demonstrate work
sharing; no new throughput comparison is claimed.

## Reusable relation definitions

`SharedJoin` and `SharedExistence` share one relation for each definition identity,
ordered parent pair and builder. Clones retain identity; fresh definitions and
changes to either parent stay distinct. A join can use any JoinComparison and an
optional post-join predicate. An existence definition has a fixed Exists/NotExists
mode; without a predicate it uses the equality-count index, and with a predicate
it uses the indexed predicate relation. Constant keys express a Cartesian join or
unrestricted right-side presence; stable-ID inequality expresses self-exclusion.
Filtering unassigned facts remains explicit.

`with_filter` consumes a definition and returns a fresh registration identity with
the new predicate. It leaves prior clones and already-built nodes unchanged; it
replaces any prior predicate rather than composing predicates. Apply validates both
parents before looking up the registration, and runs no callbacks. Shared relation
outputs fan out to independent scoring constraints without repeating projections
per consumer. Self joins still receive one update per side of their relation.

Callbacks must be deterministic and inspect immutable inputs. Definitions retain
weak builder identities and callbacks; built graph nodes keep callbacks alive if
definitions are dropped. Dead registration owners are pruned on the next apply;
lookup scans active bindings. Graph checkpoints restore shared indexes, filtered
outputs and downstream consumers together after trials or ordinary callback failure.
Runtime comparisons against independent nested enumeration cover all ordered join
comparisons, filtered/unfiltered existence and absence, and changing facts/removals.
No throughput ratio or Java automatic-node-interning guarantee is claimed.

## Reusable aggregation definitions

`SharedGroupKeys` emits unique mapped keys; `SharedGroup` combines keys with any
reversible collector, including flat tuple collectors. `SharedCollect` aggregates
a whole stream. `SharedGroupList` and `SharedCollectList` retain duplicate occurrences
in ascending input-ID order and return immutable persistent list snapshots.
All forms emit no rows for empty input; keyed forms remove a group on its final
retraction. Tuple keys support multiple grouping fields.

Applying one definition or its clone to the same parent in one builder reuses the
complete pipeline, including intermediate mapping and aggregation nodes. Fresh
definitions and changed parents remain independent even with equivalent callbacks.
Mappings run once per put, use captured inputs on retraction, and must be pure and
deterministic. Construction runs no callback. Multiple scoring consumers do not
repeat collection work; graph checkpoints restore aggregates and consumers together.
Definitions use the existing weak parent-registration storage, prune dead owners
on application, and retain no graph state. Built nodes own their mapping callbacks.
`StreamPort::same_node` compares graph/node identity across payload types without
comparing rows or inferring equivalence. Ordinary builder calls are unchanged.
No throughput comparison or Java automatic-interning guarantee is claimed.

## Shared distinct and concatenation pipelines

`SharedDistinct` reuses one distinct occurrence index for a definition/clone and
parent. Ord determines value classes, with the lowest input ID supplying each
representative payload. `SharedConcat` reuses one routing node for a definition
and ordered parent pair. It preserves duplicate occurrences with branch-tagged IDs,
including both branches of self-concatenation, without intermediate mapping nodes.
Fresh definitions, different parents and different builders remain independent.

`SharedPaddedConcat` converts each side to one common payload type using two pure,
deterministic mappings. Option fields express null padding. Each side's mapping
is itself shared, so pairing one parent with several opposite parents does not
repeat its conversion work. Both parents validate before topology changes, mapping
runs only on puts, and retraction uses captured outputs. Runtime graph checkpoints
restore all shared branches and score consumers on rollback or callback failure.
Definitions retain only weak builder registrations and mapping callbacks; dead
registrations prune on apply and built nodes retain callbacks independently.

Existing SharedMap can append fields or map multi-fact tuples; SharedFlatMap can
flatten whole tuples or retain a prefix while flattening the final field. Positional
IDs preserve duplicates. Native arity is expressed by Rust payload types rather
than Java overloads. No throughput or automatic lambda-interning claim is added.

## Assignment scope in shared existence relations

Assignment scope belongs to the input port: an explicit assigned filter supplies
ordinary left/right streams, while an all-fact source supplies an unassigned-inclusive
right stream. Applying the same SharedExistence definition to these different right
parents produces separate relation nodes. Including unassigned right facts does not
change left-stream assignment filtering. Self-exclusion compares stable fact IDs
independently of equality keys; equal-code facts with different IDs can match.

When one source feeds both sides or several positions in a Cartesian left tuple,
assignment changes propagate through every role. Original full-enumeration checks
compare all output fields and scores for four arities, assigned/all right scopes,
Exists/NotExists and optional self-exclusion through committed and rolled-back
changes. This is integration evidence for existing operators, not a new implicit
entity-discovery or callback-interning mechanism.

## Precomputed fact pipelines

`PrecomputeNode` and `StreamGraphBuilder::precompute` cache one computed output per
input ID and explicit fact key. The key selector runs on every put. A new ID or
changed key computes and emits a new output; an equal key retains the previous
output object and emits nothing downstream. Removal discards its cached entry and
emits a retraction without callbacks; reinsertion computes again. Equal keys at
different IDs never merge occurrences. Keys must include every dependency of the
computation. Planning variables omitted from the key must not influence that output.
Keys, outputs and captured fact dependencies must remain immutable.

Build fact-only joins, filtering and aggregation after this boundary. To use current
planning values downstream, retain stable IDs in cached output and join back to a
live source. This separates static membership from current assignments without
freezing planning values or using interior-mutability aliases. Fact-key selection
is explicit; no reflection, dependency discovery or global property notifications
are supplied. Related fact changes must update the relevant key or their own source.

`SharedPrecompute` reuses one boundary/cache per definition and parent within each
builder. Clones share identity; new definitions/parents/builders remain independent.
Registrations are weak and pruned on application. The built graph owns callback
references independently of the definition. Cache state uses persistent maps and
participates in graph rollback, saved candidate restoration and diagnostic rebuilds.
Standalone ordinary callback/key-equality/clone failures leave its state unchanged;
normal graph transaction failure and destructor/interior-mutation limits still apply.
Diagnostic rebuilds deliberately rerun precomputation. No throughput comparison is
claimed. Original move-scoring integration verifies current assignments and unchanged
fact-computation counts during native basic-variable trials and rollback.

## Source-emptiness activity pruning

`StreamGraphBuilder::with_activity_pruning()` opts into dynamic branch scheduling.
Sources always retain facts. Ordinary operators require every input to be potentially
populated; concat needs either input, while anti-joins and optional references need
only their left input. A backward pass additionally requires a path to an enabled
scoring constraint or captured dataset. Shared roots stay active if any consumer
needs them. Disabled zero-weight branches remain pruned. There is no callback-based
emptiness inference: predicates, key compatibility and actual output cardinality
may make a structurally active branch produce no rows.

Updates skip inactive operator nodes. A source transition between empty/nonempty
recomputes the plan; if it changes, all operator caches are cleared and active
branches are rebuilt from retained sources in node/ID order. This deliberately
uses a whole-graph rebuild rather than partial cache recovery. Precomputed outputs
may therefore be allocated again even if their fact keys did not change; allocation
identity is not retained across activity-plan transitions. Other updates remain
incremental. Rebuild callbacks count as ordinary scoring work and use the configured
arithmetic policy, including checked intermediate overflow failures. Callbacks must
be pure: inactive branches do not evaluate or validate their payloads. Frequent
source-empty transitions may cost more than they save, so pruning is disabled by
default and no general throughput improvement is claimed.

`graph.activity()` returns an immutable `StreamActivity` snapshot without callbacks,
even when pruning is disabled. Typed `get(port)` rejects foreign graph handles and
returns potential `can_produce` and observer-dependent `active` flags. Snapshots
keep their captured state; they are not live views or actual tuple-count estimates.
With pruning disabled these flags do not imply skipped execution. Native filters
and terminals remain graph nodes; Java node-kind/compilation-layout parity is not
part of this API.

Sources, cached indexes, score and the routing plan all participate in transaction
rollback and saved-candidate installation. A failed or panicking activation poisons
the transaction; dropping it restores the preceding plan and caches without running
callbacks. Diagnostic rebuilds honor the captured plan and restore all original
state on return. Captured datasets count as observers even without scoring sinks.

## First-occurrence expansion representatives

FlatMapNode::new_canonical and StreamGraphBuilder::flat_map_canonical accept a
mapper and explicit equivalence-key projection. Within each input expansion,
every output with the same key is replaced by a clone of the first output for
that key. Occurrence count, iterator order and positional IDs are preserved;
Rc/Arc outputs share that representative's allocation. Keys must include every
payload distinction that the caller intends to retain, including varying tuple
prefixes. This is an opt-in operation; ordinary flat_map remains unchanged.

Representatives are local to one input expansion and are chosen afresh on each
replacement. Reordering can change the selected object. There is no global cache
or identity guarantee across updates. Retraction uses stored lengths without
running mapper/key callbacks. Mapping, key extraction, cloning and iterator work
finish before state installation; failure leaves node/graph state unchanged under
the existing transaction contract. Key callbacks and mapping must be pure. Work
is O(n log d) for n expanded occurrences and d keys. No throughput claim is made.

## Shared equality partitions

`PartitionedJoinNode` now stores left and right ordered indexes in one shared
partition per equality key. A side remains uninitialized until its first row;
removing the last row from both sides removes the partition. Captured-key membership
maps still support callback-free retraction. Candidate persistent roots and emitted
events are completed before installing a change, including right-side event order
normalization; graph checkpoints retain the whole shared state for rollback.

`partition_count` and `partition_state` expose stored-key counts and lazy-side
state without creating anything. `matching_right(equality, left_key)` and
`matching_left(equality, right_key)` return borrowed rows for prospective inputs,
using the configured relation and its reverse respectively. Neither query requires
an input row on the querying side. Results follow ordered key and occurrence ID;
absent partitions/sides yield empty iterators. Multiple equalities use tuple keys,
and pure equality can use a unit ordered key. No runtime speed claim is made.

## Batched source changes and solver scoring

StreamBatch collects heterogeneous source changes for one graph. Foreign-source
pushes fail without adding that change. GraphTransaction.apply_batch retains only
the last change per source ID, updates source caches, and visits affected derived
nodes in topological order. It preserves the transaction failure/drop contract.
Activity pruning observes final source emptiness and rebuilds operator caches at
most once per batch when the activity plan changes. Source/ID order determines
routing; this API does not preserve intermediate input values or callback order.

For zero/one-source-change batches, pending work uses four sorted inline rank
slots, promoting once to the ordered tree when a fifth distinct rank is pending.
Events retain within-rank FIFO and shared payload identity. Discards drop requested
ranks immediately in caller order; normal/unwind drops preserve ascending remaining
rank order. Buffers retain the previous Vec growth policy. Multi-change batches
continue using retained heap routing; source coalescing, activity and rollback
semantics are unchanged. This is a private scheduling optimization.

StreamGraphBuilder.join_batched adds an equality-indexed filtered join whose two
inputs settle together. Each affected final candidate pair is evaluated once and
surviving pair payloads are updated once. Source callbacks and batched predicates
must be pure and work from immutable inputs. Other operators retain their ordinary
per-event behavior and checked arithmetic, including possible intermediate errors.
Accepted pair IDs are stored, so dense joins can still use quadratic memory;
input payloads live in side indexes and accepted output payloads live downstream.

Ordinary filters and maps collect their batch outputs directly into one vector.
Each input event still runs in encounter order, including repeated IDs and
retractions; predicate, mapping, ID/payload cloning and panic behavior retain
ordinary per-event semantics. This avoids a temporary output vector per event
without coalescing callbacks or enabling automatic batching for ordinary graphs.
Three original regressions compare callback/clone/drop traces with singleton
delivery and exercise rollback and retry after later-event callback failures.

The retained239 comparison observed 5.74% lower large hill/swap time across four
balanced pinned forks, with every fork improving. Two repeatable whole-process
allocation captures observed 8.30% fewer allocation requests for large swaps and
4.93% fewer for large changes. These count malloc/calloc/realloc requests, not
bytes or live/peak memory. A mixed workload with no changed unary operators had
identical allocation counts but was 3.50% slower initially and 4.58% slower in a
longer same-artifact confirmation; this regression is retained as an explicit
tradeoff, not dismissed as noise. There is no general speedup or Java-parity claim.
Raw artifacts, work-signature checks and comparisons are under
`work/round239-unary-batch/`.

Basic, list and mixed stream score directors automatically submit complete initial
and move updates as batches when a batched join reaches a scoring constraint or
captured dataset. Graphs without an observed batched join retain ordered immediate
source delivery. Explicit transaction.apply remains immediate for every graph.
Checkpoint capture, saved-winner installation, multi-move scopes and diagnostic
rebuilds include the new join state. Diagnostic rebuilding may evaluate callbacks
again; commit and saved-winner installation do not replay them. No runtime benchmark
speedup is claimed from the reduced callback-count regression cases.

### Batched existence constraints

`StreamGraphBuilder::existence_batched` accepts the same equality keys, residual
predicate and `ExistenceMode` as `existence`. Within `apply_batch`, both input
branches settle before the node publishes at most one final lifecycle change per
affected left identity. Several matching witnesses still produce one left row;
replacing a witness during a round cannot briefly expose a `NotExists` result.
An observed batched existence node enables automatic move batching in basic,
list and mixed stream score directors. Unobserved nodes do not change that mode.
Direct transaction `apply` remains immediate.

The standalone existence engine still evaluates its queued inputs in order;
this API coalesces downstream changes, not every predicate call. Callbacks must
be pure. Its accepted-pair index can retain quadratic state. Checkpoints cover
committed index roots; rollback discards pending changes as well as restoring
rows. With activity pruning, `NotExists` can remain active when its right source
is empty. No Java scheduling-distance or initialization-notification parity is
claimed by this API.

### Coalescing derived updates

`StreamGraphBuilder::coalesce_updates` publishes at most one final update per
identity during a graph batch. Place it after a collector or another operator
that emits several updates for one identity during a move. Scoring and other
downstream operators then see only the final payload. A transient occurrence
that was absent before the batch and is absent afterward emits nothing.
Replacements of visible occurrences still emit an update without requiring
payload equality. Output order is by identity. The node retains identities,
not payload copies; snapshots and transaction rollback include that state.

An observed coalescing boundary enables automatic batching in all three stream
score directors. Direct transaction `apply` stays immediate. Work and arithmetic
upstream of the boundary keep their existing semantics. This is an explicit
boundary: it does not automatically coalesce every operator in the graph.
Consistency checks for graphs configured to batch now rebuild from their source
rows in one batch too, preserving the same downstream callback contract.


## Reversible row updates in indexed batched joins

Graph transactions distinguish a rollback checkpoint from a saved candidate
snapshot. Indexed batched joins start private journals for input-row maps and save
their index roots and health flags. Existing-row replacements record the old shared
value and a compact prepared tree path. They do not share the row root merely to
establish rollback, so uniquely owned paths can be updated in place. Undo restores
values in reverse order without key comparisons, projections or payload cloning.
Retained public/saved-candidate snapshots still force copy-on-write when needed.

A structural map change first materializes each enclosing journal's original root.
This covers insertion/removal, clearing during activity or consistency rebuilding,
and installation of a prepared replacement map. Nested commits merge value records;
nested rollback restores its own entry state. Empty log buffers are pooled across
trials and nesting levels. Snapshot clones copy only roots, never active journals.

After coalescing a join batch and projecting final keys once, an all-replacement
batch whose keys are unchanged uses the existing membership indexes. It prepares
final pair payloads before installation, then updates row values under nested undo
guards. Standalone comparison failures during installation restore both sides.
Other batches retain atomic preparation on persistent snapshots. Pair ordering,
first-ID representatives, final payloads, terminal scoring counts and poisoned
transaction rules remain unchanged. No additional upstream-method credit is claimed.

Source rows, filter inputs (including rejected rows), terminal match records and
captured datasets use the same private replacement journals. Terminal checkpoints
save the previous total score before opening a row journal; a score Clone panic
therefore cannot leave an unregistered rollback frame. Score changes, structural
retractions and acceptance changes keep their prior observable behavior.

Diagnostic comparisons materialize a detached entry-state view from a journal's
saved root or reverse value log. This does not consume or alter active frames and
supports nested consistency rebuilding, including cancelling match-score changes
and dataset changes that leave the total score unchanged. Saved-candidate installs
preserve reusable journal buffers; caller-held snapshots retain their independent
contents. Ordinary graph nodes without journals keep their existing checkpoints.

Journal selection is adaptive: a root already shared with a retained snapshot uses
root rollback, avoiding an extra value log on top of mandatory copy-on-write.
Opening such a nested checkpoint first preserves enclosing journal entry states.
Unshared roots use compact value logs. Insertion reuses the old value found during
its comparison pass to record undo, without a second traversal of the tree.


## Atomic integer grouping rounds

`GroupNode::apply_batch` retains the last change per input ID, with the first
representative of repeated IDs. It prepares each affected integer collector once,
then publishes one final summary or retraction per group in key order. Existing
group-key representatives survive temporary emptiness within a round; a new group
uses its first final key in sorted input-ID order. Absent-to-absent groups emit
nothing. Counts, distinct values, wrapping sums, extrema and duplicate occurrence
removal retain IntCollector semantics. Atomic rounds replace existing input rows
without removing/reinserting their tree nodes. Nested map guards protect both
input and summary installation against comparison panics. Output keys/summaries
are prepared before installation; stable keys and non-panicking destructors remain
required. Saved snapshots and graph trial journals preserve normal rollback.

`StreamGraphBuilder::group_collect_batched` is an explicit integer-aggregation
boundary. It maps only final surviving puts per input ID and enables batched
score-director updates when observed. Downstream filtering/scoring/datasets receive
final group state once per round. Direct transaction `apply` and ordinary
`group_collect` retain immediate behavior. Generic user-defined collectors retain
their existing per-event semantics. The batched benchmark uses this boundary in
place of ordinary grouping followed by a separate coalescing node.

After initial planning binding, this batched group boundary may store its input
occurrences in private slots for a checked universe of at most 256 `usize` IDs.
Filtered inputs may be sparse and become present later. Bounds pass only through
explicit identity-preserving map/filter operators; group output keys start a new
namespace, even when their type is also `usize`. Arbitrary mutable fact IDs and
larger/unbound inputs retain the original maps. Group keys, collectors and the
mapping/preparation/publication order keep their existing semantics.

Trial journals move old payload owners into ordered undo records; nested commits
merge those records. Journal storage grows with writes until the enclosing trial
ends. Full checkpoints copy at most 256 shared pointers and preserve outer undo
when restored. Neither snapshots nor rollback clone user payloads or compare
group keys. This trades bounded checkpoint work for cheaper occurrence lookup
and restoration. Standalone grouping retains generic map storage.

A local four-fork comparison of actual release executables observed 8.05% lower
large hill/change and 12.78% lower hill/swap medians, with matching assignments,
scores and callback counts in every sample. Both improved in all four forks;
the Dense control was essentially flat, while Mixed Stream varied. Whole-process
allocation requests fell only 0.14–0.51%, not a large memory reduction. These
shared-host readings do not establish a general speedup or Java parity. Evidence:
`work/round235-bounded-group/{paired/results,allocation-counts}.json`.


## Compact filtered indexed joins

Graph `join_batched` uses the compact indexed engine with equality keys.
`join_compare_filtered_batched` additionally supports all five ordered-key
relations. A shared internal final-pair projection hook evaluates the residual
predicate once for each affected final key match, before cloning its payload.
Rejected pairs allocate no output payloads and are not retained. The graph node
stores accepted pair IDs only, suppresses rejected-to-rejected/absent-to-absent
retractions, and emits surviving accepted replacements in pair-ID order. An
unchanged accepted ID needs no ID-map replacement. Input rows, indexes and
accepted IDs all participate in graph journals and saved-candidate snapshots.

This preserves graph callback, direct-apply, batch, rollback, activity and
consistency contracts. Projections and predicates remain pure and stable; graph
transactions restore both side state and accepted membership after failures,
including comparisons that fail after pair preparation. Standalone BatchedJoinNode
retains its eager/deferred queues, committed pair dataset, retry/discard APIs and
existing behavior. The batched stream benchmark combines its second indexed join
and residual conflict filter through graph join_batched; other graph users obtain
the compact storage through the same existing API.

### Filtered joins without retained pair membership

`join_compare_recomputed_batched` is an opt-in equality/range join retaining input
rows and indexes only. It emits the same ordered final replacements/retractions
as the retained-ID filtered join, but determines whether a vanished/rejected pair
needs retraction from its previous input rows. Final matching pairs run the
predicate once; a pair with no final accepted output may also run it on its old
matching rows. Predicates must be pure functions of immutable row contents.
Existing `join_compare_filtered_batched` and `join_batched` retain their original
once-per-final-matching-pair callback contract.

This removes the potentially quadratic accepted-ID table from the join. Candidate
batches, outputs, downstream caches and retained snapshots can still consume
substantial memory. Only accepted final payloads are cloned. Both old/new predicate
work completes before join-row installation, so predicate and comparison failures
retain normal graph rollback. Saved states, nested snapshots, consistency checks,
activity pruning and self-joins use the existing transaction lifecycle.

The native batched stream comparison adapter now uses this operator for its cheap
conflict predicate. A paired development measurement showed lower swap CPU time
with identical assignments/scores/scoring-call counts, while change-move timings
were mixed. This is a workload-specific tradeoff, not a universal speedup or proof
of Java performance parity; expensive predicates can favor retained membership.

`join_compare_map_batched` fuses a recomputed filtered indexed join with its final
payload projection. The mapper borrows both final input rows, runs once per
accepted affected pair, and produces the downstream value directly. Rejected pairs
and retractions never invoke the mapper. Pair IDs, comparison relations, sorted
final updates, batching, graph rollback and saved candidates retain the indexed
join contract. Input callbacks must be pure and depend only on row contents;
prior acceptance may be recomputed, but prior output values are never projected.
Input rows/indexes are retained without a pair table. Candidate/output batches and
downstream state may still be quadratic. Output values have no Clone bound at the
join itself; downstream operators may impose one. This avoids intermediate pair
payload clones and a separately routed map node. It is not a universal speed claim.
The unchanged-key batch path also reuses already-read prior payload references
when checking retractions, without repeating a known-unchanged key comparison.
Original checks compare retained, recomputed and projected joins with pair scans
for all five comparisons, with/without pruning, and test mapper counts/failures,
self joins and rollback. No upstream method credits are added.

`PartitionedJoinNode::apply_batch` and `join_partitioned_batched` now accept both
sides as one final-state round. They coalesce repeated input IDs before projecting
keys, prepare shared partition roots off to the side, visit old/new matching ranges
only within touched equality partitions, and emit one sorted final change per
affected pair. Surviving pairs refresh; intermediate matches are never materialized.
Rows may move between equality partitions and ordered keys in the same round.
Immediate and batched standalone updates can be interleaved. Empty partitions are
removed, and a side is initialized only when it receives a row.

Ordinary iterator, projection, comparison and output-clone panics leave standalone
state unchanged; graph checkpoints also restore downstream state. Destructors and
external callback effects retain the existing exclusions. Graph observation enables
batched planning updates, including self joins and saved candidate replay. Candidate
and output vectors can still be quadratic for dense joins. Original scan-based,
clone-count, panic, pruning, rollback and solver regressions cover this behavior;
no new upstream method credits or wall-clock performance claims are made.


`ExistsCompareNode` and the graph builders `existence_compare_batched` and
`existence_partitioned_batched` specialize semi/anti joins for all five ordered
comparisons. Equality prefixes can be compound tuples. Inputs are coalesced before
key projection and each batch emits sorted final visibility changes, refreshing
visible changed left payloads. Self joins include matching self occurrences.

Persistent storage is linear in input/index entries, with no pair links or right
payloads. Equalities use per-key right counts; inequalities use each partition's
minimum/maximum witness key. Changes visit only left keys whose visibility can
cross the old/new boundary. Duplicate witnesses or unchanged extrema emit nothing.
These operators have no residual predicate or self-exclusion option; generic
existence operators retain those capabilities. Standalone right payloads need not
implement Clone. Graph transactions support pruning, rollback, and saved candidates.

Original scan oracles cover both modes, all comparisons, empty/nonempty transitions,
partition moves and duplicate inputs. A 10,000-left-key boundary shift visits fewer
than 300 key comparisons and emits one row; 4,000-by-4,000 witness tests require no
left payload clones for duplicate-witness updates. These are operation-count checks,
not measured wall-clock gains or Java parity. No upstream method credit is added.


`ExistsCompareNode::new_other`, `existence_other_compare_batched`, and
`existence_other_partitioned_batched` exclude right occurrences with the same stable
ID as the left row. Both inputs must share an identity namespace; payload types,
projections and upstream filtering may differ. Existence/absence, all five comparisons,
compound equality partitions, coalesced batches and graph rollback remain available.

Only the identity-excluding constructor retains an additional linear right-key
membership index. Equalities subtract the excluded witness from a key count.
Inequalities use the next extremum when the excluded occurrence is the sole witness
at the outer extremum. Right updates also reconsider old/new sole-witness IDs;
changing a second witness can change visibility even if the outer boundary stays
fixed. No matching-pair table or full left-bucket scan is required for this case.
Original regressions cover all comparisons/modes, independent/filtered inputs,
identity replacement, second-witness changes, payload-clone failure/retry, pruning,
rollback and full/stream solver agreement. Removing one of two equal right witnesses
against10,000 equal left rows emits one retraction with fewer than300 key comparisons.
No upstream method credits are added.

A native profile at 4822d4c still places the filtered indexed join ahead of graph
rollback and conflict-terminal scoring (335ms, 205ms and 123ms respectively in one
instrumented 96-variable hill/swap solve). Instrumentation includes overhead and
is hotspot evidence only. Two attempted bounded old-payload caches were rejected:
eager lookup regressed swap workloads; lazy lookup improved hill/swap but regressed
change workloads and tabu/swap. Production join code was restored unchanged.

Both experiments used the same baseline executable, alternating two process pairs,
32/96 variables, hill/tabu and change/swap, 20 steps, one warmup and three samples.
All assignments, scores and score-call counts matched, with independent score
validation. Timing varied substantially between pairs; no general speedup or Java
comparison is claimed. Raw results and immutable executable checksums are retained
in work/join-payload-cache-{eager,lazy}-performance.json. No compatibility credits.


`join_fixed_links_batched` compiles immutable relationships between stable input
IDs. Callers supply unique relationship IDs and left/right endpoint IDs; parallel
relationships and self-links are supported. Missing endpoints suppress output.
Output IDs contain all three identities, and batched replacements visit only
incident links, coalescing changes to both endpoints before predicate/projection.
Relationship metadata is retained across graph rebuilds; changing relationships
requires a new graph. Dynamic relationship facts still use ordinary joins.

Batched endpoint lookup retains one input-position word per declared endpoint,
plus two vector headers and a validity flag. Only positions from the preceding
indexed round are cleared before collecting new changes. Ordering, projection or
payload-clone panics cannot expose stale positions to a shorter retry. This is
private scratch state: callback/Clone/Drop order, captured rows, output identity,
snapshots, saved candidates and rollback contracts remain unchanged. The added
endpoint space avoids repeated searches within the changed-input lists.

The node retains O(links) immutable adjacency plus linked input rows, with no
accepted-pair table or intermediate edge/job tuples. Pure callbacks may be rerun
on old inputs to decide retractions. Existing graph journals, snapshots, saved
candidates, rollback and nested planning trials apply. Five original regressions
cover full-scan agreement, repeated updates, asymmetric inputs, self-links,
incident-only visits, input validation, failure recovery and saved candidates.
This native specialization earns no additional upstream-method credit.

The initial fixed-link benchmark (f38e1b3) used this operator in batched mode;
its fixed relationships were compiled inside timed setup. The immediate control
and Java adapter retain ordinary edge/job joins. Two alternating native process pairs
(20 steps, one warmup, three samples, 32/96 variables) preserved assignments,
scores and scoring-call counts, with 24–52% lower whole-process CPU across the
four algorithm/neighborhood combinations. These numbers include warmups and
both sizes, not per-solve latency. A fresh matched Java comparison validates
128 equal-score cases; the 96-variable change workloads are near parity while
swaps remain 3.1–3.3 times slower. Development-host measurements are specific
to this fixed-relationship workload, not general solver or dynamic-join parity.
Raw native evidence: work/fixed-links-performance.json; original Java run log:
work/fixed-links-java.log. The latest Java results live in work/stream-performance.json.


Fixed-link endpoint rows now occupy compiled dense slots. Each changed source ID
is resolved once; incident links read endpoints by slot, and nested journals undo
row replacements by moving old shared pointers. Full saved-candidate snapshots
copy one pointer per declared endpoint, without cloning payloads. This trades
linear snapshot cost for constant-time row access and cheaper trial updates.

`score_fixed`, `score_fixed_weighted` and `score_fixed_weighted_decimal` accept
a fixed universe of possible occurrence IDs. Duplicate declarations and incoming
IDs outside that universe are rejected, including unknown retractions. The
declared IDs supply canonical match keys. Integer/decimal weighting, overrides,
zero-weight pruning, checked/wrapping arithmetic, explanations and rollback retain
the ordinary scoring rules. Full snapshots copy O(declared IDs) row pointers.

Ordered match inspection lazily materializes a SnapshotMap and caches it until
state changes. It may clone IDs, payloads and scores; a clone panic leaves the
scoring state intact and inspection can be retried. Retained match snapshots are
independent. Activity detection uses a constraint marker and never forces this
materialization. Dense terminals favor fixed universes with frequent trials and
infrequent match inspection; they do not replace dynamically keyed terminals.

Six original scoring regressions and a dense-journal unit test cover scan/tree
agreement, nested commit/rollback, snapshot installation, payload/inspection
panics, saved candidates, arithmetic failures, weighted overrides, exact decimal
products and explanations. The existing fixed-link regressions exercise the new
endpoint storage. No upstream-method or fixture credits are added.

The dense-state comparison against f38e1b3 preserved native assignments, scores
and scoring-call counts. Whole-process CPU medians fell about 8–51% across four
algorithm/neighborhood combinations at 32/96 variables. Small tabu/swap latency
was mixed: an initial regression and a noisy focused follow-up near parity.
Separate 512/2048-variable hill/change comparisons improved median solve latency
about 30–32%. These are development-host workload measurements, not universal
speedups. Evidence: work/fixed-slots-performance.json,
work/fixed-slots-small-tabu-performance.json and work/fixed-slots-large-performance.json.
The refreshed Java comparison has 128 equal-score validation cases; large swaps
still take about 2.1–2.6 times Java's time. Match inspection is outside that solve
benchmark; the method records the lazy-materialization distinction explicitly.

## Replace an assignment without rebuilding the graph

Basic, list and mixed stream directors expose `replace_assignment(next)`. The
replacement must validate against the bound problem, including pins and ranges,
before callbacks run. Partial assignments remain allowed with their usual structural
score. The compiled graph, source identities, constraints, observations and immutable
fact rows remain bound. Every planning row is refreshed in one transaction; this
also updates payloads whose `Eq` implementation compares only identity. List value
rows keep their registered value payload and derive position/previous/next from the
replacement, matching initial binding and move semantics.

The returned `StreamAssignmentChange` exposes the assignment, scored graph, score,
terminal callback count and checkpoint count. Commit retains the replacement;
drop or callback failure restores the previous assignment and graph without callback
replay. The live assignment is installed only after scoring succeeds. Callback side
effects outside the graph are not rolled back. Initialization counters continue to
describe the original director construction. As with other guards, forgetting a guard,
process abort and panicking destructors are outside ordinary rollback guarantees.

Within a stream `MoveSession`, `replace_assignment` installs the replacement directly
and returns `Result<(), _>`, following the session's `apply` convention. Replacement
failure preserves prior session edits; successful replacements remain subject to
nested and outer rollback. This does not change solver phase orchestration or establish
upstream director identity/reuse compatibility. It avoids rebuilding the graph but
still refreshes all planning rows; no throughput improvement is claimed without a
workload measurement.


## Explicit sum-only keyed groups

`SumGroupNode<I,K>` captures every input ID's `(key,i64)` and keeps a private
member count and total per group. Input lookup is authoritative for retractions;
there is no independent per-value frequency index. Equal values and zero-valued
captures retain separate membership. `input(id)` reads a capture; `groups()`
inspects nonempty sums in key order. Clone shares immutable state and isolates
later edits; clear removes rows while preserving a fixed planning binding.
Existing IntCollector, IntSummary and full-summary group APIs remain unchanged.

`SumGroupNode::new()` uses checked signed-i64 arithmetic; `wrapping()` explicitly
uses modular arithmetic. Checked insertion adds the value, removal subtracts the
captured value, and same-key replacement checks `new-old` before adding that
difference to the total. Both intermediate overflows reject; changing keys removes
then inserts. Missing-ID removal is a no-op. `try_apply` and `try_apply_batch`
return SumOverflow without modification; their non-try counterparts panic on that
error. Count overflow also panics before installation. No Java batched arithmetic
ordering or post-error state equivalence is claimed.

`apply` publishes one immediate update. A batch keeps the first representative
and last value per input ID, processes final IDs in sorted order, and emits one
final put/retraction per affected group in key order. Group key representatives
survive temporary emptiness; new groups take the first final key in ID order.
Nonempty equal or zero totals still emit puts; only the last member retracts.
An absent-to-absent group emits nothing. Checked batch and reconstruction prefixes
must fit i64 even when the final sum fits; a reachable live state may fail a
sorted-source reconstruction if its prefixes overflow. A failed reconstruction
restores the original graph. Wrapping sums do not have this prefix restriction.

The graph builders `group_sum` and `group_sum_wrapping` publish each event;
`group_sum_batched` and `group_sum_wrapping_batched` explicitly publish final
batch groups. All return `StreamPort<K,(K,i64)>`. Mapping only runs for puts;
at a batch boundary it runs once for each final surviving put in ID order.
Direct transaction apply remains immediate, including on a batched builder.
The new boundaries integrate with existing graph transactions, saved candidate
snapshots, activity/rebuild, datasets and ordinary tracked score terminals. Full
source rows and mapped input captures remain stored. Small checked planning ID
universes may reuse private GroupInputs slots; larger/generic IDs retain maps.
Group outputs start a new ID namespace and do not forward an input binding.

Preparation and output-key cloning precede state installation. Reversible guards
restore captures and sums on ordinary comparison, Clone and arithmetic panics;
graph rollback also covers mapping, output erasure and downstream callback
failures. Snapshot installation/rollback does not replay mapping or scoring.
Stable immutable keys, deterministic callbacks and non-panicking destructors are
required. External callback/Clone side effects are not rolled back. These explicit
APIs introduce a smaller aggregate payload and different storage; they make no
promise of the full-summary API's Clone/Drop trace. Adding these APIs preserves existing declarations. The canonical stream solve
adapter now explicitly selects wrapping sum grouping because its constraint uses
only the grouped total; its bounded inputs match both arithmetic modes. This
changes the native declaration and terminal payload to `(key,i64)`, retaining
source captures, nonzero filtering, match inspection and actual score callbacks.
Full-summary production operators remain available and provide the retained
comparison control. Fixtures, budgets, move order and validation stay unchanged.


Journal checkpoints for fixed sources, fixed links and ordinary/fixed/contribution-only
terminals may retain one empty carrier allocation per node. An active carrier owns
its marker or cloned score; consuming it moves the marker/score out before the
empty allocation is recycled. Score Clone still precedes opening the frame, and
score Drop/assignment remains at the original commit/restore point. Nested markers
own separate carriers and full saved snapshots keep their existing types. At most
one idle allocation survives; it contains no score, key, tuple or snapshot. This
trades bounded retained allocation space for fewer repeated allocations, without
a standalone speed or whole-graph memory-saving claim.

## Explicit batched filter-map

`StreamGraphBuilder::filter_map_batched` combines a zero-or-one projection with
final-ID publication. A delivered batch retains the first ID representative and
last payload per ID, visits final IDs in sorted order, and maps each surviving put
once. Retractions do not run the projection. Every accepted final put publishes,
including an equal or zero output; a rejected put retracts a previously accepted
row. Direct transaction applies remain immediate. Inputs and outputs need no
`Clone` bound. Stable IDs, deterministic projections and non-panicking destructors
are required; external callback effects cannot be rolled back.

The node retains acceptance for every known source fact, including rejected facts,
in the existing native filter cache. It retains neither input nor output payloads;
full facts remain upstream. Complete validated planning universes can reuse the
existing filter slots and undo journals (usize IDs 0..count, count at most 256).
Ordinary, incomplete, generic or larger input universes retain map storage. The
explicit boundary forwards validated source identity to eligible downstream
storage; group/join/expansion namespace changes still stop propagation. Captured
acceptance, nested transactions, full saved states, activity and rebuild use the
normal graph lifecycle without replaying projections during restoration.

This new declaration allows different projection order/count and own/downstream
ID or captured-key Clone/Drop schedules, including snapshot release order. It does
not promise the physical trace of separate filter/map nodes or ordinary
`filter_map`. Existing `filter_map` binding/publication flags and its public native
node remain unchanged. Its direct batch collector preserves the old per-event
loop, including repeated events, and has original ownership/panic comparisons.
The new boundary has original regressions for independent source-state models,
sparse membership, retained snapshots, failed callbacks, nested and saved trials,
non-Clone intermediates, zero outputs and actual score callbacks. These original
tests earn no additional upstream-method credit.

## Shared fixed-link inputs and fused tracked scoring

`join_fixed_links_batched_shared` is an explicit counterpart of the ordinary
fixed-link join. Its endpoint slots retain the complete immutable producer
`TupleChange` through that event's existing `Rc`, including the input ID. Endpoint
installation neither clones the payload nor allocates another row owner. Both
endpoint payload types and projected outputs may be non-Clone. Ordinary source,
output, journal and snapshot costs still apply. Retaining complete events can
extend ID/payload lifetimes and increase retained memory. The existing join keeps
its original payload Clone/allocation behavior; this new declaration allows
different Clone/Drop counts and release schedules.

`score_fixed_links_batched_shared` fuses fixed-link projection with fully retained
scoring. Integer-weighted and exact decimal-weighted counterparts support named
weight changes and disabling. The result is an ordinary inspectable constraint
with complete `((relationship_id,left_id),right_id)` keys and `(payload,score)`
matches. Endpoint payloads need no Clone bound; projected payloads implement Clone
for lazy ordinary match inspection. Zero/equal accepted scores remain real matches.

Both sides settle their final delivered event per ID. Each affected relationship
is visited once in relationship-ID order. The fused declaration projects and
scores each accepted pair immediately, without intermediate link events or queue
routing. Retractions use captured accepted membership and do not re-evaluate the
old predicate. Projection/scoring order and ownership schedules can therefore
differ from separate operators. Callbacks must be pure, keys/content stable and
destructors non-panicking. All full source facts remain upstream, including
unlinked, filtered or disabled facts. Topology is immutable; duplicate relationship
IDs, foreign ports and duplicate constraint names are rejected before registration.

Endpoint caches settle under the graph checkpoint before fused callbacks, which
then read typed shared rows directly. Each prepared match row is installed under
the existing journal as callbacks advance, avoiding a second batch vector; totals
publish after all projection/scoring/arithmetic preparation succeeds. A failed
transaction is unreadable and restores all endpoint/match state when dropped.
Nested journals, full snapshots, saved winners, activity reconstruction and weight replay retain the
normal graph contract. Ordinary updates use configured checked/wrapping arithmetic;
replay validates completed constraint totals before publication. Structural,
impact, terminal-total and graph-total failures remain errors. Inspection can
clone IDs, payloads and scores, and preserves independent held snapshots.

Fused scoring counts actual evaluate/factor attempts, including failed or panicking
calls. Its physical counter is outside logical snapshots; a dispatch guard records
the delta in the current transaction even on unwinding. Nested diagnostics retain
separate transaction totals, and the existing metric getter stays constant-time.
Legacy incoming-event accounting is unchanged. A private optional counter registry
adds one slot per graph node and a branch at downstream dispatch; native comparisons
must include the old executable to expose costs on unchanged declarations. No
whole-graph memory or speed claim follows from sharing alone.

## Bound sum summaries and ordinal batch scratch

After a validated fixed-input binding succeeds for at most 256 `usize` input IDs,
sum-only groups whose key type is exactly `usize` use private indexed count/total
slots for keys 0 through 255. Larger keys, including `usize::MAX`, retain ordered
map storage; generic keys and unbound inputs retain the original map path. The
input universe does not restrict the mapped group-key universe. All captured
source values, nonempty zero totals, sorted publications, checked intermediate
arithmetic and representative rules remain intact. Nested undo journals and full
saved states include both storage portions. No new public bounds or declaration
are required. The indexed portion reserves 256 optional summary slots and shares
256 integer key representatives; full saved snapshots copy its primitive slots.
This trades bounded retained space for fewer shared-tree updates and allocations.

Batched sum grouping and batched filter-map also reuse private input-position
scratch when the identity type is exactly `usize`. It contains only integer IDs
and event positions, retains the last delivered event per ID and visits final
inputs in sorted ID order. Sparse and arbitrarily large IDs need no universe
scan. Scratch is rebuilt before every round, retains no input owners and is not
logical snapshot state. Generic identity types retain their original map-based
coalescing and ownership behavior; ordinary filter-map is unchanged. Sum mapping
still completes before group installation, and graph transactions retain normal
panic poisoning and rollback. Full source facts and score callback counts are
unaffected.


## Explicit fixed-universe filtering

`StreamGraphBuilder::filter_fixed(input, ids, predicate)` declares arbitrary
immutable ordered IDs and stores only absent/rejected/accepted membership (one
byte per ID), with changed-slot undo and full byte snapshots. Duplicate IDs are
rejected before graph installation; unknown delivered puts and retractions are
errors. A delivered batch validates all IDs before predicates/state changes, then
processes every event in delivered order. This is not a final-ID publication
boundary; pre-existing upstream batching still determines delivered events.

Every put calls its pure predicate exactly once. Accepted puts forward the actual
complete immutable input event, retaining its incoming ID representative and
payload owner without requiring payload Clone. Rejection/removal of an accepted
row emits a retraction cloning the canonical sorted declaration ID. Other removals
emit nothing. Declaration IDs identify cache slots, not canonical replacement
keys for accepted puts. Full source facts, including rejected facts, stay upstream.
Own and downstream Clone/Drop schedules and observable key representatives may
differ from ordinary filter. Stable ordering/identity and nonpanicking destructors
are required. Predicates, generated-key clones and downstream panics use ordinary
graph transaction rollback; snapshots never replay predicates. Physical score
callback accounting is unchanged. Full snapshots cost O(declared IDs) bytes,
and generic/sparse lookup costs O(log declared IDs); contiguous usize lookup is
direct. The node conveys existing validated unary source identity but never
invents a planning/group binding from its declaration. Ordinary Filter/FilterNode
remain unchanged. No performance or whole-graph memory claim is established.


## Native sorted-set snapshots

`SortedSetCollector<V: Ord + Clone>` is an explicit additional collector returning
`SnapshotSet<V>`. Existing `SetCollector` and its `SnapshotMap<V, ()>` output stay
unchanged. One persistent counted AVL retains distinct representatives and their
occurrence counts. The first representative survives until the final occurrence
of its ordering class is removed. Replacing a singleton with an equal-class new
value installs the new representative; replacing one of several occurrences
keeps the original representative. Missing removals/replacements return false.

`finish()` and clones share a root without traversing or cloning payloads. Later
updates isolate saved collectors and outputs. The output exposes sorted borrowed
`iter`, `first`, `last`, `contains`, `len` and `is_empty`; borrowed IntoIterator is
ascending. Membership equality ignores private multiplicities and representative
payload differences within the same ordering class. Iterators are exact-size and
fused, use 32 inline traversal slots, and spill for unusually deep trees.

Replacement preflights comparisons, required cloning and count arithmetic before
committing prepared paths when topology remains stable for those paths. An
existing different destination can be incremented before detaching a singleton
old class without changing observable removal-then-insertion semantics. Other
cases keep an isolated staging root. Comparison/clone panics preserve collector
state; user side effects are not undone. Ordering must remain lawful and stable,
and Clone must preserve the ordering class. Panicking destructors and allocation
aborts are unsupported. No Hash or generic static bound is added.

The aggregate benchmark selects this real production collector for sorted-set
workloads and still observes every sorted member after update and restore. All
original fixtures, work, assertions and committed observations remain required;
this selection is explicit, not a change to the old collector's default API.
## Primitive numeric cache storage

Graph construction can select sparse paged caches for exact `usize` IDs and
supported tuples of `usize` coordinates. These caches preserve numeric ordering,
the full unsigned coordinate range, and existing key and payload owners. Expanded
occurrence IDs do not require an inferred source bound. Generic and borrowed IDs
retain ordinary map storage; custom payloads, scores or group keys with observable
destruction retain their existing snapshot ownership paths.

Private saved-candidate captures can retain only the final entries touched since
the candidate's opening journal, including touched join membership buckets.
Installation requires restoration of that same opening state. It runs no mapping,
predicate or score callbacks and preserves enclosing rollback journals. Independent
full graph checkpoints continue to retain complete state. Public match inspection
materializes an ordered snapshot using the retained owners; cache storage does not
change contribution counts, intermediate publications or scoring arithmetic.

These implementation choices earn no upstream-method credit. Application timing
and quality evidence remains separate from the original compatibility inventory.


## Private integer grouping storage

Ordinary graph integer grouping can journal each eligible primitive-key collector
edit with an inverse operation. Shared or Weak collector observers and singleton
representative replacement keep the original path. Observations of inverse-dirty
collectors copy the internal value and share the actual canonical key; full/current
snapshots and enclosing rollback remain independent. Generic PrimitiveRows and
public standalone grouping retain their existing storage behavior.

Grouped-input page containers can be privately owned Boxes. Full snapshots copy
these containers and share the actual row Arcs. For a certified primitive captured
key and i64 value in ordinary graph Dense/Paged inputs, an identical final store
can be omitted when the current row owner is exclusive. Every mapping, group
operation, singleton key rebirth, emitted summary and scoring callback still runs
in its original phase, including bounds and error handling. Generic, standalone,
borrowed and unbound tree inputs retain every original store.

A later private input snapshot may share an omitted historical input allocation.
GroupInputs has no public owner accessor; these exact builtin values have no user
Clone, Drop or equality hooks. Separate public group keys, terminal match tuples
and their retained snapshot owners are unchanged. This private input choice does
not authorize skipping public terminal value versions or downstream callbacks.


## Graph input values and concrete callbacks

Ordinary graph grouping may store bounded captured input occurrences by value
when both the identity and mapped key are certified exact builtin types. The
original Dense/Pair binders first validate all retained IDs and universe limits.
Numeric undo, current captures and full snapshots copy values, with enclosing
rollback preserved. Private historical input allocations have no identity promise.
Separate public group keys, summaries, datasets and terminal match owners retain
their original versions. Standalone, custom, borrowed, unbound and explicitly
batched grouping keeps its previous storage. Sparse pages contain larger values;
full snapshots trade shared owners for copies of numeric values.

Private map, ordinary group, membership-filter and direct-terminal nodes may
retain concrete callback types. This changes dispatch and per-closure code
generation while preserving callback invocation, clone/destruction order,
metadata and error phases. Weighted terminals and explicit batched group aliases
retain their boxed callback path. Callback concreteness does not imply purity.

A primitive flat-map graph cache may omit an existing identical usize length only
after producing every original retraction/put and completing mapper, iterator and
identity-clone work. An absent expansion and a present empty expansion remain
distinct. These private length owners have no public accessor; public downstream
row replacements remain fresh. Standalone and custom paths keep all writes.

A terminal with exact primitive IDs and builtin integer scores may locate its
current contribution once at the original old-score phase. Evaluation, structural
validation, checked adjustment and delivered payload cloning precede publication
through the ordinary prepared insert/remove. This does not omit equal public row
writes or alter canonical keys, value owners, snapshots or rollback. Generic score,
identity and destruction-sensitive paths retain their existing sequence.
