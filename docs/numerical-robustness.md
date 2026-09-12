# Primitive numerical robustness

[Architecture](architecture.md) · [NURBS numerical policy](nurbs-numerics.md)

For mesh-specific normal, area, midpoint, and topology-editing checks, see
[mesh topology and editing](mesh-topology.md).

These policies concern primitive binary64 arithmetic. They do not establish
global Rhino parity, exact arithmetic for every geometry operation, or uniform
accuracy for ill-conditioned inputs. Finite-value and degeneracy validation
remain in force; a representable intermediate is not required when the final
result can be recovered safely.

Implementation: [vectors](../crates/viboceros-geometry/src/vector.rs),
[exact accumulator](../crates/viboceros-geometry/src/vector/exact_dot.rs),
[lines](../crates/viboceros-geometry/src/line.rs),
[planes](../crates/viboceros-geometry/src/plane.rs), and
[frames](../crates/viboceros-geometry/src/frame.rs).

Affine application and composition live in
[transform.rs](../crates/viboceros-geometry/src/transform.rs), with their focused
regression suite in [transform/tests.rs](../crates/viboceros-geometry/src/transform/tests.rs).

## Dot and cross products

Vector dot products compensate multiplication and summation rounding. When
products or their sum overflow, or product bits may be lost near the subnormal
range, an allocation-free integer accumulator spans the full binary64 product
range. It rounds only the final sum, nearest with ties to even. Up to six products
fit in 66 limbs; separate positive and negative magnitudes retain small terms
until large terms cancel.

Scaled frame projections use a separate 99-limb accumulator at quantum
`2^-3222` for products of three binary64 factors. It applies the scale before
rounding, allowing a finite display-space projection even when the unscaled
point displacement or projection overflows. `Distance` uses this path when a
reducing display-unit conversion can recover an overflowing model-unit distance.
An independent 256-case reference uses Python's arbitrary-precision `Fraction`
arithmetic and checks every result bit, including signed zero and infinity.
Cases include full-range operands, exact cancellation, and controlled exponent
sums near normal/subnormal and overflow boundaries. The generator is
`tools/numerics/generate_scaled_dot_reference.py`; it emits the committed
`vector/exact_dot/scaled_reference.txt` fixture without changing files.
Normal Rust tests need no Python runtime or additional arithmetic dependency.
This validates these scalar projections, not global geometry accuracy.

Cross products use compensated two-product determinants with component-local
fallbacks. They do not normalize all components together, which could erase
small components unrelated to a large one. The determinant fallback shares the
exact accumulator.

Evidence includes 10,000 deterministic full-range comparisons against hardware
multiplication and fused multiply-add, independent integer dot/determinant sums,
subnormal ties, exact orthogonality and parallelism, axis permutations, and
genuine result overflow. Six-product tests include 1,000 independent integer
sums and cancellation retaining a smallest-subnormal contribution.

Affine orientation uses an exact determinant-sign predicate in
[`transform/orientation`](../crates/viboceros-geometry/src/transform/orientation.rs).
It accumulates six signed triple products in 99 fixed limbs and compares their
positive/negative magnitudes without rounding. It therefore does not erase
small scale factors through global normalization or confuse determinant
underflow with singularity. Tests cover extreme diagonal scales, exact
singularity, near-cancelling products with determinant one, and 1,000 independent
integer matrices under extreme binary row scaling and row swaps. This is a sign
predicate, not a condition-number estimate or an inversion guarantee.
An exhaustive test additionally covers all 19,683 matrices with entries in
`{-1, 0, 1}`, including singular cases, transposition, row negation, and positive
row scaling by the smallest subnormal and largest finite binary64 values.
Triangular matrices use an exact sign/zero check of the diagonal, avoiding
accumulation and even multiplication. Other matrices skip identically zero
triple products. The exhaustive suite covers these paths. The ignored
`benchmark_diagonal_orientation` test measured 100,000 diagonal queries at
192 ms before and 15.4 ms after this optimization in a development debug build;
this is not a release or Rhino performance comparison.

## Point-difference projections

`Vector3::dot_point_difference` is an internal helper shared by line closest-point,
plane signed-distance, and frame-coordinate calculations. The fast path checks subtraction
residuals before using the compensated dot product. If a relevant displacement
coordinate lost bits, or displacement/projection arithmetic overflows, the
fallback projects the original point coordinates as six signed products.

The helper may return signed infinity. Line closest-point calculations clamp
the normalized projection to the finite segment; plane signed distances reject
an unrepresentable distance. Tests cover overflowing tangential displacement,
finite interior projections, oblique cancellation, opposite normal signs,
reversed point/origin roles, and small origin contributions lost by naive
subtraction, including subnormal results.

Frame conversion checks all three projected coordinates for finiteness. A
rotated frame can have finite local coordinates even when a world displacement
component overflows; an axis-aligned frame with a truly overflowing local
coordinate still returns an error. Frame tests also cover reversed origins and
small origin contributions retained after cancellation.

The reverse frame mapping includes the origin in each compensated/exact
four-term coordinate sum. It can therefore recover a finite world point when
the displacement alone overflows; `vector_at`, which has no origin to cancel
that displacement, still rejects it. Tests compare an extreme-range rotated
frame with a power-of-two-scaled ordinary case and check genuine overflow and
nonfinite input rejection.

