# Integer score contract

Reference: Timefold Solver Community 2.7.0 at
`8b815bc6b5f3eb81feedc94dea0885a93b267762`. Specifications are the four
`core/api/score/*ScoreTest.java` families listed in the provenance ledger, the
pinned documentation's score overview, and observations of the public API in the
released Maven artifact. No production source was translated.

## Implemented types

`SimpleScore`, `HardSoftScore`, and `HardMediumSoftScore` contain signed 64-bit
levels and a structural component. Greater is better. Order is lexicographic,
structural first, then each ordinary level. Simple has no hard constraints;
HardSoft and HardMediumSoft require their first ordinary level to be nonnegative.
A negative structural component makes every type infeasible.

The native Rust API uses immutable values, `new`, `with_structural`, `FromStr`,
`Display`, ordering/equality/hash traits, `+`, `-`, unary `-`, `multiply`, `divide`,
`power`, `abs`, and checked integer arithmetic. Java naming/ABI parity is not a goal.

## Structural behavior observed in the Java oracle

- Constructors and parsers retain the structural value verbatim, including positive
  values. Nonnegative structural values are feasible subject to hard levels.
- Arithmetic clears the structural component to zero, including unary operations.
  It does not add or otherwise transform structural components.
- Full strings include a structural prefix only when negative. Short strings
  include it whenever nonzero. This makes positive structural values lossy through
  full string formatting; normal structurally sound scores use zero.
- The short form drops zero ordinary levels, and is generally not parseable.

These details were not all covered by the initial adapted tests. The Java oracle
caught incorrect early assumptions before this implementation was accepted.

## Arithmetic and parsing

Integer addition, subtraction, negation and absolute value use explicit wrapping
semantics in debug and release builds. In particular, negation and absolute value
of `i64::MIN` produce `i64::MIN`. Checked addition/subtraction/integer multiplication
return `None` on ordinary-level overflow. Both arithmetic paths clear structural
values. Floating operations convert each ordinary level to `f64`, compute, floor,
and saturate on integer conversion; NaN becomes zero. Floating conversion can
lose precision above 2^53. Use checked integer multiplication for exact weights.

Full parsers require every ordinary level in order, accept signed ASCII integers
or `*` (meaning `i64::MIN`), and reject out-of-range integers and whitespace.
Examples: `-147`, `-147hard/-258soft`, `-1structural/0hard/0medium/-5soft`.

## Verification boundaries

74 adapted upstream score methods pass, preserving all assertions with native equality,
hash consistency, constructors and zero constants. Twelve original Rust regression
methods cover boundary and malformed-input behavior plus BendableScore integration. An independent line-protocol
adapter compares 9,959 original deterministic generated cases with Java, including
large integers, structural values, nonfinite floating operands, and ordering.

This is selected behavioral compatibility, not all possible input compatibility.
Unicode numeric parsing is not promised. General fractional powers may differ
across Rust's and the JVM's math libraries at rounding boundaries; the differential
random matrix limits powers to integer exponents (the adapted square-root cases
also pass). Exact decimal scores remain pending. Initialization/domain assignment
is separate from structural score; see `planning-contract.md`.


## Configurable hard and soft levels

`BendableScore<const H: usize, const S: usize>` stores `[i64; H]` hard levels and
`[i64; S]` soft levels, plus structural state. Dimensions belong to the type;
`BendableScore<2, 3>` is a different type from `BendableScore<1, 4>`. A compile-fail
documentation test verifies that incompatible shapes cannot be added. This agrees
with the pinned public documentation's fixed dimensions during solving, while
avoiding runtime shape mismatches in Rust. Dynamic resizing and Java array-mutation
semantics are not supported. Empty hard groups, soft groups or both are valid.

Order is structural, then every hard level in order, then every soft level in
order. Feasibility requires structural and **all** hard levels to be nonnegative;
a positive first hard level does not excuse a negative second hard level.
`hard_cmp` compares only the complete hard group. Constructors and accessors copy
fixed arrays. `of_hard`, `of_soft` and combined `level` access return `Option` for
out-of-range indices. Arithmetic and checked operations follow the existing
integer families, including wrapping, flooring and structural clearing.

Full text uses bracketed groups, for example `[0/-2]hard/[-7/0/3]soft`, and must
contain exactly the dimensions in the target Rust type. Empty groups print `[]`.
Short text omits an entirely zero group but retains zeros inside a nonzero group.
As with the fixed families, full text omits a positive structural prefix, while
short text retains it. Parsing supports the `*` minimum-integer marker.

All 27 methods of the pinned `BendableScoreTest` are adapted, preserving every
assertion and value. The separate late-acceptance BendableScore test preserves all
nine history-slot assertions: soft changes update the current slot; a new hard
best resets the complete history. Seven original regressions cover arithmetic
limits, malformed text, longer shapes, dimensions/index bounds, multi-hard
feasibility, score-cache commit/undo, construction and local search checked against
an independently calculated exact optimum, and annealing probability calculations.

