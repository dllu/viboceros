# Exact power-of-two recurrence arithmetic

[Architecture](architecture.md) · [Surface pole recovery](surface-pole-recovery.md)

The guarded curve, UV-curve, and surface evaluators share exact homogeneous
de Boor and derivative-control recurrences in `nurbs/exact`. Binary64 inputs are
exact dyadic rationals: integers divided by powers of two. On many knot spans,
every blend or derivative factor is dyadic too. Reducing general fractions at
every coordinate operation then does unnecessary work.

`nurbs/exact/dyadic.rs` tries the same recurrence with arbitrary-size integer
significands and signed binary exponents. This is exact arithmetic, not double
precision, a fixed-precision approximation, or a tolerance-based filter.

## Invariants and fallback

Each value is `n * 2^e`, with an odd nonzero `BigInt` significand, or canonical
zero with exponent zero. Addition/subtraction aligns exponents exactly and
removes trailing zero bits. Multiplication combines integers and exponents.
Exponent arithmetic and shift-index conversions are checked.

Inputs come from the shared evaluator's reduced, positive-denominator
`BigRational` values. A denominator must be a power of two to enter the
specialization. Knot differences and blend/derivative ratios are initially
formed as general exact rationals, so no floating-point knot subtraction or
division can alter dispatch. A non-dyadic control or factor declines the attempt;
the unchanged general-rational recurrence uses the original inputs.

Normalized results return to `BigRational` without a GCD: odd numerators and
power-of-two denominators are already coprime. Nonnegative exponents become
integers. The final rational projection and quotient-rule jets remain in the
general exact representation; only requested Euclidean outputs round to binary64.
True poles, overflow errors, and interpolated signed-zero coordinates retain
their contracts.

For example, on `[0,1.5]`, native `t=0.5` needs the non-dyadic blend `1/3` and
declines the fast exact recurrence. At `t=0.75`, cancellation in the exact ratio
gives `1/2`, so the same span qualifies. Eligibility concerns exact factors, not
whether the stored knots look like integers or have a particular exponent.

## Validation and limits

Deterministic arithmetic tests cover signed zeros, minimum subnormals, extreme
finite values, and 256 additional binary64 bit patterns. Sums, differences,
products, cancellation, and conversions match separately computed reduced
`BigRational` numerators **and denominators**, checking canonical form as well as
value. Another test checks accepted cancellable knot ratios and rejected thirds.
Independent Python-generated curve, fractional-curve, UV, and surface reference
tables continue to test final bits through the public dispatchers. Surface grids
also replay the in-domain Bernstein reference points independently of scalar
evaluation.

```sh
cargo test -p viboceros-geometry exact_dyadic
cargo test -p viboceros-geometry dyadic_recurrence
cargo test -p viboceros-geometry reference
```

Integer sizes still grow with degree and exponent range. Non-dyadic spans can
incur an unsuccessful specialization attempt before general evaluation, and
exact projection can still be expensive. This is not a constant-cost kernel or
a general performance-parity claim. The [surface audit](surface-pole-recovery.md)
records the motivating workload and its remaining cost.
