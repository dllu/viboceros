# B-rep isocurve precision

[Local UV frames](brep-parameter-frames.md) · [Surface commands](commands/surfaces.md)

Trim-aware isocurve extraction and B-rep display wires now operate in a lossless
local UV frame. The change fixes two distinct routes by which a large parameter
origin could leak into otherwise small, well-resolved model-space geometry.

## Reproduced failures

On `z = u² + v²`, trimmed to the annulus of radii `0.25` and `0.5`, translating
only the UV representation by `[1e12, -2e12]` caused:

- A U-isocurve at local `v = 0.125` to begin at
  `(-0.484130859375, 0.125, 0.250007688999176)` instead of the analytic
  `(-0.4841229182759271, 0.125, 0.25)`. The trim intersection was rounded onto
  the native parameter grid before the surface was trimmed.
- Density-1 wire sampling to return `(-0.425048828125, 0, 0.1806665062904358)`
  instead of `(-0.425, 0, 0.180625)`. Even exactly representable curve endpoints
  do not make intermediate native-domain sampling accurate.

Both regressions failed on baseline `22625f5` and pass after this change at a
model-space distance limit of `2e-12`. A separate regression verified that tight
face/B-rep bounds and trim-boundary bounds already handled these offsets through
homogeneous normalized evaluation; their implementation needed no change.

## Kernel and command behavior

`brep/isocurves` owns trimmed extraction and B-rep wire generation. A face's
surface knots and UV trim controls are translated together, using the existing
lossless frame guard. Root finding, interval classification, and NURBS trimming
all finish locally, without a rounded intermediate native UV coordinate.

The public `BrepFace::isocurve_u_segments` and `isocurve_v_segments` methods
still accept a **native fixed parameter**. Invalid inputs report that original
coordinate and domain. For extracted curves, the varying-axis native origin is
restored only if **every knot**, including newly computed intersection knots,
can be translated exactly. Otherwise the output curve keeps its local domain.
Neither case changes its control coefficients, degree, or weights.

The density-batch variants prepare one frame per face/direction and generate
fixed stations locally, including natural boundaries. Trim-aware
`ExtractIsocurve ExtractAll` uses these variants. The command now has a separate
module for parsing and document operations. Regressions check U/V/Both extraction
on a bilinear saddle at third-span stations, source preservation, output selection,
and undo/redo.

`Brep::wireframe_curves` prepares one frame per face and keeps interior display
isocurves locally parameterized even when restoring their native knots would be
exact. This protects subsequent fractional display sampling. Original 3D edges
are emitted once and remain unchanged. Tests compare density 1, 3, and 5 wires
against untranslated geometry, including their sampled points.

## Licensed Rhino comparison

The [generated fixture](../tools/rhino_oracle/fixtures/brep_isocurve_frames.json)
contains local/translated U-offset disks, V-offset disks, and annuli. Each checks
both directions at four UV pairs, including queries outside the retained region.
It uses the public
[RhinoCommon `TrimAwareIsoCurve`](https://developer.rhino3d.com/api/rhinocommon/rhino.geometry.brepface/trimawareisocurve)
API. Results are normalized to `[0,1]` only **after extraction**, canonicalized
by orientation and segment order, and sampled at six fixed fractions. This does
not repair a trim intersection that an engine has already rounded.

A fresh licensed Rhino 8 run completed in a private Xvfb session. The
[comparison](brep-isocurves-rhino-comparison.json) uses absolute `1e-9`, relative
`1e-12`; raw [native](brep-isocurves-native-reference.json) and
[Rhino](brep-isocurves-rhino-reference.json) responses are retained.

| Fixture | Local maximum coordinate error | Translated maximum coordinate error |
| --- | ---: | ---: |
| U-offset disk | `3.20e-16` | `1.53e-4` |
| V-offset disk | `3.20e-16` | `7.25e-4` |
| Annulus | `2.23e-16` | `4.81e-4` |

All local cases pass. All translated cases remain explicit Rhino discrepancies;
the comparison tolerance was not widened. Native tests independently verify the
analytic paraboloid segments for all six cases within `2e-12`. The recorded
timings include extraction, normalization, sampling, and engine-specific bridge
and cleanup costs; their ratios are not general kernel speedups.

```sh
python3 tools/numerics/generate_brep_isocurve_fixtures.py | \
  diff - tools/rhino_oracle/fixtures/brep_isocurve_frames.json
cargo test -p viboceros-geometry brep::isocurves
cargo test -p viboceros-command extract_isocurve
cargo test -p viboceros-oracle trimmed_isocurve_fixtures
tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/brep_isocurve_frames.json \
  --absolute-epsilon 1e-9 --relative-epsilon 1e-12 --timeout 240
```

The last command is a diagnostic and currently reports the translated failures.
Python tests also check direction conventions, asymmetric sample stations,
invalid inputs, and disposal of every extracted curve on sampling failure.

The full workspace passes **2,498 Rust tests**, with 18 intentionally ignored,
including the exhaustive surface-grid stress test (the geometry suite took
520.68 seconds). All **182 Python tests**, strict Clippy, formatting, and
deterministic regeneration of both paired fixture files pass as well.

## Ordinary-input cost

The [release benchmark record](brep-isocurves-performance.json) compares baseline
`22625f5` with this kernel change using disks and annuli, single-curve queries,
and wire densities 1, 3, and 5. Three rounds of 1,000 iterations produce 24
bit-for-bit identical records of degree, knots, controls, weights, and sampled
points. Median times change by **−0.6% to +1.3%**. The record includes the
temporary probe source and all raw timing rounds. Other build/stress-test work
was active; these measurements do not establish a significant speed change.

## Remaining limits

This is not universal parameter-translation invariance. A lossy frame candidate
is declined. Bare `NurbsSurface` extraction, `IgnoreTrims=Yes`, and point-picking
through a native closest-UV result retain their existing native-coordinate limits.
An extracted curve whose native knots are restored exactly can still have a
poorly resolved native sampling grid; reparameterize it before comparing its
normalized locus. Display wires deliberately do not restore that origin.

Independent 3D edges and trims' own knot parameters are not reframed. For
example, building a full bilinear saddle directly with domains
`[1e12,1e12+1] × [-2e12,-2e12+1]` can fail edge/trim correspondence validation
because its generated 3D edges inherit coarse native knot domains. The command
regression instead translates only the UV face representation of an existing
valid B-rep, retaining independently parameterized spatial edges. Arbitrary
degenerate topology, native structure edits, and model-space output rounding
are outside this correction.
