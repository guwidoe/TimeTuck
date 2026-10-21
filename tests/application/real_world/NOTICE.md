# Real-world application workload provenance

These are extra, application-origin workloads and original native regression tests.
They are separate from adapted upstream Timefold tests and earn **zero upstream
method credit**. No Timefold production source was inspected or copied for this model.

The model derives from the existing Claude RealWorld `native-application` application spike,
session `application-source-session`. Exact source paths, SHA-256 digests,
and modifications are recorded in `source-manifest.json`. The original private
scratchpad is not redistributed as an extra copy; the derived support module is
`crates/solver/examples/support/real_world_model.rs`.

Both `problem.json` and `manifest.json` files are copied byte-for-byte from the
RealWorld **public** scenario distribution. Their embedded distribution and
privacy notices are preserved. The sailing case is the approved anonymized public
derivative, not its restricted internal parent. `solver-settings.json` is the
original application spike's solver settings, carried into output for interchange.
No upstream Timefold license or test identity is claimed for application material.
The originating manifests supply public-distribution authorization. The copyright
owner separately authorized the bundled application material below under
Apache-2.0 on 2026-10-21:

Copyright (c) 2022-2026 Guido Witt-Dörring.

- The two approved public problem snapshots and their unchanged manifests,
  and `solver-settings.json` in this directory.
- The adapted native model in
  `crates/solver/examples/support/real_world_model.rs`, its benchmark CLI and
  application regression tests.
- `benchmarks/real_world/ApplicationBenchmark.java`, `prep.py`, and the application benchmark
  runner and independent reference scorer under `scripts/`.

These bundled files are licensed under the repository's [Apache License
2.0](../../../LICENSE). This grant covers these files and their distributed
adaptations only; it does not license the rest of the originating private
application. Original source hashes, modifications and fixture distribution
notices remain recorded. Application material earns no Timefold method credit.

Private source paths and session identifiers in the provenance records are
stable public aliases. Source hashes identify the original private bytes; they
are not checksums of renamed files. See [history preparation](../../../docs/public-history.md)
and the [redaction record](../../../compat/public-redaction.json).
