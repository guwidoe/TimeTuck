# Compatibility assets

`upstream.json` records the exact Community reference; the submodule pins the same
commit. `provenance.json` starts with zero adapted cases. Add fixtures and the Java
oracle adapter here when the first behavioral contract has been selected.

Example case fields for the future ledger (not an implemented case):

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
