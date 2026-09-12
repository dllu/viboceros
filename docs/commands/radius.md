# Radius and Diameter

[Command index](README.md)

Enter `Radius 2,0,0` or `Diameter 2,0,0`. With no coordinates,
the application prompts for one picked or typed point. The closest point on the
nearest eligible curve determines the evaluation location. With no preselection,
all selectable curves are candidates: hidden/locked objects and hidden/locked
layers are excluded, and non-curves are ignored. Preselect curves to restrict
the search to those objects. Both commands report
radius and diameter in model units. They do not change geometry, selection, or
undo/redo history by default. Esc cancels the interactive prompt. Failed point
evaluations keep the prompt and previous point anchor intact for correction.

These are local curvature measurements, not circle-fitting operations: radius
is the reciprocal curvature magnitude and diameter is twice that value. Lines,
polylines, and zero-curvature locations report `infinite`. Other supported inputs
are arcs, circles, ellipses, NURBS curves, and polycurves, using the same
differential evaluation as `Curvature`. A finite result outside the representable
numeric range is an error, not an infinite-curvature-radius report.

`Radius MarkRadius=Yes 2,0,0` or `Diameter MarkDiameter=Yes 2,0,0` adds a point
and, where the existing curvature-marker policy permits, an osculating circle.
Markers use the current layer, preserve source selection, and form one undoable
operation. Flat curves receive a point only. All markers are constructed before
insertion; failed commands roll back atomically.

Tests cover circles, lines, ellipse endpoint radii (1 and 8 for semiaxes 4 and 2),
the ellipse's exact NURBS representation, nearest-curve selection, marker undo,
invalid-input rollback, read-only history, interactive cancellation, unrestricted
point lookup, selection restriction, hidden/locked filtering, and failed-pick recovery.
These are native analytic regressions, not live Rhino command captures.

[Rhino's reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/radius.htm)
also describes cursor feedback, `SelectCurve`, display `Units`,
`SubCrv`, and nested numeric input. Those workflows and live hover/status-bar
curvature feedback are not implemented yet. Point lookup currently uses 3D
nearest distance, not a screen-space hit aperture. Explicit mixed selections
containing non-curves are rejected.
