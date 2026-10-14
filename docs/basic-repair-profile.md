# Bounded phased basic repair example

`basic_repair_profile` runs two actual native repair phases on a homogeneous,
required, unpinned weighted graph-coloring problem. The first uses independent
sampling and sequential reconstruction; the second uses coverage-cycle sampling
and cheapest insertion. Each phase begins from the retained best assignment.
The executable checks final validity, full score and retention of the first
phase's score, and verifies its finite phase/shared work limits.

It accepts the existing development solver protocol on standard input:
case ID, domain size, comma-separated initial values, pin bits, comma-separated
edge pairs, then flattened weights; fields are tab separated. Initial values
must all be zero and have each variable's lowest weight.

```bash
printf 'profile\t3\t0,0\t00\t0:1\t-12,-1,-4,-12,-7,0\n' | \
  cargo run --locked --example basic_repair_profile -- 2 8 0 1
```

Arguments are ruined-group size, attempts per phase, warmups and output samples.
Eight attempts per phase means sixteen total repairs. Output retains the existing
seven tab-separated fields: case ID, elapsed nanoseconds, hard score, soft score,
assigned values, logical score evaluations and total repairs. Debug-mode elapsed
values are diagnostics; performance comparisons use the dedicated benchmark.

The original 64-case suite qualifies both Dense and Stream: 48 optima, 21 improved
scores and 43 equal scores versus the eight-attempt baseline, with no regressions
in these fixtures. It does not establish a general optimum or equal-work speedup.
The single-phase cheapest-insertion benchmark separately records its higher
scoring work, two quality regressions, and slower large-instance timing.