The Java score oracle now covers five Bendable shapes: (1,2), (2,3), (0,0), (0,2)
and (2,0). It uses public score APIs plus explicit dimension checks matching the
Rust target type. The 9,959-case matrix contains 9,929 valid operations and 30
expected malformed-input rejections. Java runtime parse exceptions normalize to
`ERROR`; both adapters returning `ERROR` for a valid case now fails validation.
Exception-class equality is not claimed. These are correctness checks, not new
BendableScore timing benchmarks; existing timing charts still cover three fixed
integer families.

`Score::ordinary_levels()` returns an iterator of its associated Level values
instead of a borrowed contiguous slice, allowing fixed, split-array and owned
storage. Integer families yield i64 without allocation. Consumers must preserve
the declared level order. Construction, incremental scoring, exact search and
acceptance policies use the common traits.

## Hard-level metadata and progress

`Score` now requires `HARD_LEVEL_COUNT`, the number of leading ordinary levels
that represent hard constraints. It is 0 for SimpleScore, 1 for HardSoftScore and
HardMediumSoftScore, and H for BendableScore<H,S>. Custom native score implementations
must supply this constant consistently with their ordinary levels and feasibility.
`ScoreTargetProgress` and `FeasibilityProgress` provide validated weighted progress
estimates with exact stopping predicates; their contract and source-test mapping
are documented in `incremental-search-contract.md`. Gradient differences widen
integers before conversion to floating point; ordinary score arithmetic retains
its existing wrapping semantics.

## Owned score values

The common `Score` bound is now `Clone + Ord + Debug`; it no longer requires Copy.
`structural`, `hard_cmp` and `is_feasible` borrow their receivers, and `hard_cmp`
also borrows its other score. Custom implementations must update those signatures.
The four existing integer families remain Copy and retain their arithmetic and
ordering contracts. Custom scores now declare an associated Level implementing
ScoreLevel; the three fixed decimal families are described below.

Caches, saved candidates, result accessors, acceptor history and phase plans clone
scores where an independent retained value is needed. Read-only comparisons borrow
where possible. AdditiveScore still supplies an owned ZERO constant and consuming
arithmetic. Custom clones must preserve immutable logical values; neither cloning,
comparison nor arithmetic may mutate another retained score through shared interior
state. Result accessors that return owned scores now require Clone instead of Copy.
Acceptance validation/start borrow their configuration so a phase can retain it.

Graph checkpoints capture an owned total before mutation. Rollback moves that
saved total back with mem::replace rather than cloning during Drop. Ordinary clone
failures while preparing/applying a trial therefore unwind through the existing
journal. Scoped move caches remain unpublished until commit; a clone failure during
candidate preparation drops the move guard and restores the assignment. This does
not make arbitrary user destructors or shared interior mutations transactional.

Six original regressions use an actual heap-owned two-level integer score with no
Copy implementation. They compare 720 configurations against HardSoftScore across
all six solve entry points, two construction orders, initial/empty assignments,
five acceptance policies, count caps, saved candidates and two improvement phases.
Exact search is checked across all three model types and budgets 0..130, including
structural rejection and prefix optimality. Additional checks cover late-acceptance
hard resets, annealing probabilities, target/feasibility progress, every injected
score-clone failure through graph and scoped-move preparation, and rollback with
the very next clone configured to fail. A public doctest shows a custom owned score
used by exact search. These checks add no upstream test-method credit and do not
claim arbitrary-precision or decimal compatibility yet.


## Numeric levels

`Score` requires `type Level: ScoreLevel`, and `ordinary_levels()` yields that type.
Existing integer implementations declare Level = i64. The public ScoreLevel trait
supplies numeric zero/comparison, bounded progress fractions and cooled annealing
losses; i64 and Decimal implement it. Decimal's representation-sensitive Eq/Ord
remain unchanged, while score-level comparisons use numeric_cmp. A custom decimal
score must independently implement numeric score ordering/equality, hard comparison
and feasibility; deriving these from Decimal's representation ordering is wrong.

Integer progress and annealing preserve the previous i128 widening and floating
operation order. Decimal differences are computed exactly when cancellation can
matter, then approximated with eighteen leading coefficient digits and a separate
decimal exponent. If the scale gap exceeds the larger coefficient's digit count
plus 32, the smaller operand contributes less than 10^-32 of the larger and can be
omitted from an estimate without allocating scale-gap padding. Zero operands also
avoid padding. Near floating-point limits, split powers retain representable
subnormal/finite ratios. These are bounded floating estimates, not a guarantee of
correct rounding or Java decimal gradient/annealing equivalence. Target ordering,
feasibility, score goals and retained-best comparisons remain exact.

