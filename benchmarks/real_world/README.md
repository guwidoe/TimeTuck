# Public RealWorld application benchmarks

These are extra application tests. They add zero Timefold upstream-method credit
and do not change the original 18 comparison jobs or 109 timed workloads.
`origin.json` identifies the original RealWorld application spike source. Public
fixture approvals, hashes and notices are in `tests/application/real_world/`.
No Timefold production implementation was consulted or copied.

Run from the repository root with pinned Community 2.7.0 jars already available
under `work/oracle/solver/dependencies/`:

```sh
python3 scripts/benchmark_real_world.py
```

The default is both complete public cases, seeds 7, 11 and 23, 10 seconds for each
runtime, paired serially on the same machine and CPU affinity. Runtime order
rotates by seed. Rust and Java artifacts build locally before timing. The Java
reference executes the actual Timefold constraint streams engine. The optional
`--java-modes streams incremental` adds the original application's handwritten
Java domain-cache scorer as a distinct comparator, and `--rust-modes streams
incremental` adds the native handwritten scorer with the same caches (occupancy,
members, pair encounter counts and attribute tallies). The two incremental modes
are compared with each other; neither replaces the streams reference. `--no-build` requires
a matching source/artifact receipt produced by a previous runner build.

For a short diagnostic, use `--seconds 0.2 --seeds 7 --report work/real_world-smoke.json`.
A nondefault matrix is never marked `qualified`. A qualified report means the
complete default matrix was measured and independently validated; it does not
mean speed, score quality or general Java parity was achieved.

The report is `work/real_world-performance.json`. Timestamped directories under
`work/real_world-runs/` retain initial schedules, flattened inputs, assignments,
converted Java schedules and per-process receipts. The report records exact
commands, stdout/stderr/errors, versions, machine, affinity, input/artifact hashes,
configuration, work counts and raw hard/soft scores. Solver elapsed time excludes
process startup; process wall time is also retained. No old result is rekeyed or
relabeled. Source changes during a run invalidate its evidence.

Independent Python recomputation reads the original JSON, not the solver caches
or flattened model. It checks initial and final hard/soft scores even when the
schedule is infeasible. Attendance, known IDs, unique assignment, capacities,
fixed placements and all represented pair/balance constraints are inspected.
Hard capacity and pair violations remain visible as negative hard scores; they
are not suppressed by an infeasibility-only validator. Structural or pin violations
and either score-component mismatch fail validation.

The retained spike uses integer soft weights multiplied by 100, fixed placements
first and most remaining capacity with the last group breaking ties. Both runtime
initial scores must equal independent recomputation. Group order within an
exported member list is not a scheduling decision. Attribute balance applies to
occupied groups only, preserving the application spike's sparse stream policy;
this benchmark does not claim all RealWorld product semantics. Public weights
are exactly integral after scaling; other fractions are rejected.

Both models offer capacity-filtered individual changes and same-session swaps.
They lack whole keep-together moves. Both accept with the same per-level
`exp(-loss / temperature)` rule at `0hard/500soft` and cool linearly over the time
limit. The Java change filter reads occupancy from the solver's working solution,
not from the input clone. Equal numeric seeds do not imply equal RNG
streams, move counts or traces. The sailing case can remain infeasible and its
quality is not equal-gated. Per-runtime evaluation throughput and raw hard/soft
quality are reported separately. `throughput_target_met` is a bounded observed
throughput target, not Java behavior/quality parity. Exported RealWorld
`solver_settings` are preserved metadata, not the configuration of these solvers.

Focused checks:

```sh
python3 -m unittest discover -s scripts -p test_real_world_reference.py
```

## Superseded comparisons

Reports produced before the occupancy and cooling corrections are not like-for-like.
The Java change filter counted occupancy on the unchanged input seats, so on Sailing
it admitted changes into groups that had since filled and traded keep-together
violations for capacity violations. Golfer was unaffected because every group stays
full there. The native model cooled over 40000 steps and still ran hot when a
10-second run completed fewer steps. Retained observations keep their original
bytes; treat their Sailing Java quality and throughput, and both runtimes' quality
comparisons, as superseded until the default matrix is rerun.
