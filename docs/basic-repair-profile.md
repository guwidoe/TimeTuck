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

Arguments are ruined-group size, attempts per phase, warmups and output samples,
followed by optional `full` (default) or `incremental` scoring.
For example, append `incremental` to the command above to use separate scoped
edge and variable-weight constraints instead of one full-score callback. Both
models retain the same candidate order and logical evaluation budgets. The
incremental model updates affected contributions through the Dense director;
it does not reduce the number of pooled candidate evaluations.
Eight attempts per phase means sixteen total repairs. Output retains the existing
seven tab-separated fields: case ID, elapsed nanoseconds, hard score, soft score,
assigned values, logical score evaluations and total repairs. Debug-mode elapsed
values are diagnostics; performance comparisons use the dedicated benchmark.

The original 64-case suite qualifies both Dense and Stream: 48 optima, 21 improved
scores and 43 equal scores versus the eight-attempt baseline, with no regressions
in these fixtures. It does not establish a general optimum or equal-work speedup.
The single-phase cheapest-insertion benchmark separately records its higher
scoring work, two quality regressions, and slower large-instance timing.
New original qualification preserves all 64 phased assignments, scores, logical
work and retained-best updates across full and incremental Dense scoring. Both
actual executable modes are also independently checked on all 64 inputs. The
six-field required-variable protocol remains the scope of this scoring helper.

The dedicated comparison can measure the incremental mode with a fresh native
full-scoring baseline and Java's existing full EasyScoreCalculator:

```bash
python3 scripts/benchmark_repair.py --ruin-sampling coverage-cycle \
  --construction-placement cheapest --native-scoring incremental
```

The report retains all three runtimes' samples and rejects differences between
native models in assignments, scores, logical evaluation counts or repair counts.
The Java comparison uses different reconstruction and scoring architectures;
its ratio does not establish general solver speed or identical work.
