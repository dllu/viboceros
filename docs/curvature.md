# Curvature measurement

[Commands](commands/editing.md) · [Surface evaluation](surface-evaluation.md) · [Oracle](oracle.md)

`Curvature` evaluates the selected curve or surface nearest a model-space point.
Omit the point in the application to pick one location in a viewport; Esc cancels
without changing the document. Existing objects, selection, attributes and groups
are retained. B-rep queries respect face trims and face orientation.
Underlying closest points outside a face's retained trim region are rejected;
this does not search for a nearest point on the trim boundary.

```text
Circle 0,0,0 2
SelLast
Curvature 2,0,0
Curvature MarkCurvature=Yes 2,0,0
```

The default `MarkCurvature=No` only reports a measurement, without adding an undo
entry. Curve reports contain the native parameter, point, curvature magnitude
and radius. Surface reports contain the native U/V parameters, point, oriented
normal, principal curvatures, Gaussian curvature and mean curvature.

## Permanent markers

`MarkCurvature=Yes` adds an unselected point and osculating geometry with fresh
attributes on the current layer. All markers form one undo/redo transaction:

- A curved curve gets a full circle; a flat curve gets only the point.
- A curved surface direction gets a normal-section half-circle through the
  evaluated point. Its center is `point + normal / curvature`.
- A flat surface direction gets a tangent line centered at the point, with
  half-length equal to 10% of the source's control-geometry bounding-box diagonal.
  The parabolic-cylinder fixture distinguishes this from a tight geometric box.

Curvatures outside `1e-16..=1e16` in magnitude use the flat-marker fallback,
matching the range of Rhino's public osculating-circle helper. This is a display
policy, not a clamp on the reported curvature. Marker construction uses a local
size-aware tolerance so valid small markers are not collapsed by model tolerance.

## Geometry API and numerical policy

`CurveRef::curvature_vector(t)` exposes the curve's derivative of unit tangent
with respect to arc length. Analytic circles/arcs retain their radial formula;
NURBS and composite curves use their native derivatives. Polyline vertices use
the active segment's zero curvature, not an invented unique vertex curvature.

`NurbsSurface::curvature_at(u, v)` and `curvature_at_on_sides` evaluate analytic
second partials. `SurfaceJet2::curvature()` also accepts an already evaluated
jet, including boundary-span continuation. The result `SurfaceCurvature`
contains the point, normal, two principal curvatures and their unit directions.
The first principal curvature has the largest **absolute** magnitude, not
necessarily the largest signed value. Signs refer to `Su × Sv`: an outward
sphere has principal curvatures `-1/r`. Reversing orientation negates both
principal curvatures and the mean, while preserving the Gaussian curvature.

The shape operator is formed in an orthonormal tangent frame. First partials
are normalized independently, avoiding the subtraction `E G - F²` and avoiding
dependence on parameter units or model tolerance. Scaled second fundamental
form coefficients yield a symmetric 2×2 operator. Its large eigenvalue is
computed directly; the small eigenvalue uses a compensated determinant divided
by the large one, avoiding center-minus-radius cancellation. Directions form a
right-handed orthonormal frame with the normal.

Principal-direction signs are arbitrary; at an umbilic even their axes are not
unique. Oracle records therefore compare the full spatial shape operator
`Σ kᵢ dᵢ dᵢᵀ`, rather than requiring an arbitrary eigenvector sign or frame.
Marker records likewise retain circle planes and half-circle loci without
requiring an arbitrary endpoint traversal direction.
Marker-tensor overflow is a probe error, not a JSON null accepted as a measurement.

Zero first partials and numerically parallel tangents are errors, not flat
curvature. The tangent-plane guard uses `64 ε` on the sine of the angle between
normalized first partials. No singular limiting curvature is supplied; the
tested NURBS sphere poles are unavailable in both implementations. Gaussian
curvature can overflow even when both principal values are finite, so
`gaussian()` is independently fallible. Ill-conditioned parameterizations and
unrepresentable derivative intermediates remain finite-precision limitations.

## Verification and compatibility limits

Native analytic tests cover arbitrary graph Hessians, skew parameter frames,
saddles, planes, spheres, cylinders, tori, small eigenvalues, independent U/V
scales through `1e±150`, subnormal tangent-product intermediates, model scales
`1e±140`, orientation reversal, and exact
large-translation invariance. Command tests cover fresh marker attributes,
selection, undo/redo, cancellation, malformed input and choosing the nearest
object before differential evaluation.

`surface_curvature.json` contains 27 API cases, including exact one-sided knot
limits, rational weight scales, continuation and singular poles. They pass at
absolute `2e-12` plus relative `1e-12`. The large absolute differences in the
small-radius fixture are relative rounding in curvature squared, not point error.

`surface_curvature_umbilic.json` separately checks a radius-two sphere. Native
principal curvatures remain within `1e-12` of the analytic `-0.5`. Rhino sometimes
splits that repeated eigenvalue by about `7.45e-9`; the full-record comparison
therefore uses absolute `1e-8` plus relative `1e-12`. Normals, points, mean and
Gaussian curvature are checked separately at the ordinary epsilon. The
native implementation does not reproduce Rhino's spurious eigenvalue split.

`curvature_command.json` contains 17 actual command cases: curves, planes,
cylinders, a saddle, reversed B-rep faces, scaled markers and a parabolic cylinder.
They check marker geometry, source preservation, selection, fresh attributes
and the presence of a command report. Printed strings are not an unrounded
numeric oracle; the separate API fixtures check the numerical measurement.
Rhino runs use a private Xvfb display. Unique history markers prevent a full
Rhino history buffer from making later measurements appear absent.

```sh
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/surface_curvature.json \
  --absolute-epsilon 2e-12 --relative-epsilon 1e-12
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/surface_curvature_umbilic.json \
  --absolute-epsilon 1e-8 --relative-epsilon 1e-12
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/curvature_command.json \
  --absolute-epsilon 2e-12 --relative-epsilon 1e-12
```

Continuous hover previews, automatic curvature-extremum/inflection finding,
curvature graphs and false-color surface analysis remain unimplemented.
See the public [Curvature command](https://docs.mcneel.com/rhino/8/help/en-us/commands/curvature.htm)
and [SurfaceCurvature API](https://developer.rhino3d.com/api/rhinocommon/rhino.geometry.surfacecurvature)
for the broader Rhino behavior.
