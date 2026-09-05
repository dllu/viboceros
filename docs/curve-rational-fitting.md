# Rational candidates for nonlinear curve fits

[Curve morphing](curve-morphing.md) · [Surface candidates](surface-rational-fitting.md) · [3DM validation](brep-3dm-interchange.md)

For a rational source of degree at most three, the curve fitter checks two
bounded candidates before its adaptive polynomial cubic fallback:

1. Map the original Euclidean controls, keeping their weights and knots.
   This preserves affine images without refitting. Mapping an off-curve control
   may fail even when the source curve's image exists; that failure skips this
   candidate. Direct source-point failures still propagate.
2. Interpolate a rational composition space with denominator W³ and degree 3p,
   where the source is N/W of degree p. A polynomial XYZ map of total degree
   at most three fits in this space. The implementation does not assume or
   symbolically identify such a map: inaccurate candidates are rejected.

Both checks use the existing independent uniform and cosine-spaced validation
stations on every native source span, including one-sided limits. Mapped controls
must meet the requested absolute tolerance; the composition candidate must meet
80% of it, like adaptive refinement. Polynomial inputs keep their adaptive path.
Finite sampling is not a continuous error certificate for a black-box mapping.

## Shared numerical policy

Curves and surfaces share `morph/denominator` and `morph/interpolation/axis`.
The composition space retains native domains and source knots, increasing
interior multiplicities from m to `3p - p + m`. Full-order positional jumps have
independent endpoint rows. Periodic/unclamped sources can become clamped while
retaining their active-domain image. The degree ceiling is nine and the curve
control ceiling remains 512; oversized spaces are skipped before candidate maps.

Nonconstant source weights must have one common sign. They are normalized by
their maximum absolute magnitude before cubing the scalar weight function.
Mixed signs, constant weights, underflowed normalized weights/cubes and unsupported
degrees skip the composition candidate. The original-net check can still handle
a mixed-sign affine image, subject to direct source evaluation and validation.

Interpolation solves centered homogeneous XYZ/W right-hand sides. Cubic axes
use the existing banded solver; higher degrees use bounded full-pivot faer
solves. Solutions must be finite, reconstructed weights strictly positive, and
controls finite. Numerical candidate failure falls back; source mapping failures
do not retry. The native-parameter/side cache and adaptive error tests are shared
by all curve-fitting paths.

The review also corrected the surface original-net validation path: it previously
swallowed an actual source-map error before retrying adaptive fitting. A regression
reproduces two failed evaluations before the fix and requires immediate propagation
afterwards. Off-surface control-map failures still allow a valid surface fit.

## Reproduced limit and validation

The lift `(x, y, z + x² + xy/4 + y³)` of a radius-0.4 rational XY circle formerly
exhausted 512 polynomial controls at `2.5e-12` tolerance, with measured error
`8.81e-10`. The rational fit passes using fewer than 64 controls and 1,000 point
maps; a separate 1,025-point offset grid checks the same requested tolerance.
Tests also cover degrees one through three, common weight signs/scales, W³
reproduction, full-order sided derivatives, native and unclamped domains,
off-curve control failures, exact large constant targets, resource/underflow
rejection, and a non-cubic wave that must use adaptive fitting.

The cubically lifted sphere's shared edge previously failed at B-rep tolerance
`1e-11`, with 512 controls and error `2.21e-11` against its component allocation
of `2.5e-12`. It now uses 13 degree-six controls; the unchanged 25×13 rational
surface candidate completes the B-rep. Its 3DM fixture now requires `1e-11`.
Serialization retains its independent `2e-12` absolute / `1e-14` relative bound.

`curve_rational_morph.json` adds seven public `SplopSpaceMorph` cases: a circle,
a shifted native domain, a rotated/scaled ellipse, open quadratic and cubic
curves, an affine image, and a quartic fallback. In an isolated Rhino 8.32 run,
direct point maps agree within `8.89e-16` per coordinate. Native sampled fit
errors stay below `1.1e-15` for the six `1e-11` cases and `5.84e-10` for the
`1e-9` quartic case. Rhino's corresponding sampled fit errors reach `9.53e-6`
despite the tighter requested tolerances, as in the earlier curve probes.
This observation does not establish a universal Rhino tolerance floor.

The direct-map comparison passes at absolute `2e-12`, relative `1e-12`. The
separate fitted-output comparison uses the existing curve-probe bound of
absolute `1e-5`, relative `1e-12`, with maximum coordinate difference `8.96e-6`.
Native fit checks are never relaxed to that cross-engine approximation bound:

```sh
tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_rational_morph.json \
  --absolute-epsilon 1e-5 --relative-epsilon 1e-12
```

The combined checkpoint passes 1,350 Rust tests, 29 Python tests, formatting,
and strict Clippy. All 121 native fixtures (1,217 operations), 369 recorded
curve comparisons and 43 existing morph/jet/mesh comparisons pass at their
documented bounds. Eight fresh Rhino reads of exported B-reps also pass validity
and mesh checks, with maximum cross-reader difference `5.33e-15`, including the
sphere at its new `1e-11` fitting tolerance. Full Rhino compatibility remains
incomplete.
