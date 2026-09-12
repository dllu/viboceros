# Angle, Distance, Length, Area, and Volume

[Command index](README.md)

## Angle

Enter `Angle 0,0,0 1,0,0 4,5,6 4,6,6` to measure the angle between two
directions, or enter `Angle` with no selection and pick/type the four endpoints in order. The
result is an unsigned 3D angle in degrees in `[0,180]`, independent of CPlane
orientation and the separation between the two lines. Reversing one direction
changes the result to its supplement. The query does not alter geometry,
selection, or model undo/redo. Esc cancels interactive input.

Both directions must be nonzero. Range-safe endpoint subtraction and
normalization also handle finite endpoints whose differences or norms would overflow. The
cross/dot `atan2` formula retains small angles that a rounded-dot `acos` would
lose; no document-distance tolerance is used to reject a nonzero direction.
An invalid second or fourth picked point leaves its prompt active for correction.

This implements the four-point workflow in [Rhino's Angle documentation](https://docs.mcneel.com/rhino/8/help/en-us/commands/angle.htm).
`SubCrv` is not implemented. Seven live Rhino
8.32.26160.13001 probes cover parallel, opposite, perpendicular, acute, obtuse,
and spatial directions, including a rotated/translated CPlane. The
[captured reports](../angle-rhino-reference.json) agree with native command
results within Rhino's three-decimal display precision. The probes do not
establish parity for degenerate inputs or extreme numeric ranges. Reproduce with:

```sh
tools/rhino_oracle/run_headless.sh rhino tools/rhino_oracle/fixtures/angle-command.json --timeout 240
```

The capture shares bounded command-history extraction and CPlane restoration
with the Distance probe; it creates no geometry and reports no performance timing.
Kernel and command tests additionally cover analytic angles, small
angles, direction magnitudes from `1e-300` through `1e300`, interactive rejection,
and unchanged document/history state.

Separate [object-mode captures](../angle-objects-rhino-reference.json) establish
the orientation policy for `TwoObjects` mode:
two preselected lines whose directed angle is 135 degrees report 45 degrees;
reversing one line still reports 45. Planar surfaces with opposed/acute normal
orientations likewise both report 45. Thus the directed four-point formula must
not be reused unchanged for these objects. A mixed line/plane case reports 45,
but that symmetric input does not distinguish a normal angle from its complement.
The preselection macro is simply `_Angle`: adding `_TwoObjects` after it becomes
an unknown command because Rhino has already completed the measurement.

```sh
tools/rhino_oracle/run_headless.sh rhino tools/rhino_oracle/fixtures/angle-objects-command.json --timeout 240
```

This oracle-only probe creates two temporary objects, deletes its owned objects,
and restores prior selection even on failure. Mock tests cover partial
construction and measurement failures.

Use `Angle TwoObjects` to select two objects, or select two objects first and
enter `Angle`. Supported inputs are straight curves (native lines or curves
whose exact NURBS representation passes the kernel's linearity check) and
planar NURBS surfaces or single-face B-reps. Planarity and linearity use document
tolerance. Curved/nonplanar geometry, multi-face B-reps, and selections other
than exactly two objects are rejected. Object picking changes selection as usual;
the measurement itself preserves selection and geometry and creates no undo step.
Failed postselection measurements keep the selection prompt open.

Line/line and plane/plane results are the acute unoriented angle in `[0,90]`.
Mixed inputs report the angle between the line and the plane, not its normal.
Additional [mixed-object captures](../angle-mixed-rhino-reference.json) establish
the distinction: a line with direction `(1,0,2)` and an XY plane report `63.435`,
a parallel line reports zero, and a normal line reports 90. Native regressions
cover both selection orders, small acute angles, single-face B-reps, linear
NURBS curves, rejection paths, and these reported values.

```sh
tools/rhino_oracle/run_headless.sh rhino tools/rhino_oracle/fixtures/angle-mixed-command.json --timeout 240
```

## Distance

Enter `Distance 0,0,0 3,4,0`, or enter `Distance` and pick or type two points.
The report includes distance in document units, world and active CPlane axis
deltas, signed XY azimuth in degrees in `[-180,180]`, and elevation in `[-90,90]`.
Coincident points report zero distance and zero angles; vertical displacements
use zero azimuth. Full command arguments are world coordinates; interactive
typed points use the usual point-input coordinate modes. Selection is not needed
or changed, and the query creates no geometry or history entry. Esc cancels point
picking. Non-finite inputs and unrepresentable distances are rejected.
The accepted first point remains marked in each viewport while typing options;
the shared drafting renderer draws a dashed segment to the live cursor.
Clearing the first point with local `Undo`, completing, or cancelling removes
the accepted-anchor marker.
At the second-point prompt, `Undo` revises the first point without invoking
document undo. It restores the relative-coordinate anchor from before the
measurement and releases the first point's captured drafting plane. Both typed
command entry and direct command dispatch use this local input revision;
existing model undo/redo entries remain intact. The first-point prompt has no
local `Undo` option.

The reporting categories follow [Rhino's Distance documentation](https://docs.mcneel.com/rhino/8mac/help/en-us/commands/distance.htm).
For a display-only conversion, use a trailing option such as
`Distance 0,0,0 25.4,0,0 Units=Inches`. It accepts the standard unit names and
abbreviations used by `Units`, including British spellings. The override scales
the reported distance and axis deltas, appends the target unit name, and leaves
angles and the document's unit setting unchanged. Custom physical source units
are supported; unitless/unset sources or targets, overflowing results, and
nonzero values that underflow to zero are rejected. Omit the option to report
unchanged document coordinates, including unitless models. The override is
normally applied after measurement. If the source-unit distance overflows and
the requested conversion reduces magnitude, an exact scaled-projection fallback
converts before rounding and measures the finite display-space displacement.
Tests cover overflowing coordinate differences and overflowing norms separately.
The override is available in complete typed commands and at either interactive
point prompt: enter `Units=Inches` (or another physical unit), then continue
picking. `Units=Model_Units` clears the override. Unit choices preserve accepted
points. You can also start with `Distance Units=Inches`; underscore-prefixed
macro forms such as `_Distance _Units=_Inches` are accepted. Complete commands
containing both coordinates still execute directly instead of opening a prompt.
Unit choices survive local `Undo`, but reset for each new measurement. Invalid
choices leave the prompt unchanged; failed final conversions retain the first
point so units or the second point can be corrected.
Nesting a measurement inside another command's numeric prompt is not implemented.
Six live command probes on Rhino 8.32.26160.13001 cover positive deltas,
negative X/Y, vertical up/down, and a translated/rotated CPlane. The
[captured output](../distance-rhino-reference.json) established the signed
azimuth convention and supplies regression values at Rhino's printed precision.
Exact text formatting, coincident points, and signed-zero branch-cut cases are
not covered by this comparison. Reproduce the Rhino-only capture with:

```sh
tools/rhino_oracle/run_headless.sh rhino tools/rhino_oracle/fixtures/distance-command.json --timeout 240
```

This probe captures public command history, restores its CPlane on success or
failure, and reports no performance timing. It runs in an oracle-owned session.

## Selected-object measurements

Select objects, then enter `Length` (alias `Len`), `Area`, or `Volume` without
arguments. Each command reports the selected count and aggregate measurement.
They do not change geometry, selection, or undo/redo history. An empty selection,
unsupported object, invalid measurement, or numerical failure rejects the query
without reporting a partial total.

Totals use round-trip decimal digits, rather than a fixed twelve decimal places.
Nonzero magnitudes below `1e-6` or at least `1e12` use scientific notation; zero
is displayed as `0`. Small valid measurements therefore remain visible, and
parsing the displayed number recovers the computed binary64 total. Extra digits
preserve the computed value, not a claim of accuracy beyond the geometry kernel's
tolerances.

| Command | Supported geometry |
| --- | --- |
| Length | Lines, circles, arcs, ellipses, polylines, NURBS curves, and polycurves |
| Area | Circles, ellipses, closed planar polylines/NURBS/polycurves, NURBS surfaces, B-reps, and meshes |
| Volume | Closed meshes and solid B-reps |

NURBS/B-rep measurements use the geometry kernel's accuracy-controlled routines,
not viewport tessellation. General curve area uses `CurveRef::planar_area`,
which constructs a temporary validated planar face retaining the exact rational
boundary. This face is never added to the document. Open, nonplanar, or invalid
general boundaries are rejected. A checked unit-domain copy prevents failures
caused solely by extreme NURBS parameter scales; controls, weights, and the
stored source remain unchanged. Polycurve outer domains are normalized before
NURBS conversion, so a leaf's internal knots are not first squeezed into an
outer interval with too few representable values. This also leaves the stored
composite and its independent leaf domains unchanged. The shared polycurve
integration frame also normalizes temporary NURBS leaves before conversion.
Collapsed knot intervals
are not silently lost; normalization does not guarantee that every possible
relative span size or extreme leaf domain can be converted.
Self-intersecting winding-area semantics are
not established. Separate selected curves contribute separate areas, not holes
in one region. A lower-overhead standalone boundary-integral path remains future
work. Volume is signed: reversing orientation reverses its contribution,
so oppositely oriented objects can cancel. Open meshes/B-reps are rejected for
volume. Length and area contributions must be nonnegative.

`viboceros-command/measurements` streams selected objects without building a
temporary selection vector. All three commands share an allocation-free
[exact finite-value accumulator](../numerical-sums.md), with one final rounding.
Overflowing intermediate totals may cancel to a finite result without losing
small contributions. Non-finite input values or rounded totals are errors rather
than formatted infinities. This improves aggregation, not the accuracy of each
underlying geometry measurement.

Tests cover mixed analytic lengths/areas, surface and B-rep area, signed mesh
and B-rep volumes, rational circles and mixed polynomial/line polycurves,
small terms amid large contributions, and unchanged selection
and both history stacks after failed queries. The [curve-area oracle](../curve-area.md)
records analytic checks and the current differences from Rhino's public API.
