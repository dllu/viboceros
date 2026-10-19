# RemoveControlPoint

Run `RemoveControlPoint`, select one curve or single surface, and move the
pointer over its controls. The highlighted control or row is removed when you
click. The command then ends; ordinary `Undo` restores the edit. Esc before
picking discards the getter without editing geometry or losing redo history.

For surfaces, `Direction=U|V` names the row orientation: U removes a V-axis
index. `Toggle` swaps directions. A direction containing only two controls
forces removal on the other axis. Open boundary rows are retained. Picking uses
the projected control polygon, including the underlying net of trimmed faces.
The remembered direction belongs to this command independently of RemoveKnot.

Typed model points use the same picker. Scripts can use:

```text
RemoveControlPoint Points Direction=V 4,3,1
```

This form accepts one point. `remove_control_point::Draft` exposes control
markers and the highlighted row without changing the document. Its commit
validates source ownership and tolerance; a successful edit is one history item
and retains object identity, attributes, and group membership.

Command removal keeps rational weights. It differs from deleting selected
grips, whose endpoint normalization remains available through the existing
`RemoveControlPoint index Direction=U|V` API. Open curve charts start at zero
after command removal; surfaces retain their UV charts. Open polylines receive
new vertex-index parameters. Quadratic closed seams and periodic controls use
the shared cyclic knot utilities. Trimmed surface boundaries use the existing
certified surface editor.

The [validation receipt](../remove-control-point-validation.json) records both
backends, UI/API checks, known differences, and proof hashes.
Qualification uses actual Rhino 8 commands and source 3dm files in
`tools/rhino_oracle/observations/remove_control_point_command.json`, including
polynomial and rational curves, periodic curves and tensors, circle and ellipse
seams, open polylines, transformed inputs, custom domains, negative weight
gauges, surface control grids, trims, postselection, no-ops and history replay.
Of 133 captures, 112 agree, 11 cover unsupported composite inputs, and 10
retain known surface getter differences in
`crates/viboceros-command/tests/support/remove_control_point.rs`. In nine of these ten cases Rhino
leaves the surface unchanged after an interior pick; in the remaining case it
removes an end row that Viboceros retains. These are not geometry agreement passes.
Workflow invariants and undo/redo are still checked for those ten cases, and
an unexpected failure or a change in the discrepancy set fails the regression.
The captures use explicit world-coordinate command macros, with no new native
mouse-event matrix. Tests compare retained definitions, sampled geometry and recorded boundary
endpoints at `1e-8`. The shared boundary certificate uses the document tolerance;
the captures do not contain complete native spatial edge definitions.

Interactive polycurves and polysurfaces are explicitly rejected in this
increment. Their captured observations remain in the matrix as unsupported
cases, separate from agreement checks. General nested/composite junctions,
non-quadratic closed seams, all periodic degrees and knot layouts, high-DPI and
all camera configurations, edited-file roundtrips and general performance are
not qualified by this matrix. Projected control hulls must lie in front of the
camera. The existing numeric API remains unchanged.

Rhino reference: [McNeel command help](https://docs.mcneel.com/rhino/8/help/en-us/commands/removeknot.htm).
The captured Rhino version ends this command after a single point, even though
the help page also mentions pressing Enter to complete it.
