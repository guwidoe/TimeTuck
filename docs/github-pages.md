# GitHub Pages performance dashboard

The project site is intended for
[guwidoe.github.io/TimeTuck](https://guwidoe.github.io/TimeTuck/). Hosting the
dashboard is separate from making the source repository public. The repository
remains private. GitHub rejected activation on 2026-10-21 with HTTP 422:
“Your current plan does not support GitHub Pages for this repository.” The owner
chose to keep hosting prepared for later rather than create a separate public
dashboard repository. The URL above is reserved as the intended destination and
is not a live-site claim.

## What the site contains

The site serves the existing dashboard UI with an explicitly reviewed report in
`dashboard/public/report.json`. That public snapshot has generic **Real-world use
cases**, **Use case A**, and **Use case B** labels. It includes the observed
throughput, scores, feasibility, seeds, evidence dates, source hashes, original
status flags, and coverage counts. It omits application identities, local paths,
commands, raw logs, private origin manifests, and large per-state observations.
The download link points to the same sanitized snapshot.

The public page identifies itself as a snapshot. Source freshness is captured at
export and cannot track later source changes on a static host. Native readiness
still requires live verification and is not inferred from a published snapshot.
A focused Rust verification log can leave adapted methods unverified; the export
preserves those counts. Publishing does not run tests, refresh benchmarks, or
turn historical evidence into current evidence.

The local dashboard continues to read `dashboard/data/report.json` and query its
local freshness endpoint. Original records, fixtures, and attribution remain in
the repository; anonymization happens at the public presentation boundary.

## Build or preview the approved snapshot

Python 3 is sufficient. No Java, Rust build, upstream checkout, benchmark, account
credential, or dependency installation is needed:

```sh
python3 scripts/build_dashboard_site.py --output work/pages
python3 -m http.server 8877 --bind 127.0.0.1 --directory work/pages
```

Open [the local static preview](http://localhost:8877/). The build copies only the
HTML, CSS, JavaScript, bundled font and its license, project license, `.nojekyll`,
and public report. It refuses a contaminated output directory instead of including
extra files. Use a dedicated output path.

## Update the published data deliberately

After obtaining the relevant genuine verification or benchmark results, generate
the ordinary local report using the [existing pipeline](dashboard.md). Do not run
the whole pipeline just to publish existing data. Export an explicit snapshot:

```sh
python3 scripts/build_dashboard_site.py \
  --report dashboard/data/report.json \
  --snapshot-output dashboard/public/report.json \
  --output work/pages
python3 -m unittest discover -s scripts -p test_build_dashboard_site.py
```

Review the JSON diff and privacy scan before committing the new snapshot. The
export records the source report's checksum and a separate export timestamp;
original evidence timestamps, numeric results, digests, and statuses are retained.
The source report is never overwritten. Rebuilding an existing approved snapshot
preserves its publication metadata rather than claiming a new measurement.

After changing the UI, verify the static build in desktop and mobile browsers.
Update the script/CSS cache-version hashes in `dashboard/index.html` when those
assets change. The public snapshot need not be remeasured for a presentation-only
change; its live-readiness evidence may appropriately become stale.

## GitHub hosting

The prepared [Pages workflow](../.github/workflows/pages.yml) is disabled by default:
its build job requires the repository Actions variable `PUBLISH_DASHBOARD` to be
exactly `true`. When enabled, it builds the committed public snapshot on relevant
`main` pushes or manual dispatch and uploads only
`work/pages`. The build reads Pages configuration with `pages: read`; its deployment
job uses `pages: write`, `id-token: write`, and the
`github-pages` environment. It has no benchmark or full-test step and cannot read
ignored local report files from a fresh checkout.

When hosting is authorized, enable GitHub Pages with GitHub Actions as its
publishing source and set `PUBLISH_DASHBOARD=true` before the first workflow
deployment. With the source repository private, account eligibility must
support Pages for private repositories. The Pages site can be publicly accessible
even while the source remains private. See [GitHub's workflow and eligibility
documentation](https://docs.github.com/en/pages/getting-started-with-github-pages/using-custom-workflows-with-github-pages).

Keep the source repository private during this preparation. No plan upgrade,
visibility change, or publication of the full repository is part of this hosting
configuration.

## Preparation verification — 2026-10-21

The public projection was exported from the existing report generated at
`2026-10-21T16:47:11.670792+00:00`. It preserves the source report checksum and
all observed pass/unverified counts; no Rust tests or benchmarks were rerun for
publication. The current saved coverage is a focused result with 4,412 adapted
methods unverified, not a fresh full-suite pass. The two application throughput
ratios remain historical: 1.5284327987555946 and 0.8515569311051224.

Three focused publication-boundary tests pass. The approved-snapshot build needs
only Python and the allowlisted files, and the public bundle has zero Gitleaks
findings. Headless Chrome checked desktop 1440×1100 and mobile 390×1000: both
rendered two anonymously labeled application rows, preserved stale status,
exercised search and focus, and had no runtime errors, failed responses, or page
overflow. The public page did not request the local freshness endpoint. The
Impeccable detector reported no findings for the changed UI files. Receipts and
screenshots are retained locally in ignored `work/pages-verification/`.

The Pages workflow and static files are prepared locally. No source branch was
pushed, no separate public repository was created, and no site was published.
