# Curvature measurement

[Commands](commands/editing.md) · [Surface evaluation](surface-evaluation.md) · [Oracle](oracle.md)

Enter `Curvature` and select a curve or surface, or preselect a single source.
Move the pointer over the source and click to report the displayed
sample. Continue picking or finish with Enter or Esc. A curve shows its
osculating circle and tangent. A surface shows the two principal normal
sections. The status bar displays the radius or principal curvatures.

The curve getter marks and snaps to detected maxima and inflections.
It finds exact maxima on analytic ellipses and searches bounded native
parameter intervals on general curves. A surface pick starts on the visible
tessellated face and refines against its underlying surface along the viewing
line. Face numbering, trims and orientation are preserved.

A complete positional command evaluates the supported selected object nearest
the world-space point. For that form, an underlying closest point outside the
retained trim region is rejected. The viewport preserves a single preselected
source. With several preselected objects it asks for one source and clears the
prior selection when that source is accepted.

```text
Circle 0,0,0 2
SelLast
Curvature 2,0,0
Curvature MarkCurvature=Yes 2,0,0
```

The initial `MarkCurvature=No` setting produces read-only measurements. Use
`MarkCurvature` to toggle permanent markers or enter `MarkCurvature=Yes|No`.
The interactive setting carries over to the next session. Curve reports
include the native parameter, 3-D point, tangent, curvature magnitude, radius
and curvature center. Surface reports include native U/V, the 3-D point,
oriented normal, both principal curvatures and directions, Gaussian curvature
and mean curvature.

## Permanent markers

`MarkCurvature=Yes` adds an unselected point and osculating geometry with fresh
attributes on the current layer. The accepted picks in one viewport session
form one undo step and remain after Enter or Esc.

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

Headless Rhino viewport probes covered repeated curve and surface picks,
command-first curve selection, the saved marking option, and marker creation.
The ellipse with semiaxes 4 and 2 reported radii of 1 and 8. The plane
produced two tangent lines, and the radius-three cylinder produced a
half-circle and one tangent line at each marked point. Egui tests check that
the visible guides and accepted marker geometry agree, including at a snapped
inflection. Landmark recovery on general curves uses a bounded numerical
search. Closely spaced or unusually high-order extrema can be missed.

[McNeel's Curvature command reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/curvature.htm)
describes the viewport guides.
