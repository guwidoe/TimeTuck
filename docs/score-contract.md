# Integer score contract

Reference: Timefold Solver Community 2.7.0 at
`8b815bc6b5f3eb81feedc94dea0885a93b267762`. Specifications are the three
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

47 adapted upstream methods pass, preserving all assertions with native equality,
hash consistency, constructors and zero constants. Five original Rust regression
methods cover boundary and malformed-input behavior. An independent line-protocol
adapter compares 5,409 original deterministic generated cases with Java, including
large integers, structural values, nonfinite floating operands, and ordering.

This is selected behavioral compatibility, not all possible input compatibility.
Unicode numeric parsing is not promised. General fractional powers may differ
across Rust's and the JVM's math libraries at rounding boundaries; the differential
random matrix limits powers to integer exponents (the adapted square-root cases
also pass). Bendable and BigDecimal scores remain pending. Initialization/domain
assignment is separate from structural score and is not implemented yet.
