# Rational surface evaluation

[Architecture](architecture.md) · [Curve limits](curve-sided-evaluation.md) · [Oracle](oracle.md)

`NurbsSurface` evaluates native U/V parameters in the dedicated
`nurbs_surface/evaluate` module. `SurfaceJet2` contains the point and five exact
partial derivatives: `derivative_u`, `derivative_v`, `derivative_uu`,
`derivative_uv`, and `derivative_vv`. The mixed partial is d²S/(du dv), with no
factorial scaling. These are analytic rational derivatives, not finite differences.

`evaluate_with_second_derivatives(u, v)` returns the complete jet.
`evaluate_with_derivatives` and `evaluate` compute only the requested order:
an unrepresentable higher derivative does not prevent a finite point query.
Reversal negates the corresponding first partial and the mixed partial;
pure second partials retain their sign. Changing a parameter domain scales
each derivative by the appropriate first or second power of its parameter map.

## Limits and continuation

The `_on_sides` variants accept independent `ParameterSide::Left` or `Right`
choices in U and V. This enum is shared with curves. Ordinary methods select
right/right; at either outer domain endpoint both choices use the only interior
span. Interior full-order knots can have different point limits, not merely
different derivatives. Selection does not perturb the requested parameter,
insert knots, or allocate a trimmed surface.

The shared curve/surface span selector skips empty endpoint spans, including
equal-knot groups that straddle the active-domain indices. It also handles
unclamped and periodic inputs without treating their outer controls as endpoints.

Strict evaluation rejects nonfinite or out-of-domain parameters. The
`evaluate_extended`, `evaluate_extended_with_derivatives`, and
`evaluate_extended_with_second_derivatives` methods instead continue the nearest
nonempty boundary span. They do not clamp the parameter or wrap a periodic
surface. Continuation uses unbounded de Boor blend factors; the earlier surface
implementation inadvertently clamped those factors. A rational denominator
that evaluates to zero remains an error.

## Numerical policy

The active control rectangle is translated to a local origin and its weights
are divided by their largest absolute value before homogeneous evaluation.
First and second derivative nets are obtained by exact knot divided differences.
For homogeneous numerator A and denominator W, the quotient rules include:

```text
S_u  = (A_u  - S W_u) / W
S_uu = (A_uu - S W_uu - 2 S_u W_u) / W
S_uv = (A_uv - S W_uv - S_u W_v - S_v W_u) / W
```

V derivatives are analogous. The local point is used throughout these rules;
world translation is restored only afterward. In particular, degree-one
homogeneous pure second derivatives vanish, but rational Euclidean second
derivatives generally do not. Interpolated corners return their original control
point exactly. Constant rational patches have exact zero partials at regular
parameters, including the tested large world offsets.

If subtracting the local origin would overflow, evaluation uses world coordinates.
A signed-weight patch can also leave its control hull: a nonfinite local result
is retried without centering when the world-space answer may be representable.
This is not arbitrary-precision arithmetic. Extreme *relative* weights in one
active rectangle can still underflow during normalization, derivative intermediate
values can exceed `f64`, and ill-conditioned denominators amplify error. The
[curve numerical limitations](nurbs-numerics.md) apply here too. Singular-surface
limiting normals and general curvature analysis are not implemented by this change.

## Validation

Twelve geometry regressions cover analytic polynomial and rational partials,
all four limits at crossed full-order knot lines, degree-multiple joins,
unclamped/periodic endpoints, domain chain rules, huge translations, positive
and negative global weight scales, continuation poles, and finite point queries
whose derivatives overflow. Two oracle tests check permanent fixtures and exact
translation invariance; two Python tests check quadrant preparation, derivative
ordering, and disposal on failure.

`surface_jets.json` contains 20 independent Rhino comparisons with 430 sampled
jets. The worker uses the public [Surface.Evaluate API](https://developer.rhino3d.com/api/rhinocommon/rhino.geometry.surface/evaluate)
and its documented derivative order `{Su, Sv, Suu, Suv, Svv}`. Explicit side
requests prepare exact trimmed rectangles before timing, since this API has no
quadrant parameter. Ordinary samples evaluate the original surface directly.
All 20 passed in Rhino 8.32 under Wine/FEX at absolute `2e-12`, relative `1e-12`;
the largest absolute component difference was `4.55e-12` on continuation of a
nonuniform rational patch. Internal full-order positional jumps remain native
analytic tests, not single-NURBS Rhino input fixtures.

The broader checkpoint passes 1,277 Rust tests, 16 Python tests, strict Clippy,
and 115 native fixtures containing 1,180 operations. The 369 previously recorded
curve comparisons still pass at their documented epsilons; six fresh trimmed
mass-property comparisons and the surface-array command probe also pass.
At that checkpoint, a fresh `OrientOnSrf` comparison exposed fixed-cubic line
fitting and a control-net-only probe. The subsequent [curve-morphing work](curve-morphing.md)
replaces both and documents direct-map agreement separately from Rhino's fit error.

```sh
tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/surface_jets.json \
  --absolute-epsilon 2e-12 --relative-epsilon 1e-12
```

`surface_translated_jets.json` is a separate diagnostic, not a passing reference
at that epsilon. At offset `(1e12, -2e12, 3e12)`, native jets for its rational
plane and constant patch retain bit-identical partials. Rhino's plane differed
by up to `1.23e-4` in position, `1.27e-4` in first partials, and `2.04e-4` in
second partials; its constant patch had false partials up to `9.77e-4`.

The sampled native release path took roughly `0.27–1.1 µs` per jet in this run,
versus approximately `2.9–23 µs` through Rhino's Python/.NET API. Construction,
quadrant trimming, and JSON conversion are outside the timed loops. These are
small, host-dependent API benchmarks including allocation and interop overhead,
not a claim about native Rhino kernel performance or continuous error bounds.