Affine point transforms use the same four-term sum, including translation before
rounding the linear result. Exact binary tests cover a doubled coordinate whose
overflow is cancelled by translation, plus normal and subnormal residuals after
large-term cancellation. Vector transforms omit translation and continue to
reject genuinely unrepresentable displacements.
Centered maps still store rounded matrix and translation coefficients. Their
intended fixed points are not guaranteed bit-exact; the directional-scale test
uses a tight binary64 error bound rather than relying on intermediate rounding
to cancel coefficient error.
Centered and origin-mapping constructors compute translation as one
`target - A*source` sum per coordinate, sharing the same compensated/exact
arithmetic. They do not first require `A*source` to be finite. Exact scale tests
cover cancellation of an overflowing mapped center and rejection when the
translation itself is unrepresentable.

`AffineTransform3::then(next)` composes in application order: first `self`, then
`next`. Its linear part uses compensated/exact row-column products, and its
translation uses the checked affine point evaluator. Singular maps are allowed;
unrepresentable final coefficients are errors even if some particular input
points would map to finite values. Integer-matrix tests compare every pair's
point/vector application with sequential evaluation and check triple
associativity without rounding ambiguity. Extreme cases cover overflowing
products that cancel and translated sums that remain finite. In general,
rounded composition is not guaranteed bit-identical to sequential application.
An end-to-end B-rep box test compares complete sequential and composed results
for exact integer maps, including one and two reflections. It checks topology,
unchanged parameter-space trims, propagated component tolerances, face reversal
parity, positive volume, and an unchanged source B-rep.
Mesh transforms instead retain face index order: a reflection reverses signed
volume, and two reflections restore its sign. This agrees with the low-level
`ON_Mesh::Transform` implementation in the vendored
[OpenNURBS source](../third_party/opennurbs/opennurbs_mesh.cpp), which does not
flip mesh faces. A native mesh-box test checks retained indices, signed-volume
parity, and sequential/composed equality. This does not establish every Rhino
UI command's higher-level mesh-orientation policy.
Construction populates the fixed-size matrix directly, without a temporary
heap-allocated flattened array. Composition and direction mapping share one
checked matrix-product helper. Row/column-basis tests protect matrix layout,
and invalid-coefficient tests cover all nine matrix entries.

The maximum-linear-scale bound used for B-rep tolerance propagation rounds
nonnegative normalization, norm sums, products, and square root outward.
Round-to-nearest row sums could otherwise erase a small positive entry and
underestimate the true stretch. Scaled coordinate permutations (including
diagonal maps) retain the exact maximum absolute coefficient. A symmetric
matrix with known norm `1 + 2^-53` checks that the result rounds upward, not to 1.
The final B-rep component-tolerance multiplication also rounds outward. An
integer-significand comparison detects downward rounding even below the
subnormal quantum, where an FMA residual could itself round to zero. Exact
products are retained, including identity scaling and the largest finite result.
Tests cover subnormal underflow and 10,000 mantissa pairs at three binary scales
against independent FMA residuals in their representable range.
Subnormal tests instead use exact integer ceiling in units of the smallest
subnormal, covering every scale `n/256` from zero to one, small bit patterns,
and values adjacent to the normal/subnormal boundary in both operand orders.
This arithmetic and its regression tests live in the independent
[`brep/tolerance` module](../crates/viboceros-geometry/src/brep/tolerance.rs),
separate from topology construction and validation.
The module validates both nonnegative finite operands before zero shortcuts or
multiplication, so invalid scales cannot be hidden by a zero tolerance or by
two negative operands producing a positive product.

## Line interpolation and extrapolation

Line evaluation fuses coordinate scaling and translation, retaining finite
extrapolated results when the offset alone would overflow. Interpolation uses
the nearer endpoint as its origin to reduce endpoint-difference rounding error.
Exact endpoint parameters return the stored endpoints directly.

Power-of-two tests cover both signs, all axes, reversed endpoints, invalid
parameters, true result overflow, and an interior sample that previously rounded
onto an endpoint. These are targeted guarantees, not a claim of correctly rounded
interpolation for every binary64 input.

## Point-to-point directions

`Point3::direction_to` normalizes the difference between distinct finite points.
When subtraction overflows, it halves both endpoints before subtracting; this
retains the direction without requiring the displacement or distance to fit in
binary64. Ordinary and subnormal displacements use the unscaled path. Tests cover
opposite extreme coordinates, reversal, coincident-point rejection, and small
representable components alongside an overflowing dominant displacement.
Four-point `Angle` and its interactive validation use this query. These extreme
cases are analytic numerical regressions, not additional Rhino oracle captures.

## Focused checks

```sh
cargo test -p viboceros-geometry point::
cargo test -p viboceros-geometry vector::
cargo test -p viboceros-geometry line::
cargo test -p viboceros-geometry plane::
```

Run `cargo test --workspace` for downstream geometry, command, drafting, I/O,
document, viewport, and saved oracle-fixture coverage. Native tests and saved
Rhino observations are distinct evidence; the exact binary examples above do
not require, or claim, a fresh Rhino measurement.
