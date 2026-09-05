# Native curve cutting and rational parameter maps

[Curve parameters](curve-parameters.md) · [Split](commands/split.md) · [Trim](commands/editing.md)

Cutting-object `Split` and `Trim` compute intersections using rational geometry,
then convert the returned parameters back to the original curve. Their output
retains lines, arcs, polylines, NURBS, and native polycurve leaves. Ellipses use
their parameter-equivalent rational representation. Projected trimming applies
the same correspondence to the original, unprojected source, preserving its depth.
Attributes, groups, selection, source identity policies, and undo remain document
operations. Curve cutting and interval replacement live in `curve_cut.rs`.

## Parameter correspondence

`CurveRef::nurbs_parameter(t)` and `parameter_from_nurbs(t)` are checked inverse
maps for `CurveRef::to_nurbs()`. They correspond to Rhino's public
[curve/NURBS parameter-conversion API](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Geometry_Curve_GetCurveParameterFromNurbsFormParameter.htm).
They do not use closest-point inversion, which would lose branch identity when
a curve visits the same point more than once.

Circular curves map within their one, two, or four rational quadratic spans.
For a span's half-angle `h` and native fractional angle `f`, the rational fraction
is `sin(h*f) / (sin(h*f) + sin(h*(1-f)))`. The inverse uses a half-angle `atan2`
formula. This avoids subtraction of rounded world coordinates. Endpoints and
span boundaries are exact identities; sufficiently small angles have a correction
below machine precision and use the identity map. Polycurves compose the leaf
map with their independent outer/local intervals. Noncircular leaves are already
parameter equivalent, so unnecessary round-trip affine rounding is avoided.

Document `nurbs_curve_representation` preserves native polyline parameters.
Explicit `ToNURBS` retains its separate chord-length conversion policy. This
distinction also fixes extrusion: Rhino `ExtrudeCrv` preserves the profile's native
parameters, including a user-assigned interval. The extrusion probe disables
Rhino crease splitting to compare the single underlying surface, matching the
current Viboceros output policy; crease-splitting options remain incomplete.

## Closed curves

Rhino's interactive cutting commands differ from its low-level `Curve.Trim` and
`Curve.Split` APIs. Commands relocate the seam before trimming a wrapped interval,
retaining one arc, polyline, or NURBS rather than a two-part seam polycurve.
A seam intersection is a real cut station: the equivalent start/end hits are
counted once. A single closed-curve station does not produce separate pieces and
leaves the source and its seam unchanged. Duplicate-hit filtering uses the domain
width plus floating-point roundoff, not the absolute parameter origin, to avoid
collapsing distinct coincident branches on translated intervals.

## Evidence and limits

| Fixture | Cases | Observed maximum numeric difference |
| --- | ---: | ---: |
| `curve_parameter_map.json` | 38 | `4.8e-11` (complete native records) |
| `curve_native_cutting.json` | 34 | `6.4e-12` |
| `curve_native_extrusion.json` | 3 | `0` |

The cutting fixture checks native type, domain, 17 parameter-uniform points,
complete rational definitions, attributes, groups, identity, and selection.
It covers reversed curves, seam hits, tangent no-ops, repeated branches, periodic
and unequal-weight closed NURBS, curve/surface/B-rep cutters, and apparent cuts.
These comparisons use `1e-8` absolute and `1e-10` relative limits; extrusion uses
`1e-9` absolute. They do not prove general intersection or full command parity.

An older Trim tangent fixture exposes an ill-conditioned Rhino cut location:
its quadratic is `(10*t, 16*t*(1-t), 0)` and its cutter is `y=4`, so the exact
contact is `t=0.5`. Viboceros returns that value; the observed Rhino command
returned `0.49999965669161467`, only `1.9e-12` off the cutter but `3.4e-6` away
along X. That one full-output comparison uses `5e-6` absolute; the remaining
15 legacy Trim cases agree below `1.8e-15`. The kernel retains the exact root,
with an explicit regression test, rather than reproducing the approximate result.
