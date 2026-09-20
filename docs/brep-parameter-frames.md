# Local parameter frames for B-rep evaluation

[UV numerical recovery](uv-rational-range.md) · [B-rep meshing](brep-meshing.md) · [Mass properties](mass-properties.md)

B-rep validation, containment, meshing, and mass integration now evaluate in a
temporary local UV frame when the stored representation can be translated
without rounding. This addresses composed-evaluation loss that exact rounding of
an individual UV point cannot fix.

## Failure and correction

A two-by-one planar face with a rectangular hole, U domain `[1e12, 1e12+1]`,
and V domain `[-2e12, -2e12+1]` failed edge-to-trim correspondence validation.
The independent 3D edge was correct, but evaluating the UV trim first rounded
its coordinates onto a grid with spacing of order `1e-4`. Surface evaluation
then magnified that quantization into model-space error above the default
`1e-9` distance tolerance. Increasing UV point precision internally but rounding
before composition did not solve the problem.

`brep/parameter_frame` removes a common UV offset **before** trim interpolation,
and translates the surface knots by the same amount. The composed image and its
native trim derivative therefore do not pass through a rounded large-origin UV
coordinate. The frame is shared by:

- bidirectional edge/trim correspondence and loop validation;
- containment scans after checking the caller's original native domain;
- independent face meshing, density selection, and conforming shared-edge meshing;
- planar and curved trimmed area/volume integration and rectangular patch integration.
- [trimmed isocurve extraction and display wires](brep-isocurves.md), including
  density-selected stations for trim-aware `ExtractIsocurve ExtractAll`.

The underlying surface preparation now lives in `nurbs_surface/parameter_frame`
and is also used by [rectangular construction and standalone surface wires](surface-wire-frames.md).
The B-rep wrapper supplies its UV trim controls to the same lossless guard.

Temporary frames are per face, not per evaluation or trim. Unshifted faces borrow
the original face; shifted faces own one temporary surface and trim set. Model
vertices and shared 3D edges are not copied into the frame. The original B-rep,
its serialized domains, topology, trim directions, weights, and tolerances stay
unchanged. The trimmed curve's own knot parameter is not reparameterized.

## Lossless preparation

Each axis is handled independently. An interval wholly above zero uses its
lower endpoint as a candidate origin; an interval wholly below zero uses its
upper endpoint. An interval containing zero stays unshifted.

Every knot in that surface direction, including exterior/unclamped knots, and
every UV control coordinate on every loop must subtract the candidate exactly.
An error-free difference recurrence checks the subtraction residual and requires
all intermediates to remain finite. A nonzero residual or overflow declines
that axis. The other axis can still shift. No tolerance decides whether lost
coefficient bits are acceptable; even minimum-subnormal excursions must survive.
This also preserves knot multiplicities and nonempty spans. Applying the frame
again is a no-op.

This is an exact translation of the stored representation, not a claim that
all subsequent floating-point evaluations are exact. It performs no weight
normalization, coordinate scaling, fitting, or mutation of the source.

## Validation

The subtraction check is tested against exact rational differences over a
deterministic cross-product of extreme, adjacent, subnormal, and pseudo-random
binary64 inputs. Additional tests verify exact rational reconstruction of every
translated knot/control, unchanged metadata and source data, borrowed identity
frames, idempotence, and separate rejection of lossy exterior knots and controls.

Integration regressions cover both UV signs and axes, holes, full surface grids,
independently parameterized shared edges, and conforming reconstruction. Curved
paraboloid disks, annuli, and capped solids retain analytic area and signed volume
within `2e-12`, including reversed solids. Public meshes preserve orientation,
closed/naked boundary topology, and samples on the analytic spatial circles.
A direct composition test also checks points and native first derivatives of
a nonlinear rational line against analytic formulas with both weight signs;
the same intermediate native UV output demonstrably loses more than `1e-6`.

