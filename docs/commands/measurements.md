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
| Area | Circles, ellipses, planar closed polylines, NURBS surfaces, B-reps, and meshes |
| Volume | Closed meshes and solid B-reps |

NURBS/B-rep measurements use the geometry kernel's accuracy-controlled routines,
not viewport tessellation. Area does not yet support general closed NURBS curves
or polycurves. Volume is signed: reversing orientation reverses its contribution,
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
and B-rep volumes, small terms amid large contributions, and unchanged selection
and both history stacks after failed queries.
