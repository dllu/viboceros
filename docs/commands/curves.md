# Curve creation and editing

[Command reference](README.md) · [Project overview](../../README.md)

## Construction and fitting

Enter `Point`, `Line`, `Circle`, `Arc`, `Ellipse`, `Polyline`, `Curve`,

`InterpCrv`, `Rectangle`, `Polygon`, `SrfPt`, `Sphere`, or `Ellipsoid` without
coordinates to pick points in the viewport; press Enter to finish a polyline or curve. `Curve`
creates an open control-point curve, defaults to degree three, accepts degrees
through 11, and lowers the degree when too few controls are supplied.
`Close=Smooth` creates a periodic seam (degree one remains linear and closed);
`Close=Sharp` repeats the first control to create a kinked, non-periodic seam.

`InterpCrv` defaults to an open, degree-three, chord-knot curve through the
input points.
It also supports `Degree=1`, `Knots=Uniform|SqrtChrd`, smooth periodic or sharp
`Close=` modes, and start/end tangent directions on open cubics.

`CurveThroughPt` orders selected point objects into a nearest-neighbor chain,
ignores coincident locations, and creates one control-point or
interpolated curve; `Closed=Yes` requests a smooth closed seam (degree-one
output remains linear and non-periodic). `CurveThroughPolyline`
creates one curve through each selected polyline's vertices, inheriting whether
each source is open or closed. Both commands support control-point degrees 1
through 11 (lowered when necessary), while interpolated output currently
supports degrees 1 and 3 with uniform, chord, or square-root-chord spacing.
Polyline sources are retained by default; `DeleteInput=Yes` replaces their
geometry in place while preserving identity and attributes.

`TweenCurves` requires exactly two selected curves, uses their selection order,
and leaves both sources selected. `MatchMethod=None` connects corresponding
NURBS control locations and weights, including Rhino's last-control rule when
the counts differ. `MatchMethod=SamplePoints` supports 2 through 9999
equal-length divisions with bounded, linear-memory interpolation.
`OutputLayer=` accepts `CurrentLayer`, `StartCrv`, or `EndCrv`; `FlipStart=Yes`
and `FlipEnd=Yes`
reverse either temporary source direction. `MatchMethod=Refit` adaptively
rebuilds both sources as non-rational cubics on one bounded shared
structure, retaining source-span boundaries and qualifying kinks while meeting
the document tolerance before their corresponding controls are blended.

`FitCrv` adaptively approximates every selected line, analytic curve,
polyline, or NURBS curve with a non-rational NURBS curve of degree 1 through
11. Output parameters approximate source arc length; endpoints, endpoint
tangents, and polyline/NURBS kinks above `AngleTolerance` (in degrees) are
preserved. `Tolerance` defaults to the document absolute tolerance.
Kink and endpoint tangents use [exact one-sided limits](../curve-sided-evaluation.md),
including stationary points with a nonzero higher derivative.
Failing spans are bisected rather than inserting knots at their worst sampled
point, which could cluster knots and leave large gaps unresolved. Cubic fits use
a banded solve with exact endpoint/kink handle constraints; other degrees use
dense full-pivot solves. Repeated error checks cache exact floating-point source
distances locally (at most 16,384 points), without quantization. Accuracy remains
sampled, with a 512-control budget; exhaustion is an error, not a relaxed tolerance.
`DeleteInput=Yes` and `OutputLayer=InputObject` are the Rhino-compatible
defaults and replace each result in place; `DeleteInput=No` retains the source,
while `OutputLayer=CurrentLayer` puts fresh results on the current layer.

`Rebuild` reconstructs selected curves as uniform, non-rational NURBS curves
with a fixed `PointCount` (default 10) and `Degree` from 1 through 11 (default
3). Open results are clamped and closed results use Rhino-compatible periodic
structure; both interpolate equal-arc-length source stations. Positive point
counts below the structural minimum are raised automatically, as in Rhino.
`PreserveTangents=Yes` aligns eligible open-curve end handles. The
`DeleteInput` and `OutputLayer` options follow `FitCrv`; surface rebuilding is
not yet represented.