The generated [paired oracle fixture](../tools/rhino_oracle/fixtures/brep_parameter_frames.json)
contains ten operations: local and translated versions of U-only/V-only disks,
an annulus, and outward/inward capped paraboloids. Dyadic UV control coordinates
ensure that generating the translated input does not itself round away geometry.
Native probes check analytic masses within `2e-12` and exercise the public `Area`
and `Volume` command invariants. Local/translated area differences in the
[recorded native run](brep-parameter-frames-native-reference.json) are at most
`2.23e-16`, and volume differences at most `1.39e-17`.

Both oracle builders now check UV endpoint gaps against the fixture's absolute
tolerance instead of using model-space `IsClosed` to classify UV loops. The
model-space origin-relative degeneracy rule can otherwise classify a small,
genuinely closed UV loop near `1e12` as a point. This does not bypass either
engine's B-rep validity or interior-point checks. A Python regression verifies
accepted zero gaps, rejected excessive gaps, and cleanup on failure.

The first fresh licensed Rhino 8 comparison [timed out after 240 seconds](brep-parameter-frames-rhino-timeout.json).
Worker progress reached the V-translated disk, but no completed numerical response
was received. Its owned processes were cleaned up. The full paired fixture is
therefore a diagnostic, not an all-passing Rhino reference; native analytic
agreement is not being substituted for external parity.

A subsequent [isolated comparison](brep-parameter-frames-rhino-comparison.json)
completed with all five local fixtures passing at absolute `1e-8`, relative
`1e-10`. The U-translated disk failed: Rhino reported area `0.9573965442376333`,
versus native `0.9573622037878233`, a difference of `3.43e-5`. The native result
also matches the analytic paraboloid formula within the independent `2e-12`
test limit. The external comparison tolerance was not widened. Reconstruct
this request by keeping every `-local` operation plus
`parameter-frame-disk-u-translated` and setting `iterations=1`.

The recorded U-translated Rhino integration took about 27.7 seconds per timed
iteration; its local counterpart was much faster. These harness measurements
do not establish a general kernel-speed ratio, and ordinary mass fixtures do
not yet establish native-Rhino speed parity.

A separate fresh run passed all [five ordinary mesh-boundary comparisons](brep-parameter-frames-mesh-rhino-comparison.json)
with exactly matching recorded geometry/topology at absolute `1e-9`, relative
`1e-12`.

```sh
python3 tools/numerics/generate_brep_parameter_frame_fixtures.py | \
  diff - tools/rhino_oracle/fixtures/brep_parameter_frames.json
cargo test -p viboceros-geometry brep::parameter_frame
cargo test -p viboceros-geometry local_parameter_frames
cargo test -p viboceros-oracle local_and_translated_uv_fixtures
```

The full workspace passed **2,490 Rust tests** with 18 intentionally ignored.
The subsequently added direct composition/derivative regression passed separately,
bringing the verified total to **2,491**. A follow-up workspace run also passed
with only the already-completed exhaustive grid stress test filtered out. All
**179 Python tests**, strict Clippy, formatting, and deterministic fixture
regeneration passed.

## Ordinary-input cost

The [release measurement record](brep-parameter-frames-performance.json) compares
baseline `1f896d5` using three rounds of 100 iterations. All 15 mesh records and
18 mass-property records remain bit-for-bit identical. Median mesh timings
change by **+0.2% to +3.3%**, and mass timings by **-2.1% to -0.2%**. Other
build/test/oracle work was active, and the raw rounds include outliers. These
bounded measurements do not establish a statistically significant speedup or
general performance guarantee. Shifted faces additionally pay for lossless
coefficient checks and a temporary face per evaluation phase. Density selection,
independent meshing, and a conforming retry currently prepare separate frames.

## Limits

An axis whose full representation cannot be translated exactly retains its
native evaluation behavior. Other consumers of native UV outputs, including
closest-parameter results and structure-edit APIs, do not automatically inherit
this frame. Composed bounds use their own homogeneous normalized evaluation;
the [large-offset isocurve regressions](brep-isocurves.md) also verify those
bounds without changing their implementation. Nor does the frame fix a trim's
own poorly resolved native knot
parameter, arbitrary near-degenerate topology, or model-space output rounding.
These are bounded improvements, not universal translation-invariance or Rhino
compatibility guarantees.
