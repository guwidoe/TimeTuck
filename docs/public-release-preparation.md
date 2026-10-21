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

## Items to settle before publication

1. **Private vulnerability reporting.** The GitHub reporting endpoint returned
   HTTP 404 for this private repository, so an active public reporting route could
   not be verified. Enable and verify GitHub private vulnerability reporting when
   available, or supply another working private contact. [SECURITY.md](../SECURITY.md)
   describes the current private collaboration route without inventing a contact.
2. **Historical metadata disclosure.** Evidence contains local source paths,
   machine names, and internal development context. Git history has one author
   identity with a direct email address. The reviewed current tree had 51 files
   containing local machine-path patterns. Decide whether those disclosures are
   acceptable before making the existing history public. Deleting current lines
   alone would not remove their historical copies. No history was rewritten and
   no evidence hashes were relabeled during this pass.
3. **Historical artifact availability.** Provenance references
   `work/round138-kopt-seam-certificate.py` and
   `work/AccessTokenCipherRecognition.java`, neither present in the audit checkout.
   They are recorded diagnostic material, not redistributed files. Recover their
   original bytes into an attributed archive if that historical investigation is
   to be fully reproducible; otherwise keep the archival limitation explicit.
   Other retained observations also reference ignored raw outputs that a clone
   does not contain. Existing measurements were not regenerated.
4. **Application licensing clarity.** The RealWorld manifests explicitly permit
   public distribution of the two bundled snapshots. The original notices assert
   no additional license for that application material; decide and document its
   intended reuse terms before promoting it as a freely reusable benchmark dataset.
   Preserve the manifests and their exact bytes.
5. **Publication authorization and metadata.** The owner must explicitly authorize
   a future visibility change. At that point, review the final tree/history/log
   delta, update the temporary private-status wording, and choose repository
   description/topics. A crate release additionally needs the decisions in the
   [roadmap](roadmap.md#before-a-distributable-crate-release).

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

- [GitHub visibility consequences](https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/managing-repository-settings/setting-repository-visibility):
  Actions history and logs become public with the repository.
- [Gitleaks](https://github.com/gitleaks/gitleaks): secret-scanning tool and release source.
- [Apache License 2.0](https://www.apache.org/licenses/LICENSE-2.0): redistribution and attribution terms.
- [Timefold licensing](https://timefold.ai/license) and [trademarks](https://timefold.ai/trademarks):
  Community/Enterprise and branding context.