## Conics and spatial curves

`Conic` creates Rhino's exact normalized rational quadratic from a start, end,
apex, and either a rho value or through-point. `Apex` switches the documented
pick order; off-plane through-points are projected into the control triangle's
plane before their unique positive weight is recovered. Typed rho input also
preserves Rhino's collinear or coincident control-point degeneracies.

`Parabola` supports Rhino's default `Focus focus direction-point end-point` and
`Vertex vertex focus end-point` constructions. It creates the exact normalized
quadratic NURBS curve, projects the picked end point perpendicular to the focus
axis in vertex mode, and supports `Half=Yes` and `MarkFocus=Yes`.

`Parabola3Pt` creates the exact normalized quadratic from two endpoints and a
focus, through-point, or vertex. It matches Rhino's `ThroughPoint` default,
all three `PickOrder` forms, the additional through-point opening-direction
pick, and `MarkFocus=Yes`.

`Hyperbola` supports Rhino's center, coefficient, two-focus, and vertex input
modes. Each branch is one exact normalized rational quadratic span;
`BothBranches=Yes` and `MarkFoci=Yes` preserve Rhino's object ordering.
`ShowAsymptotes=Yes` is accepted as a preview-only option.

`Helix` creates a non-rational uniform cubic along an explicit axis. Numeric or
picked radii, arbitrary axis directions, fractional `Turns=`, `Mode=Pitch`,
and `ReverseTwist=Yes` are supported. `Spiral` extends the same workflow with
independent numeric or picked start and end radii. `Flat` creates a planar
spiral with an optional `Axis=` normal. `AroundCurve` follows a whole named or
last-selected line, analytic curve, polyline, or NURBS rail. Its first radius
must be a point to seed the radial direction; the end radius can be numeric or
picked. It supports Turns/Pitch, reverse twist, and `PointsPerTurn=` values of
at least five (12 by default).
The axial linear-time tridiagonal constructor
uses Rhino's 24-span density for long forward constant-radius helices and 36
spans otherwise, interpolates every analytic sample, and has a one-million-
control resource ceiling. Its C2 endpoint rule differs from Rhino's legacy C1
endpoint perturbation by less than `3e-5` in the permanent `helix.json` and
`spiral.json` NURBS-control oracle fixtures. Swept spirals use equal-arc-length
rail stations and shared [adaptive frame transport](../curve-frames.md); their complete cubic
control layout agrees with Rhino within `2e-7` across straight, planar curved,
and spatial curved rails in `swept_spiral.json`.

`Catenary` constructs the physical hanging-chain curve from a through point,
analytic length, catenary parameter, or apex height. The third point fixes the
gravity direction from the start point. `Output=Smooth` creates Rhino's
chord-parameterized cubic approximation with exact analytic endpoint tangents;
`Output=Polyline` samples uniform horizontal stations. Both use 20 points by
default, accept `PointCount=`, and can add the exact analytic apex with
`MarkApex=Yes`. Overflow-safe hyperbolic evaluation and bounded root solvers
cover asymmetric and arbitrarily oriented endpoint frames. All nine permanent
smooth/polyline oracle cases agree with Rhino within `2e-8` (the observed
maximum difference is below `2e-10`).

## Division, closure, and seams

`Divide` creates equal arc-length points on selected curves by segment count or
requested segment length; add `MarkEnds` to include open-curve endpoints.

`CrvStart` and `CrvEnd` place attribute-preserving point objects at the natural
ends of every selected curve. `CloseCrv [Tolerance=value] [CloseWideGapsWithLine=Yes|No]`
closes polylines, NURBS curves, arcs, and polycurves. Eligible nearby flexible
endpoints move to the start; otherwise an allowed closing line creates a polycurve.
Straight curves and already-closed objects remain unchanged. `Tolerance=0`
forces eligible endpoint edits and completes an arc's supporting circle while
retaining its original parameter interval. See [closure rules and limits](../curve-editing.md).

