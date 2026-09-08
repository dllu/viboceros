# Length, Area, and Volume

[Command index](README.md)

Select objects, then enter `Length` (alias `Len`), `Area`, or `Volume` without
arguments. Each command reports the selected count and aggregate measurement.
They do not change geometry, selection, or undo/redo history. An empty selection,
unsupported object, invalid measurement, or numerical failure rejects the query
without reporting a partial total.

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
temporary selection vector. All three commands share compensated summation;
non-finite values and totals are errors rather than formatted infinities. The
accumulator is not arbitrary precision: overflowing intermediate sums remain
an error even if later signed cancellation could yield a finite result.

Tests cover mixed analytic lengths/areas, surface and B-rep area, signed mesh
and B-rep volumes, small terms amid large contributions, and unchanged selection
and both history stacks after failed queries.
