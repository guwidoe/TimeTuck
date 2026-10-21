# Naming and provenance

## Project identity

TimeTuck began with the idea of rewriting Timefold Solver Community in Rust,
with native deployment and performance as motivations. Production Rust is
independently implemented against Community tests and public API specifications;
adapted test and fixture material retains its own attribution. The
[compatibility overview](timefold-compatibility.md) explains why this origin does
not imply Java drop-in compatibility or general performance superiority.

The project name is **Timetuck**; the repository and Rust crate use `timetuck`.
The user selected it on 2026-10-01. It is wordplay on folding/tucking time, not
an official Timefold port. The name has not received formal trademark clearance.
`timefold-rs` was avoided because it could suggest an official port or affiliation;
adding `-rs` would not remove that concern.
Use Timefold in factual compatibility/source descriptions and retain required
attribution. Do not reuse its logos or imply endorsement.

Before committing to a public brand, check similar marks in relevant jurisdictions
and software classes, as well as repositories, package registries, and domains.
A web search or free crate name is not legal clearance. Obtain legal advice for
commercial branding or unresolved licensing/patent questions.

## Sources and reuse

Timefold states that Community is Apache-2.0 and Enterprise is proprietary.
Apache-2.0 section 4 sets redistribution conditions, including providing the
license, identifying changed files, retaining applicable notices, and carrying
forward relevant upstream NOTICE attributions if present. Section 6 does not grant
general trademark rights. Implementing from scratch does not waive obligations
for tests/fixtures that are copied or adapted.

- Keep the full pinned upstream checkout and its license intact.
- Mark adapted test files prominently as modified/adapted from the named source.
- Preserve applicable copyright/attribution notices and license information.
- Add every copied/adapted test, data file, and fixture to the provenance ledger.
- Update `THIRD_PARTY_NOTICES.md` when importing material outside the submodule.
- Review individual files/assets for different terms; do not assume every asset
  has the repository-level license.
- Keep Enterprise material out of the reference and implementation.

Original implementation and a formal clean-room process are different claims.
This scaffold requires original production code but does not establish a legally
reviewed clean-room organization or guarantee freedom from third-party claims.

## References (checked 2026-10-21)

- https://timefold.ai/license
- https://timefold.ai/trademarks
- https://www.apache.org/licenses/LICENSE-2.0 (especially sections 4 and 6)
- https://github.com/TimefoldAI/timefold-solver/tree/8b815bc6b5f3eb81feedc94dea0885a93b267762
