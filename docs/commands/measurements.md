# Length, Area, and Volume

[Command index](README.md)

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
composite and its independent leaf domains unchanged. Collapsed knot intervals
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
