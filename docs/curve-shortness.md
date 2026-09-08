# Short-curve selection

[Command reference](commands/document.md)

`SelShortCrv` is not simply an accurate length query followed by a comparison.
Its threshold includes a relative `1e-6` allowance. Analytic lines, arcs,
circles, and polylines use direct length formulas. NURBS use a separate,
representation-dependent predicate in `viboceros-geometry/curve_shortness`:

1. Visit nonempty parameter spans in order.
2. Estimate the next interval with three-point Gauss–Legendre integration.
3. Reject immediately if that estimate exceeds the remaining length budget.
4. Otherwise compare it with two half-interval estimates. Accept their sum when
   they agree within relative `1e-9`, or recursively refine from left to right.

Previously accepted intervals reduce the remaining budget. The traversal is
bounded to depth 24 and 65,536 refined intervals; exhaustion reports an error
before selection is changed. Scaled quadrature products avoid avoidable overflow
and underflow. Ellipses use their rational representation; polycurves share the
remaining budget across their segments. Those two dispatch paths are not yet
independently oracle-audited at the selection boundary.

This is an independently implemented approximation model, not a claim about
Rhino's proprietary implementation. It matches 136 retained classifications:
[40 lines](short-curve-selection-measurement.json),
[30 analytic/rational circles](short-curve-circle-measurement.json), and
[66 refined/elevated circles and polynomial arches](short-curve-representation-measurement.json).
The polynomial arch rules out simply using a single fixed quadrature estimate.

The predicate can reject a genuinely short curve on an overestimated coarse
interval. Knot refinement can therefore change selection without changing the
locus. It is **not** a certified length bound, and must not replace `Length`,
division, fitting, or other accuracy-controlled arc-length operations. Those
continue to use adaptive Gauss–Kronrod integration. Numerical tests verify this
distinction, finite-value validation, bounded work, and extreme constant speeds.
