# Roadmap

TimeTuck is an experimental Rust library. This roadmap describes priorities,
not release dates or a stable API commitment.

## Public-development preparation

- Make the native modeling workflow approachable through a short README, examples,
  architecture, and a compatibility guide.
- Complete the [publication review](public-release-preparation.md), including
  history and Actions disclosure, attribution, and a private security-reporting route.
- Keep the repository private until the owner explicitly authorizes publication.
  Keep all crates unpublished during this preparation.

## Solver development

- Improve complete application performance with independently checked scores and
  separate solution-quality reporting. The active RealWorld throughput work is
  described in the [implementation handoff](next-session.md).
- Preserve demonstrated native planning behavior while improving internals.
  Extend deterministic regression coverage where application usage reveals gaps.
- Make documented native policies and known reference discrepancies easy to find.
  A shrinking compatibility backlog must reflect complete assertions and actual
  executions; exclusions and original tests do not earn upstream pass credit.
- Review ergonomics with real Rust models before promising API stability.

## Before a distributable crate release

Establish a supported Rust version and platform matrix, decide versioning and
API-stability policy, verify package contents and notices, and deliberately select
release-specific correctness and performance checks. Publishing a repository and
publishing a crate are separate decisions.

Java API/ABI parity, Java framework loading, Enterprise features, and universal
performance or stochastic-trace equivalence remain outside the current scope.
See [Timefold compatibility](timefold-compatibility.md).
