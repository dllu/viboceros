# Exact finite-value sums

[Measurement commands](commands/measurements.md) · [Architecture](architecture.md)

`viboceros-geometry::FiniteSum` accumulates finite binary64 values without
rounding intermediate totals. Positive and negative contributions are stored
separately, subtracted at query time, then rounded once to nearest, ties to even.
The accumulator can therefore recover `MAX + MAX - MAX = MAX`, or retain a
single smallest subnormal after enormous contributions cancel. Input order does
not affect the result. Empty sums and exact cancellation return positive zero.

Each magnitude has 34 fixed 64-bit limbs at quantum `2^-1074`. A finite term
occupies at most 2098 bits; another 64 bits cover up to `usize::MAX` terms on
supported targets. Count overflow and non-finite inputs are rejected before
changing state. Storage is fixed and no heap allocation is needed. Querying an
overflowing total also leaves the accumulator usable for later cancellation.

The shared `binary_accumulator` module supplies significand decomposition,
integer addition, signed subtraction, and rounding. The existing exact
three-product dot fallback uses the same code with 66 limbs and quantum
`2^-2148`; its product-scale underflow and tie tests remain separate.

Tests compare sums against hardware-rounded addition across the binary64 range
and independently accumulated integer totals at several binary scales. Explicit
regressions cover subnormals, ties, signed cancellation, overflowing prefixes,
non-finite final results, and rejected additions. The measurement commands use
this accumulator for totals; individual geometry integrals retain their existing
accuracy and failure limits. Other kernel accumulators have not all migrated.
A command regression sums two enormous closed tetrahedra and one reversed copy,
whose finite signed total survives an overflowing positive prefix.

The shared limb arithmetic also has direct structural tests: shifted 128-bit
products are checked against an independent bit-by-bit adder at every word
offset, using random magnitudes and long carry chains. Signed subtraction
recovers a single subnormal across long borrow chains up to the sum accumulator's
high limbs. Both sum and product scales exercise every guard-bit offset, distant
sticky bits, and even/odd tie rounding. These tests supplement the end-to-end
numeric comparisons; they do not change the runtime algorithms.
