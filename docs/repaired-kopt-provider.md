# Seeded repaired k-opt provider

`RepairedKOptProvider::new(sizes, weights, seed, max_attempts)` feeds the existing
list `*_with_provider` searches for dense and stream scoring. It is optional;
built-in providers and the odd sequential selector are unchanged. The existing
generic even-k and repartition samplers remain available for broader partition
exploration.

Positive weights select consecutive initial cut counts starting at the configured
minimum; positive odd-count weights are rejected. The configured maximum also
bounds final cuts after repair. Each splice adds two cuts, so a small maximum may
reject disconnected initial tours. No retry exceeds the per-opening attempt
budget. `open(problem, assignment, step)` validates the assignment, including when
disabled, then lazily attempts proposals. `attempts()` includes rejected draws;
there are no attempts when no movable owners exist.

Each attempt samples an unpinned owner uniformly, then one position uniformly in
its movable suffix, and a direction. Selected owners appear in first-draw order;
their suffixes form one combined cyclic tour. Repair callbacks sample currently
retained forward original edges whose endpoints share a current component, using actual
combined adjacency. The selector chooses the other bridge edge under its native
policy. Duplicate cuts, impossible bridges, oversized initial counts and exhausted
cut budgets emit no move. Tour orientation and first endpoint are sampled.

Destination lengths are the selected owners' current movable suffix lengths,
never caller-supplied expected result partitions. The existing move factory and
checker enforce receiving ranges, pins, dependencies, the internal proposal
snapshot and actual change. Emitted native moves are positional; consume them
before unrelated edits rather than treating them as snapshot-bound replay receipts. A caller cannot mutate the borrowed opening assignment during iteration;
reopen after edits. Search passes each live assignment and step to the provider.
For identical configuration, opening assignment and step, native draws repeat;
this is not uniform final-tour sampling or Java RNG trace compatibility.

Four original controls verify varied multiowner lengths, pins and undo; receiving
range rejection and reopening after ownership/length changes; finite attempt accounting and disabled-input
validation; and real dense/stream search under seed37, five steps,64 attempts per opening and256 evaluations.
Final scores are recomputed independently from the resulting routes. No source
method or fixture credit is added. The two nonsequential Community methods remain
pending because their raw partner arrays expose sixteen slots while the native
active graph derives twelve for those fixtures. No surplus slots are fabricated.
