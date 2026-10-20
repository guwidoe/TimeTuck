# TimeTuck implementation handoff

## Objective and boundaries

Build an independent Rust constraint optimization solver using pinned Timefold
Community tests/public APIs as specifications, with a visual compatibility and
performance dashboard. Objective incomplete; continue closing the remaining compatibility gaps.
Keep pursuing the full objective; this batch is not completion. Never copy or translate
upstream production; Community only, no formal clean-room claim. Upstream pin
remains v2.7.0, `8b815bc6b5f3eb81feedc94dea0885a93b267762`.

Record complete adapted tests/fixtures in `compat/provenance.json`. Partial,
original, skipped or empty tests earn no upstream-method credit. Crates remain
unpublished. Conventional Commits, never amend/rebase/push. The current user direction is to finish integrating completed work, then close
compatibility gaps so the pending backlog shrinks. Keep performance important,
commit occasionally and reuse unchanged verification. Use up to three concurrent GPT-6.1 Sol implementation agents and distribute
builds/tests across available machines. The latest user instruction is to leave
runner-c alone for now; use runner-a and local, with no runner-c probes or wake/auth requests.

## Latest qualification — 292 explicit binary64 Great Deluge

`GreatDelugeRise::Binary64Ratio` adds an explicit floating accumulation policy;
exact `Ratio` and configured defaults remain unchanged. Integer initial water
must have scale zero and fit i64. Separate operand casts, cumulative ratio,
absolute-base multiplication, base addition and floor define the operation order.
Wide finite thresholds decode exactly to BigInt without i64 saturation. Initial
conversion may lower the first threshold; later thresholds can stagnate. A
u64-bounded rise count keeps operations finite and freezes at MAX. All six
basic/list/mixed scoped/stream phase paths use the existing shared acceptor.

Actual Java and independent Python comparisons agree in **276/276 original
cases**, **25/25 boundary prefixes**, and **184/184 separate additional ratio/budget
probes**. Complete exact-default controls remain **275/276** and **23/25 prefixes**;
their full Java/native outputs and exact references match prior actual outputs.
No fixtures, budgets, order, pins, quality checks or old differences were removed.
No Java internal thresholds/traces, general arithmetic/overflow/decimal equivalence
or additional upstream-method credit is claimed. The explicit policy closes the
observed boundary; the original exact arithmetic contract was not a native bug.

Final `bash scripts/check.sh`: **8,574 Rust +292 doctests**,
**107 RUN/41 REUSE**, 856.146s, source
`838a5d9408978312d16a475f30512b14b8bf42a4164ece84aaf76f87f9e64ad3`. Four original arithmetic regressions,
a private counter-saturation boundary and expanded six-engine phase coverage are
included. Actual desktop/mobile browser checks cover selected and prior exact
presentations plus current source/model/data checks; the UI detector is clear.

Only solver-differential was refreshed. All other 17 raw reports retain bytes,
dates and original keys; mixed 291 and stream 289 are now stale. Inventory stays
4410/4855, 10 pending, 435 excluded, no failed/unverified. The dashboard index is
**85.448%**, **1/18 current**, Java multiplier **1**. It measures tracked targets,
not remaining effort or an ETA. Larger mixed Stream parity and recorded range,
list-metadata and Java-error gaps remain. Goal incomplete. All execution was local;
runner-c remains parked, no new remote authentication attempt or new subagent was made.

Evidence: work/round292-qualification/{integration,solver-differential-receipt,
gate-receipt,collection,browser-proof,current-state}.json, retained complete
controls and work/round292-deluge-review/{README,FOLLOW-UP}.md;
work/parallel-round292-evidence.json.

Next: work/round293-registry-choice/README.md contains an unexecuted explicit
EqOnly/default versus HashIndexed client overlay and complete-control driver.
Review its hashes before integration; preserve canonical reports during trials,
original inputs/full observations and cold-mode construction policy. No gain or
qualification is established by that source-only bundle. For local shared changes,
use the already-proven eight-worker build-only precompile on frozen source while
keeping verification environment/cache settings at four. Do not repeat successful
unchanged comparisons or import/rekey caches.

## Remaining limits and retained evidence

- Ten pending upstream methods are eight JVM/loading/framework seams and two
  private k-opt descriptor-array assertions, not ten missing native algorithms.
  Original methods/denominators stay visible; no reclassification for credit.
- Connected-range agreement remains 66/197 under the explicit wrapping metric;
  native/reference checks pass all 197. The 131 differences include Java history
  bugs. `compat/observations/connected-range-history.json` retains the contradictory
  histories; the user policy question remains unanswered. Do not infer a waiver.
- List metadata retains 425/514 agreements: 16 differences, 40 Java errors and
  33 unsupported empty cases. Prior public probes found no justified native fix;
  retain their complete observations rather than repeating the same audit.
- Cyclic-supplier checks retain 107/128 agreements and 21 actual Java errors.
  Native-reference passes never count Java failures as successful agreement.
- Great Deluge's explicit mode is qualified above; exact default stays 275/276.
  Encounter-order ranking has 4824/4824 agreements; total-order controls retain
  4499 agreements/325 differences. Load-balance binary64/wrapping policy has
  101/101 last-observed agreements, exact-root control 100 and default 97; it is
  stale now. Do not repeat unchanged successful investigations for a new turn.
- Mixed 291 (now stale) retained 6/8 Java-time parity with equal quality. Larger
  warm Stream was 22.042ms/Java17.253ms, cold23.970/19.124ms. No general speed or
  memory claim. Arena 291's modest larger warm gain and slower small controls,
  rejected inlining/in-place tuning and invalid Cargo-reuse attempt retain their
  distinct actual source/artifact evidence in work/round291-qualification/.
- Source restoration must write bytes or advance mtime; never copy old Rust
  source mtimes or target fingerprints/binaries into a supposed new build.
  Verify actual compiler artifacts before performance comparisons.

Use docs/completion.md for requirements, docs/incremental-search-contract.md and
other native contracts for behavior, and docs/compatibility.md for qualification
history. The long previous handoff is recoverable with
`git show a8f40aa:docs/next-session.md`; it was shortened without deleting history.
The local dashboard remains http://localhost:8765/. Do not probe/wake/authenticate
runner-c. runner-a's last bounded check requested reauthentication then timed out; no
remote jobs remain live and no expired authentication link is reusable.
