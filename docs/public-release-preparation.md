# Public-release preparation review

Review date: **2026-10-21**. Publication is **not authorized** by this review:
the owner requested preparation while keeping the repository private. GitHub
reported `PRIVATE`; no visibility, sharing, release, package-publishing, or remote
repository settings were changed. Crate publishing remains disabled.

## Documentation prepared

The [README](../README.md) now leads with problems, a runnable example, capabilities,
and links to [guides](README.md). New material covers getting started, architecture,
Timefold migration expectations, performance methodology, a roadmap, contribution
instructions, and security reporting. Existing contracts and detailed evidence
remain available; the dashboard documentation now labels its retained acceptance
counts as historical.

The origin is explicit: the idea was a Rust rewrite of Timefold Solver Community.
The documented claim is independently implemented native behavior checked against
pinned tests and public APIs. Existing Java applications need migration; general
drop-in compatibility and universal speed superiority are not supported claims.

## Review scope and results

The initial audit snapshot was commit
`2154501abb466133db3e741d65992d2e13942950`. The shared checkout continued receiving
other solver-development commits during preparation. Local audit receipts record
the captured refs and subsequent incremental checks; they are kept in ignored
`work/public-release-audit/`. These security/documentation checks do not replace
source-bound solver correctness or performance qualification.

| Check | Observed result |
| --- | --- |
| GitHub visibility | Private; no published releases, no issues/pull requests returned, and no Actions artifacts at inspection. |
| Remote history coverage | All 32 remote branch tips were contained in the captured local graph of 1,249 reachable commits. No tags were advertised. |
| Secret scanning | Gitleaks 8.30.1 scanned the captured Git history, the tracked working-tree snapshot, and all available Actions logs. Its downloaded release checksum was verified. |
| History findings | 64 generic-key matches: 60 evidence/cache fingerprints, two measurement-fingerprint references, and two collector-key descriptions. Reviewed as false positives; no unresolved credential findings. |
| Working-tree findings | 43 matches: 42 evidence/cache fingerprints and one collector-key description. No unresolved credential findings; no broad scanner allowlist was introduced. |
| Final documentation and history delta | All 15 documentation files changed by this pass scanned with zero findings; the committed development tail after the captured audit source also scanned with zero findings. |
| Actions logs | All 46 runs downloaded, covering 184 log files; zero scanner findings. No workflow runs or logs were deleted. |
| Sensitive file names | No tracked environment files, PEM/private-key files, or common SSH private-key filenames were found. |
| Community reference | Unmodified Community 2.7.0 pin verified; 1,501 Java test-source files and 4,855 annotated source-method identities inventoried. No JUnit suite was run. |
| Adapted cases | All 4,414 ledger identities resolve to the pin with existing local paths and notice paths. The ledger marks 4,412 implemented and two excluded. This is mapping validation, not 4,412 new test passes. |
| Fixture provenance | All 2,371 fixture records have valid pinned source paths, recorded modifications, licenses, and existing notice paths. Two historical ignored diagnostic files are absent; see below. |
| Application fixtures | Both exact RealWorld snapshots match the source-manifest hashes and their approved public-distribution manifests. One is synthetic; the sailing derivative is explicitly approved and anonymized. |
| Dependency license metadata | Cargo resolved 36 packages with declared license expressions; none lacked a declaration. This checks metadata, not every dependency's redistribution contents or advisories. |
| Fresh-start example | An isolated archive of the audit source, with an empty build target and no initialized upstream submodule, built and ran `cargo run --locked --example tiny_schedule` with Rust/Cargo 1.92.0. Output matched the README: 243 evaluations, exhausted search, feasible score `0hard/-3soft`. The existing machine toolchain and Cargo dependency cache were available. |

Scanner reports were redacted and remain local. A pattern scan and provenance
validation cannot prove absence of every secret, private datum, licensing defect,
or third-party claim. Production-source authorship was not forensically audited.
The native implementation boundary and attribution records remain the project's
stated development policy.

## Anonymization follow-up — 2026-10-22

The owner authorized repository and history anonymization while keeping the
repository private, approved Apache-2.0 for the specifically bundled application
files and public fixtures, and selected GitHub private vulnerability reporting
for activation after publication. The application copyright remains
Guido Witt-Dörring's; the rest of the originating private application is outside
the grant. See the [application notice](../tests/application/real_world/NOTICE.md).

A separate release candidate uses `real_world` throughout code, imports, Cargo
targets, file paths, provenance, report keys, branch names and commit messages.
Private source paths, session identifiers, direct author email, machine names
and archived SSH destinations are anonymized. Required notices, author names,
original source hashes and measured results are retained. The original private
history and editor checkpoints remain in an ignored private backup; the active
solver-development checkout has not been rewritten.

The captured original source is `acfbaaad908a5e964dae3344083173d985544b04`.
The initial filter covered 1,279 commits. Ninety private editor checkpoint refs
were archived and excluded from the release mirror. The completed mirror audit
covered **1,194 commits across 132 branch refs** and every stored Git object,
including tree filenames and unreachable objects, with **zero forbidden-identity
findings**. A final documentation commit and bundle check are recorded in the
private preparation receipts. The public [redaction record](../compat/public-redaction.json)
keeps neutral current paths and separate original/candidate checksums.