Five original numeric integration methods check 42,875 fixed-point combinations
against independent i128 rational arithmetic, varied cooling, 1,000-digit common
offsets, differently represented equal values, i32-extreme scales, subnormal ratios,
validation and exact-versus-estimated predicates. An additional original matrix
compares 2,160 decimal solver configurations with scaled integer equivalents across
all six model/backend entry points, five acceptance policies, budgets, construction
orders and starts. It uses scales -1000, 3 and 1000 and independently rescales
returned levels with BigInt arithmetic. A doctest demonstrates tiny-scale progress.
These original numeric-level checks add no upstream method credit. The fixed
decimal families below now have their own adaptations; Bendable remains pending.


## Fixed decimal score families

SimpleDecimalScore, HardSoftDecimalScore and HardMediumSoftDecimalScore are owned,
arbitrary-precision native scores. They use Decimal levels and lexicographic
structural/ordinary ordering, with numeric equality and hashing across scale.
The latter does not change Decimal's representation-sensitive equality/hash.
HardSoft/HardMediumSoft feasibility uses structural and hard levels only; medium
and soft values do not affect feasibility. Each family implements Score and
AdditiveScore, with ZERO, new, from_parts, with_structural, level accessors,
ordinary_levels, is_zero, abs, addition/subtraction/negation, parsing/full/short text
and scale-directed multiply/divide/power. Hard/medium/soft factory helpers supply
zero for other levels.

new and parse without a structural prefix normalize an all-zero score to ZERO;
otherwise all scales are preserved. from_parts and parse with an explicit prefix
preserve the supplied scales, even for structural zero. Structural '*' means
I64_MIN. Full text prefixes negative structural state, while short text includes
any nonzero structural value; this follows the existing integer score contract.
Consequently positive structural values do not round-trip through full text.
Short form omits numeric zero levels and is generally not parseable.

Decimal now parses ASCII decimal/scientific notation with an optional sign and
an i32 resulting scale; invalid/nonfinite input is rejected. Its existing Display
(coefficient followed by e and exponent) remains unchanged for collectors.
to_standard_string, used by score Display, preserves scale and uses plain notation
when scale is nonnegative and adjusted exponent >= -6, scientific notation
otherwise. Parsing and formatting extreme scales need no scale-gap padding.

multiply/divide take an exact &Decimal factor; power takes u32. They return Result,
floor each level at max(original scale, 0), clear structural state and normalize
all-zero outputs. They do not convert through f64 or implement Java's floating
input conversion/fractional exponent truncation. Division by zero is an error.
Scale adjustments beyond u32 power-of-ten support report ScaleOverflow. Coefficients
and ordinary arithmetic may otherwise grow without a fixed memory/time bound;
callers must bound problem inputs. Negative sub-quantum results floor to -1 unit;
positive ones floor to zero. abs retains input scales before all-zero normalization.
Negation of a numerically zero, structurally sound score returns its representation;
other negations subtract from ZERO, raising negative scales to zero.

All 19 SimpleBigDecimalScoreTest, 20 HardSoftBigDecimalScoreTest and 21
HardMediumSoftBigDecimalScoreTest methods retain their assertions. Their double
factor literals map to exact native decimal factors and their integral double
powers to u32. scripts/adapt_decimal_score_tests.py reads only those pinned test
files and records every generated method in the provenance ledger. Shared Rust
helpers preserve pairwise equality/hash/order and feasibility assertions.

Six original regressions additionally check parsing/formatting bounds, raw/factory
zero representations, numeric hashing beyond i32 normalization scale, exact large
arithmetic, floor/zero-divisor behavior, observed negation scale rules and fractional
ordering in exact search. The 2,160-case six-backend decimal solver matrix now uses
the public HardSoftDecimalScore instead of its custom fixture. The independent
numeric-level tests retain their separate custom score fixture.

Original generated comparisons (seed 20261212) exercise 3,390 cases, including
86 expected errors, through public Community and Rust adapters plus independent
Python Fraction arithmetic. They compare coefficient/scale, full/short text,
structural state, feasibility, ordering and equality. Selected finite Java factors
(-2, 0, 0.5, 1.2, 2, 5, 1.25) correspond to native exact decimals; powers are 0..4.
Thirty malformed decimal cases and zero-divisor cases must fail explicitly;
unexpected matching errors cannot pass. Results appear separately in the dashboard
under score comparisons and add no source-method credit. Timings remain integer
workloads. Bendable decimal scores, general f64 input behavior and decimal-specific
performance comparisons are not established by this increment.


Both decimal gradient methods in BestScoreTerminationTest now preserve all six
observations each, including exact 0.6 assertions, original 0.00001 tolerance for
one-third and the HardSoft 0.75 split weight. ScoreTargetProgress replaces mocked
score definitions and termination construction. The numeric estimate folds a
negative decimal exponent into a finite denominator to avoid the extra rounding
of (6/1)*0.1 versus 6/10; exponent-separated fallback still handles extreme values.
This is two additional upstream method adaptations, not whole lifecycle parity.
