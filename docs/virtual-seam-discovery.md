# Optional virtual-row three-opt discovery

`discover_virtual_seam_three_opt(problem, assignment, max_attempts)` derives the
existing `VirtualSeam3OptPlan` family from the current assignment. It traverses
ordered pairs of distinct unpinned entities. The first movable suffix must have
at least four values: its local wrap cut and strictly interior adjacent cut
supply four distinct endpoints. Each adjacent cut of the second movable suffix
supplies the other two. Payload labels, caller-supplied edges and target lengths
are unnecessary. Pin prefixes remain outside both virtual rows.

The iterator is lazy and deterministic in entity/cut order. The attempt limit
counts topology candidates before receiving-range/dependency rejection, rather
than successful moves. Zero validates the assignment and produces no candidates.
The search engine's scoring/evaluation/step limits remain independent. Discovery
may retain duplicate resulting states from different cuts and is not uniform
sampling. Each plan binds its endpoints to this snapshot; reopen discovery after
an edit. Current assignments are validated, and plans are checked against actual
receiver ranges and dependency constraints before publication.

`VirtualSeam3OptProvider::new(max_attempts)` implements the existing `MoveProvider`
interface for both dense and stream list local search. Each scan regenerates
coordinates from live positions and ownership. `ProviderUnion` can combine it
with existing families; built-in default neighborhoods and their seeds are
unchanged. Arbitrary entity counts are supported by selecting eligible pairs;
this remains the specific two-row virtual-wrap/internal/internal topology, not
all possible multi-row k-opt descriptors.

The old combined-cycle repartition sampler is not missing general repartitions:
it samples physical reconnections, orientations and weak compositions, and
larger cut sets can represent these virtual-row outcomes. But three declared
virtual cuts can change four (or more) physical combined-cycle edges. Such a
candidate is outside the physical k<=3 proposal space: physical reconnection
retains every uncut edge, independently of rotation and destination partition.
The independent original regression certifies this distinction without a Java
fixture or an expected-state table. Larger physical-k coverage is not disproved.

Five original tests exercise live discovery, arbitrary labels/shapes, pins,
owner permutations, bounded attempts, receiving ranges, exact allocation
ownership and undo, the physical-edge certificate, and seeded bounded real
search with both scoring backends. These add no upstream-method credit. No
performance claim or Java sampling/ABI equivalence is established.
