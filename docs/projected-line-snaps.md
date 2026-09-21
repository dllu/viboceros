# Straight-line snap projection

[Near](near-snaps.md) · [Mid hover](mid-hover-snaps.md) · [Architecture](architecture.md)

An audit after `ca54e32` reproduced three numerical failures: Near missed a thin
visible portion of a camera-crossing line, a long projected segment could choose
its endpoint instead of an interior target, and hover distance could similarly
round an interior minimum to an endpoint. The new focused regression tests failed
before the repair; these were not discovered by the original Rhino fixtures.

## Shared implementation

`viboceros-drafting/object_snap/projected_line` owns straight-locus clipping,
screen distance and inverse projection. Near uses it for analytic lines,
polyline segments and common-sign degree-one NURBS spans. Mid and polygon Center
share its distance query; their target/visibility/priority policies are unchanged.
The implementation is independent of egui and document mutations.

If one endpoint projects and the other does not, the visible boundary is bisected
in model coordinates. Every retained inside point actually projects. Midpoint
stagnation terminates the search; the 2,200-step guard covers the binary64 exponent
and significand range. No fixed sampling interval or geometric epsilon removes a
thin visible portion. The resulting visible segment uses the same direct query
as an unclipped line. The callback must remain affine/projective in a convex
visible half-space; it is not an arbitrary nonlinear map or occlusion test.

Screen distances are evaluated from the nearer endpoint. Inverse projection
computes both endpoint fractions independently, then interpolates from the nearer
model endpoint using the smaller model fraction. It never recovers a tiny offset
by subtracting a rounded fraction from one. An adaptive interior image station
recovers the one-dimensional projectivity when a midpoint loses the camera-depth
ratio. Parallel axis queries bypass that projectivity recovery.
Near rejects projected segment bounds outside the capture square before norms
or inverse projection; the distance query chooses its nearer endpoint from the
two fractions without computing two additional endpoint norms.

Two unprojectable endpoints do **not** establish invisibility: floating-point
projection limits can hide endpoints but leave a projectable interior. Such
unresolved intervals retain the bounded curve-search fallback. Extreme roundoff,
near-degenerate projections, curved visible slivers and mixed-sign rational spans
are not universally solved or certified. Clipping and interpolation use binary64
coordinates and do not establish an all-range exact model-locus guarantee.

## Independent evidence

[`projected_lines.py`](../tools/rhino_oracle/references/projected_lines.py) uses
Python `Fraction` arithmetic, without either engine's output or APIs. It clips
exactly against homogeneous `W >= near`, minimizes projected squared distance and
inverts the line homography. All operations are rational until final conversion
of expected outputs to binary64. Its Python tests check exact stationarity,
endpoint/boundary derivative signs, visibility, reversal and known analytic cases.

The generated [288-row corpus](../crates/viboceros-drafting/src/object_snap/projected_line/reference.csv)
combines three camera-axis permutations, dyadic shear/scaling/offsets, depth ratios
through `1e12`, clipped and fully visible sources, six cursor positions and both
endpoint orders. Native model coordinates and projected/hover distances are
checked within `3e-11 * max(1, |expected|)` per component/value. This is an
independent mathematical reference, not a new Rhino measurement.

Additional regressions cover camera-crossing depths through `1e100`, Near on
line/polyline/rational degree-one sources, clipping-boundary targets, rejected
invisible segments, common-positive/common-negative weight hover and asymmetric
screen extents through `1e300`. The actual perspective viewport also captures a
thin visible line portion in both endpoint orders; its `1e-5` model-point bound
includes egui's binary32 pointer quantization.

The existing calibrated Rhino Near/Mid/Center replays remain regression evidence
for their retained cases. No user desktop or existing Rhino process is accessed
by this independent audit.

Validation checkpoint: 3,126 ordinary release-workspace tests pass (29 opt-in
tests ignored), plus all 301 Python tests and seven opt-in offscreen GPU tests
on NVIDIA GB10/Vulkan. Formatting, whitespace, strict Clippy and strict rustdoc
checks pass as well.

The existing warm `near_scene_timing` diagnostic measured these local release
samples after the final checks:

| Scene objects | Lines, microseconds/query | Quadratic NURBS, microseconds/query |
| ---: | ---: | ---: |
| 1 | 0.228 | 23.847 |
| 100 | 4.852 | 42.956 |
| 1,000 | 47.106 | 190.877 |

The diagnostic uses 100 queries after 10 warm-up queries, identity XY projection,
and separated objects. These samples do not isolate this change's timing effect
and are not a statistical speedup, frame-rate or Rhino comparison. Clipped-line
queries additionally pay for visible-boundary bisection; scene traversal remains
linear in object count.

## Reproduction

```sh
python3 -m tools.rhino_oracle.references.projected_lines
python3 -m unittest tools.rhino_oracle.test_projected_lines
cargo test --release -p viboceros-drafting object_snap
cargo test --release -p viboceros perspective_near_captures
```

The generator writes CSV to stdout; the Python regression also verifies that the
checked-in corpus exactly matches its deterministic output.
