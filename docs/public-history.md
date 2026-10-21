# Preparing an anonymized public history

The owner authorized anonymizing the application-origin workloads throughout
Git history on 2026-10-21, while retaining required attribution and keeping the
repository private. The release candidate is prepared separately from the active
development checkout. Its original history is retained in a private Git bundle.
This avoids rewriting another contributor's live work or losing diagnostic data.

## What changes

The application uses the neutral `real_world` name in paths, modules, scripts,
Cargo targets, report keys, branch names and commit messages. Private source
locations and session identifiers become aliases; development machines receive
consistent runner aliases, including archived SSH destinations. Commit authors
retain their names and use the owner's GitHub noreply address. Original source
checksums and measurement dates, scores,
timings and input digests are retained as historical evidence.

The application notice retains Guido Witt-Dörring's copyright and the owner's
Apache-2.0 grant for the specifically bundled files and approved public fixtures.
The remaining private application is outside that grant. Timefold's pin, adapted
test identities, licenses and required notices remain intact. The approved
problem and manifest snapshots retain their original bytes.

Renaming source files changes source and input digests. Existing measurements do
not qualify the renamed source. Do not replace their hashes with new ones or
label them current. The public dashboard snapshot retains its numeric evidence
and export date; its freshness flags are conservatively cleared for this release
candidate. Fresh focused checks are recorded separately from historic timings.

## Reproduce preparation privately

`scripts/prepare_public_history.py` takes a private, untracked JSON policy and a
separately obtained, pinned `git-filter-repo` executable. The policy contains
original identities; never commit it, the original backup, or private receipts.

```sh
python3 scripts/prepare_public_history.py prepare \
  --source . \
  --destination work/release-preparation/public.git \
  --rules work/release-preparation/policy.json \
  --filter-tool work/release-preparation/git-filter-repo \
  --receipts work/release-preparation/receipts
```

The tool refuses an existing destination, captures every source ref in a bundle,
clones that captured graph, checks for ref-name collisions, rewrites the clone,
and audits all stored Git objects and refs. A privacy rule overlapping a copyright
notice stops preparation for an attribution review. The tool does not push or
change visibility. Inspect the result and run checks appropriate to the renamed
entry points before exporting a final candidate.

The separate candidate checkout and private receipts live under ignored
`work/release-preparation/`. Follow the dated [publication review](public-release-preparation.md)
for its captured source and validation. A later development commit needs its own
review and sanitized integration; an earlier audit does not cover it.

## Hosted-history cutover remains a publication step

The existing private GitHub repository still holds the original history and
Actions logs. **Do not make that existing repository public unchanged.** A normal
push of a sanitized tip would leave old branch tips, logs, cached commits and
other historical references exposed.

For this anonymization, the recommended cutover is a clean repository populated
only from the sanitized candidate, with the original repository retained privately.
GitHub's [history-removal documentation](https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/removing-sensitive-data-from-a-repository)
states that force-pushed commits can remain accessible in cached views and that
Support does not remove non-sensitive data. A support purge cannot be assumed
for application-name anonymization. Reusing the existing hosted repository would
require a separately verified way to remove all retained content.

Preserve the original private archive. Review Actions logs, artifacts, releases,
issues, pull requests and repository metadata; remove or replace original hosted
records that disclose private identities. Local filtering alone cannot certify
GitHub's retained objects or caches. Recheck the hosted result before changing
visibility. These remote actions require the owner's later cutover instruction.

Immediately after an authorized publication, enable and verify the selected
[GitHub private vulnerability reporting route](../SECURITY.md). Enable the prepared
[Pages workflow](github-pages.md) only when hosting is authorized. No hosted
repository, site, package or release is published by preparation.

## Retained archive limitations

Two provenance records refer to historical diagnostics whose original bytes are
absent: `work/round138-kopt-seam-certificate.py` and
`work/AccessTokenCipherRecognition.java`. They are not redistributed files.
Other observations reference ignored raw outputs unavailable in a fresh clone.
These limitations remain explicit; preparation does not reconstruct missing
evidence or claim to rerun those investigations.