`CrvSeam point` relocates exactly one selected closed curve's start/end seam to
the closest curve location; omit the point to pick that location in a viewport,
or use `CrvSeam Parameter=value` for a native parameter. Circles, closed arcs,
polylines, and polycurves retain their native representations; ellipses use their
parameter-equivalent NURBS form. Finite parameters outside the old interval wrap
around the closed curve. Shape and parameter-span length are preserved, and
the output domain starts at the chosen parameter. Smooth periodic seams remain
periodic and gain a control point when required, while an existing
multiple-knot seam becomes Rhino's equivalent clamped form. Rational geometry,
object identity, attributes, groups, selection, and undo are preserved.

## Extension

`Extend Length=value Side=Start|End|Both` extends one selected open curve;
`Both` adds the full requested model-space length independently at each end.
`Type=Natural` selects the source end's natural line, same-radius arc, or smooth
continuation, while `Type=Smooth` explicitly requests curvature-continuous
free-form extrapolation. `Type=Arc` follows the endpoint's exact osculating
circle and falls back to a degree-matched tangent line at zero curvature;
requests beyond one revolution use Rhino's full-circle cap while retaining the
requested parameter-domain interval. `Type=Line` uses Rhino's straight tangent extension.
With `Join=Merge`, line and polyline ends merge into their terminal span,
higher-degree straight curves collapse to one line, and other curves retain
Rhino's exact degree-matched span, knot, and rational-weight rules. A merged
same-radius arc is rebuilt as one canonical rational arc. `Join=Yes` retains an
explicit segment boundary, including Rhino's unit-span polyline
parameterization and exact full-multiplicity free-form seam. `Join=No` leaves
the source untouched and creates one or two native line, arc, or smooth
extension curves with copied attributes but without copying source group
membership. Rhino's post-command deselection and undo behavior are retained.

`Extend point Type=Natural|Arc|Line|Smooth Join=Merge|Yes|No` extends one of
the selected open curves to the nearest intersection with the other selected
curve, surface, or trimmed B-rep boundaries. The point chooses the closest
source endpoint; in the UI, omit it and pick that endpoint in a viewport after
preselecting the source and boundaries. Exact span subdivision handles
transverse and tangent curve-surface hits, while finite coplanar overlaps stop
at their entry edge and B-rep trim holes are excluded. Intersections already
on the source are ignored, multiple boundaries resolve to the first forward
hit, and the source identity, attributes, groups, undo state, and Rhino's
separate-output behavior are preserved.

`Extend Domain=start,end` exposes exact analytic NURBS-domain extension.
Analytic curves and polylines are promoted to exact NURBS form as needed while
identity is preserved. The permanent length-command fixture covers all join
modes for explicit Smooth; its `2e-8` threshold contains Rhino's approximately
`1.2e-8` cubic length-solver variation while Viboceros resolves that case to
machine precision. The boundary-command fixture covers every continuation
style, all join modes, both-end extension, nearest-boundary selection,
standalone surfaces, solid and trimmed B-reps, curved and tangent surface hits,
coplanar overlap, and trim holes to `1e-10`. Arc cases compare a canonical
knot parameterization because Rhino leaks its temporary boundary-search extent
into geometrically equivalent segment domains. Arc geometry and commands
otherwise agree to floating-point roundoff except for the same pre-existing
Smooth length-solver variation.

## Subcurves

`SubCrv start_point end_point` replaces one selected curve with the exact
directed portion between the closest curve locations; omit both points for two
viewport picks, or use `Parameter=start,end` for exact parameters. Reversing
the point order reverses an open result and crosses the existing seam on a
closed curve, matching Rhino/OpenNURBS trim behavior. `Copy=Yes` retains the
source and adds an attribute-preserving result to its groups. Numeric parameters
refer to the native source, without changing angular parameterization first.
Lines, arcs, polylines, and polycurves retain native leaves; partial circles become
arcs and ellipses use their parameter-equivalent NURBS form. A seam-crossing result
is a polycurve containing both trimmed portions. Replacement preserves identity,
attributes, groups, selection, and undo. See [native domain editing](../curve-domain-editing.md).
