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

Exception: bare `Radius` or `Diameter` immediately reports a single preselected
circle or circular arc. This includes NURBS curves recognized by whole-span
circularity bounds, not just native primitives. Non-circular or inconclusive
NURBS curves still require a point. Explicit marking options also retain point
input so the marker location is unambiguous. The shortcut does not create markers.

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
The [live Rhino 8.32.26160.13001 reference](../radius-rhino-reference.json)
independently evaluates rational quadratic circular/elliptical arcs through
RhinoCommon `ClosestPoint` and `CurvatureAt`. Its radii are approximately 2, 1,
and 8; the existing native analytic/NURBS tests agree within `1e-10` model units.
The line returns zero curvature; JSON `null` radius/diameter denotes infinity.
Only the circular-arc case includes actual command text, reporting radius 2 and
diameter 4. The ellipse and line values are public-API evidence, not command captures.
The probe's zero elapsed times are not performance measurements.

```sh
tools/rhino_oracle/run_headless.sh rhino tools/rhino_oracle/fixtures/radius-command.json --timeout 240
```

Live prompt inspection also found that Rhino immediately reports a preselected
circular arc's radius, whereas a preselected line still opens the picking prompt.
The native UI now implements that distinction for one preselected curve. Rhino's unrestricted
picker did not consume the world-coordinate token used by the probe, so this
fixture does not establish parity for typed versus mouse-picked input. The probe
cleans up its temporary source and restores prior selection, including failures;
mock tests exercise construction and command-capture failures.

[Rhino's reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/radius.htm)
also describes cursor feedback, `SelectCurve`, display `Units`,
`SubCrv`, and nested numeric input. Those workflows and live hover/status-bar
curvature feedback are not implemented yet. Point lookup currently uses 3D
nearest distance, not a screen-space hit aperture. Explicit mixed selections
containing non-curves are rejected.
