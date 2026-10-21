# Compatibility assets

For project origin and migration expectations, start with the
[Timefold compatibility overview](../docs/timefold-compatibility.md). For timing
claims and reproduction, read [performance](../docs/performance.md).

`upstream.json` records the exact Community reference; the submodule pins the same
commit. `provenance.json` records adapted test methods and fixtures, including
their source paths, licenses and modifications. The generated dashboard reports
current verified counts by subsystem; `exclusions.json` documents JVM integration
boundaries. The original
Java adapters under `oracle/` call public Community APIs; no Java implementation
is translated into the production Rust library. See `docs/score-contract.md` and
`docs/dashboard.md` for the verified scope and measurements.

Illustrative record shape (not an implemented case):

```json
{
  "id": "stable-case-id",
  "status": "pending",
  "upstream_commit": "<40-character SHA>",
  "upstream_path": "core/src/test/java/.../SomeTest.java",
  "upstream_test": "testMethod",
  "local_path": "<adapted test or fixture>",
  "license": "Apache-2.0",
  "notice_paths": [],
  "modifications": "Describe translation, fixture extraction, and semantic changes"
}
```

Do not add invented upstream identities. Verify paths and methods against the pin.

For nested Java tests with repeated method names, a case may include an integer
`upstream_line` naming the declaration line at the pinned revision. Path, method
and line must resolve to exactly one inventoried declaration. Without this field,
ambiguous names remain errors; a stale line is never silently remapped.
