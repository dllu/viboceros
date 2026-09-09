# Primitive numerical robustness

[Architecture](architecture.md) · [NURBS numerical policy](nurbs-numerics.md)

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

## Dot and cross products

Vector dot products compensate multiplication and summation rounding. When
products or their sum overflow, or product bits may be lost near the subnormal
range, an allocation-free integer accumulator spans the full binary64 product
range. It rounds only the final sum, nearest with ties to even. Up to six products
fit in 66 limbs; separate positive and negative magnitudes retain small terms
until large terms cancel.

Cross products use compensated two-product determinants with component-local
fallbacks. They do not normalize all components together, which could erase
small components unrelated to a large one. The determinant fallback shares the
exact accumulator.

Evidence includes 10,000 deterministic full-range comparisons against hardware
multiplication and fused multiply-add, independent integer dot/determinant sums,
subnormal ties, exact orthogonality and parallelism, axis permutations, and
genuine result overflow. Six-product tests include 1,000 independent integer
sums and cancellation retaining a smallest-subnormal contribution.

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

## Line interpolation and extrapolation

Line evaluation fuses coordinate scaling and translation, retaining finite
extrapolated results when the offset alone would overflow. Interpolation uses
the nearer endpoint as its origin to reduce endpoint-difference rounding error.
Exact endpoint parameters return the stored endpoints directly.

Power-of-two tests cover both signs, all axes, reversed endpoints, invalid
parameters, true result overflow, and an interior sample that previously rounded
onto an endpoint. These are targeted guarantees, not a claim of correctly rounded
interpolation for every binary64 input.

## Focused checks

```sh
cargo test -p viboceros-geometry vector::
cargo test -p viboceros-geometry line::
cargo test -p viboceros-geometry plane::
```

Run `cargo test --workspace` for downstream geometry, command, drafting, I/O,
document, viewport, and saved oracle-fixture coverage. Native tests and saved
Rhino observations are distinct evidence; the exact binary examples above do
not require, or claim, a fresh Rhino measurement.
