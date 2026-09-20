# Local surface frames for edges and wires

[B-rep frames](brep-parameter-frames.md) · [Trimmed isocurves](brep-isocurves.md)

Natural and rectangular surface-to-B-rep construction now generates spatial
edges in a lossless local parameter frame. Standalone surface borders,
topological edges, wireframes, and density-batch isocurves share this behavior.
The stored surface and native UV trims remain unchanged.

## Reproduced failures and correction

At baseline `9bd67f4`, constructing a full bilinear saddle with domains
`[1e12,1e12+1] × [-2e12,-2e12+1]` failed B-rep validation with
`a p-curve interior leaves its model-space edge`. Newly created 3D edges
inherited coarse native knot domains; their sampled points disagreed with the
accurately evaluated UV trim image.

A separate rational-surface boundary sample was
`(0.18177782710021084,0,0)` rather than `(0.18181818181818182,0,0)`.
This failed before any interior wires were requested (`density=-1`). Both
regressions now pass with model-space sample distances below `2e-12`.

`nurbs_surface/parameter_frame` owns shared preparation. Each axis may remove
an offset only when every stored knot, including exterior knots, and every
caller-supplied coordinate subtracts that origin exactly. The existing
error-free difference guard now lives in `parameter`; no tolerance decides
whether coefficient bits may be discarded. An inadmissible axis stays native.

`brep/parameter_frame` supplies all UV trim controls to this shared preparation.
`brep/rectangular_surface` supplies the requested rectangle's corners, builds
corner points and edge curves locally, and stores the original surface and
native trim corners in the final face. New spatial edges keep local domains.
Seam sharing, singular trims, vertex grouping, orientation, and iso classes
are preserved. The constructor also avoids cloning its owned surface just to
forward it, and avoids constructing an unused tensor trim for domain validation.
Curve trimming and rectangular construction share the same interval checks.

`nurbs_surface/wires` owns natural borders, topological edges, wire-density
rules, and density-batch extraction. It generates stations and returned curve
domains locally. Native `wire_parameters_u/v` remain native-parameter APIs;
clients needing geometry without intermediate native rounding can use
`isocurves_u_at_density` or `isocurves_v_at_density`.

`ExtractIsocurve ExtractAll` uses those batch methods for standalone surfaces
and for `IgnoreTrims=Yes`. Trim-aware B-rep extraction retains the
[previously documented lossless domain-restoration policy](brep-isocurves.md).

## Verification

Native tests cover polynomial and rational saddles with both weight signs,
full and partial rectangles, both UV offset signs, face reversal, and unchanged
source data. Cylinder, cone, sphere, and torus regressions verify seam/pole
topology, border counts, wire samples, and mesh faces/vertices after translation.
The translated primitive inputs explicitly check that creating their shifted
knot vectors did not already round away source data.

Command regressions now construct the large-domain B-rep directly, instead of
using a hand-built translated face as a workaround. They cover surface/B-rep
sources, trim-aware/untrimmed extraction, U/V/Both directions, third-span
stations, selection, source preservation, and undo/redo. Two older assertions
that required spatial curves to retain native domains were updated to check
local domains **and** their original 3D loci; source UV checks remain in place.

The exhaustive geometry run passed its surface-grid stress test and exposed
one of those domain assertions. After correction, the full workspace passed
with only that already-completed stress test filtered out. Together the runs
verify **2,503 Rust tests**, with 18 intentionally ignored. All **190 Python
tests**, strict Clippy, formatting, and fixture regeneration pass.

## Licensed Rhino comparison

The [12-operation fixture](../tools/rhino_oracle/fixtures/surface_wire_frames.json)
uses rational saddles and cylinders, densities `-1`, `1`, and `3`, and local/
translated domains. The Rhino worker calls public
[CreateFromSurface](https://developer.rhino3d.com/api/rhinocommon/rhino.geometry.brep/createfromsurface)
and [GetWireframe](https://developer.rhino3d.com/api/rhinocommon/rhino.geometry.brep/getwireframe)
in a private Xvfb session. Topology counts and six samples per curve are recorded.
Only the output curves are normalized after extraction; source surfaces and
the positions of extracted wires are not repaired or moved.

The [native](surface-wire-frames-native-reference.json) and
[Rhino](surface-wire-frames-rhino-reference.json) responses retain measured
coordinates. Sorting keys alone are quantized to avoid last-bit ordering noise
at symmetric coordinates. Larger translated errors can still permute symmetric
wires, so the [recorded comparison](surface-wire-frames-rhino-comparison.json)
also verifies unique nearest sample matches form a bijection before reordering
reference wires. It then applies the unchanged standard comparator to every
coordinate and topology field, at absolute `1e-9`, relative `1e-12`.

All six local cases and four translated boundary/midpoint cases pass, with a
maximum coordinate difference of `1.34e-15`. Two translated density-3 cases fail:

| Fixture | Maximum matched sample-coordinate difference |
| --- | ---: |
| Rational saddle | `8.14e-5` |
| Cylinder | `1.85e-3` |

The comparison tolerance was not widened. Native paired fixtures preserve
topology and agree with analytic geometry and their unshifted samples within
`2e-12`. Correspondence diagnostics reject missing, duplicated, ambiguous,
non-bijective, malformed, and nonfinite records. Six-sample comparison is not
a general curve-distance proof or proof of all Rhino behavior.

```sh
python3 tools/numerics/generate_surface_wire_fixtures.py | \
  diff - tools/rhino_oracle/fixtures/surface_wire_frames.json
cargo test -p viboceros-geometry brep::rectangular_surface
cargo test -p viboceros-geometry nurbs_surface::wires
cargo test -p viboceros-oracle surface_wires
python3 -m tools.numerics.compare_surface_wire_records \
  docs/surface-wire-frames-native-reference.json \
  docs/surface-wire-frames-rhino-reference.json
```

The final command intentionally reports the two translated failures. Rhino
timings include Python sampling and cleanup; native timings include two wire
generation paths and sample outside the timed region. These are different
workloads, not kernel-speed ratios.

## Cost and limits

The [ordinary release checks](surface-wire-frames-performance.json), against
`9bd67f4`, preserve all 15 mesh and 18 mass records bit-for-bit. Three rounds
of 100 iterations change median mesh timings by `+0.5%` to `+3.4%`, and mass
timings by `-1.9%` to `+0.2%`. Other compilation/stress tests were active; these
measurements do not establish a significant speed change.

The frame does not fix lossy input reparameterization, axes whose complete
representation cannot be translated exactly, arbitrary degeneracy, or native
point-picking/structure-edit intermediates. Single native isocurve APIs and
already-existing 3D curves can still have poorly resolved native sampling
grids. Trim-aware B-rep extraction can restore exactly representable native
knots; normalized locus comparisons of those output curves still require
reparameterization. This correction does not silently alter stored source
geometry or promise universal parameter-translation invariance.
