# Integration test layout

`bash scripts/check.sh` runs the original integration tests through 16 generated
shards and six standalone targets. Original files, assertions, fixture paths and
provenance identities stay in place. The standalone targets preserve crate-root
lookups and isolate two pairs of PID/counter-based temporary-directory fixtures.
Each other original file is a separate public module, retaining its helper
visibility and static state. Fixture modules are not deduplicated across files.
Generated harnesses allow only Clippy's `duplicate_mod` lint for that intentional
fixture isolation; native-library lint settings are unchanged.

After adding or removing a top-level integration test, run:

```sh
python3 scripts/rust_test_layout.py --write
```

Commit the generated Cargo entries and wrappers with the test. Validation rejects
unregistered additions, missing files and edited wrappers instead of silently
dropping tests. A stable filename hash assigns shards, so an addition does not
redistribute existing files. All original Cargo target names remain available:

```sh
cargo test -p timetuck --test bounded_group_planning_regression
```

The verification runner compiles affected executables together before running
them, using four compiler jobs by default (`CARGO_BUILD_JOBS` overrides this).
It caches each actual executable using its source/fixture dependencies. An edit
can invalidate its owning shard; focused development can still run the individual
original target. Native Cargo settings and the selected
test-target declaration have separate input projections. Adding another target
does not invalidate unrelated tests, documentation tests or measurements. Native
dependencies, profiles, features and other build settings remain hashed.

Captured logs keep their real module-qualified names. The report parser removes
exactly one validated shard-module prefix to attribute a result to its original
path and test name. Unknown prefixes fail; missing/ignored tests earn no credit.
Old results keep their original source keys and dates. A fingerprint-schema change
can make previous measurements stale; it does not justify relabeling them or
refreshing every benchmark during a test-layout change.
