# Radius and Diameter

[Command index](README.md)

Select curves, then enter `Radius 2,0,0` or `Diameter 2,0,0`. With no coordinates,
the application prompts for one picked or typed point. The closest point on the
nearest selected curve determines the evaluation location. Both commands report
radius and diameter in model units. They do not change geometry, selection, or
undo/redo history by default. Esc cancels the interactive prompt.

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
invalid-input rollback, read-only history, and interactive cancellation.
These are native analytic regressions, not live Rhino command captures.

[Rhino's reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/radius.htm)
also describes unrestricted cursor picking, `SelectCurve`, display `Units`,
`SubCrv`, and nested numeric input. Those workflows and live hover/status-bar
curvature feedback are not implemented yet; currently preselect curves before
starting either command. Mixed selections containing non-curves are rejected.
