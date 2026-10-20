# RealWorld application workload provenance

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
The originating RealWorld manifests supply public-distribution authorization;
no additional third-party license is asserted by this integration.