Preparation verification:

- **29 focused Python tests pass:** eight independent application scoring/conversion
  checks, three application-report checks, eleven origin-accounting checks,
  three publication-boundary checks, and four history-tool checks.
- **Four Rust application regressions pass**, covering both workloads' full
  scoring/rollback and deterministic bounded search. The renamed benchmark example
  checks successfully, and the README example returns 243 evaluations and
  `0hard/-3soft`. The renamed Java benchmark compiles against the pinned Community
  artifacts. No full Rust suite, benchmark matrix or Java solver suite was run.
- All 724 native production source files are byte-identical to the captured
  source. The pin, provenance ledger, licenses, Cargo lockfile and dashboard UI
  assets are unchanged. All existing copyright lines are retained. The previous
  4,414-case/2,371-fixture attribution audit is reused because its inputs are
  unchanged; this is not new upstream-method test credit.
- The four approved application problem/manifest files retain their original
  hashes. Historical branch versions match the same approved snapshots.
- The static dashboard builds. Its evidence, counts, dates, ratios, scores and
  original checksums are unchanged; snapshot freshness flags are cleared because
  renamed paths change source fingerprints. Existing desktop/mobile UI evidence
  remains applicable to the unchanged UI assets. No old timing is relabeled.
- Gitleaks history findings match the 64 previously reviewed hash/fingerprint and
  collector-key false positives. The final tree and publication-tail scans are
  recorded in private receipts; no unresolved credential finding is accepted.

Focused correctness evidence binds to application input SHA-256
`4f1b37e056939c21db5aec3076f8e0f83f33ae21248448cff8787fa75ae0a5d9`.
It does not refresh historical throughput qualifications. The two absent
historical diagnostics and other ignored raw-output limitations remain explicitly
recorded in [history preparation](public-history.md#retained-archive-limitations).

## Publication steps still intentionally deferred

1. Obtain the owner's explicit publication/cutover instruction. The hosted
   repository remains private and retains its original refs and Actions history.
   **Do not simply change its visibility.** Use the reviewed sanitized candidate
   for a clean standalone GitHub repository; preserve the original privately.
   A normal force-push is insufficient assurance about retained cached objects.
   See the [hosted cutover](public-history.md#hosted-history-cutover-remains-a-publication-step).
2. Review changes after the captured source and the hosted result before making
   anything public. Prepared metadata: description “Experimental Rust constraint
   optimization solver inspired by Timefold Community”; topics `rust`,
   `constraint-programming`, `optimization`, `scheduling`, `local-search`.
   Apply metadata and update private-status wording during the authorized cutover.
3. Immediately after publication, enable and verify the chosen
   [GitHub private vulnerability reporting route](../SECURITY.md). GitHub makes it
   available for public repositories; it remains inactive during preparation.
4. When hosting is authorized, activate GitHub Pages and set
   `PUBLISH_DASHBOARD=true`. Hosting remains deferred by the owner's instruction;
   the [Pages workflow and README link](github-pages.md) are prepared. Crates remain
   unpublished and `publish = false`.

Apache-2.0 project licensing and existing third-party notices remain in place.
The [legal/provenance guide](legal-and-provenance.md) retains the naming and
attribution boundaries. This pass did not perform trademark clearance or a legal
opinion on the full implementation.

## Validation and handoff

The source snapshot example, pinned-source/provenance checks, fixture hashes,
dependency metadata, Markdown links/anchors, and whitespace/diff review are the
relevant preparation checks. All 95 local links/anchors across the ten entry-point
documents resolved at review. Local receipts include redacted scanner reports,
triage, captured refs, and protected-input hashes. No Rust implementation, test
assertions, dependency versions, pin, fixture bytes, CI configuration, or benchmark
records were changed by this documentation pass. No Rust suite, benchmark matrix,
or browser/dashboard rendering check was repeated.

Repeat only checks whose relevant inputs change. Before any later publication,
refresh the remote visibility, branch, Actions, and final-history delta review;
this dated audit is not a perpetual publication clearance.

## Review references

Subsequent dashboard preparation is recorded in [GitHub Pages hosting](github-pages.md):
the reviewed public snapshot anonymizes application identities and omits raw
execution details. GitHub rejected activation for the private repository's current
plan; the owner chose to defer hosting. The README's Pages link is explicitly a
planned destination. The source repository remains private.

- [GitHub visibility consequences](https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/managing-repository-settings/setting-repository-visibility):
  Actions history and logs become public with the repository.
- [Gitleaks](https://github.com/gitleaks/gitleaks): secret-scanning tool and release source.
- [Apache License 2.0](https://www.apache.org/licenses/LICENSE-2.0): redistribution and attribution terms.
- [Timefold licensing](https://timefold.ai/license) and [trademarks](https://timefold.ai/trademarks):
  Community/Enterprise and branding context.
