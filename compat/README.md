# Compatibility assets

`upstream.json` records the exact Community reference; the submodule pins the same
commit. `provenance.json` records 66 adapted methods: 47 score methods, ten basic
move cases, eight neighborhood-provider cases, and one acceptance case. `exclusions.json` documents JVM integration boundaries. The original
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
