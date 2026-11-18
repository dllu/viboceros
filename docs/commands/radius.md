# Radius and Diameter

[Command index](README.md)

Run `Radius` or `Diameter` and move the pointer over a curve. The viewport
marks the evaluation point and the status bar reports the current value.
Click to report both radius and diameter in the command log. Pick more
locations or press Enter to finish. Esc ends the session.

Without preselection, the viewport looks for visible, selectable curves
within the pick aperture. Preselect curves to limit the search. `SelectCurve`
opens a one-curve picker. Once chosen, the pointer is constrained to the curve
and the viewport highlights it. Press Enter in the curve picker to make all
eligible curves available again. Use `SelectCurve`, then `SubCrv`, to measure a
temporary portion. Pick a start and end or enter a length and confirm its
direction. The measured portion stays private to the command.

`Radius 2,0,0` and `Diameter 2,0,0` perform a one-shot model-space query
at the closest location on an eligible curve. A bare command also reports
a single preselected circle or circular arc immediately. That shortcut
includes NURBS curves recognized by whole-span circularity bounds.

These are local curvature measurements, not circle-fitting operations: radius
is the reciprocal curvature magnitude and diameter is twice that value. Lines,
polylines, and zero-curvature locations report `infinite`. Other supported inputs
are arcs, circles, ellipses, NURBS curves, and polycurves, using the same
differential evaluation as `Curvature`. A finite result outside the representable
numeric range is an error, not an infinite-curvature-radius report.

`MarkRadius` and `MarkDiameter` toggle the creation of a point and an
osculating circle. Straight or zero-curvature locations receive a point.
Markers use the current layer and preserve source selection. All marked picks
in one viewport session form one undo step. The toggle is remembered
independently for each command. Scripted calls can specify `MarkRadius=Yes`
or `MarkDiameter=Yes` before the point.

Use `Units` to open the display-unit menu or enter `Units=cm` directly.
`Model_Units` reports the original model lengths. The interactive choice is
remembered independently for each command. `Decimal_Inches`, `Decimal_Feet`,
`Fractional_Inches`, and `Feet_Inches` are accepted. Fractional formats display
to the nearest eighth of an inch. Unit conversion requires physical model
units and leaves the model settings unchanged. The Units option is disabled
for explicit object preselection.

At the numeric radius or distance prompt in `Fillet`, `Offset`, or `OffsetSrf`,
enter `Radius` or `'Radius` and pick a curve location. A finite result fills
the parent prompt in model units. Esc returns to that prompt with its previous
value.

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

Headless Rhino 8.32 viewport probes also verified repeated measurements,
curve restriction and temporary portions, marking on both curved and linear
inputs, and remembered unit and marker choices. The viewport cases used an
ellipse with semiaxes 4 and 2 and reported radii of 1.000 and 8.000 at the
axis ends. Rhino reported 3/8 inch and 5/8 inch for the radius and diameter in
fractional format at the minor-axis end of that millimeter model. Egui
pointer tests exercise the same getters and verify preview-to-pick agreement,
marker undo, and return to the parent numeric prompt.
A native Fillet probe measured the ellipse through `'Radius` and returned to
Fillet with its radius set to 8.

[McNeel's Radius and Diameter reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/diameter.htm)
documents the units pane, curve restriction, and nested measurement workflow.
