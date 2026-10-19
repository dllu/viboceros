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
`RemoveControlPoint index Direction=U|V` API. Open standalone curve charts start at zero
after command removal; surfaces retain their UV charts. Open polylines receive
new vertex-index parameters. Quadratic closed seams and periodic controls use
the shared cyclic knot utilities. Trimmed surface boundaries use the existing
certified surface editor.

Composite curves expose shared junction grips once. Removing a control edits
its owning leaf while retaining the parent intervals and leaf domains. Removing
an outside control of a two-control end segment drops that segment. Removing its
shared junction also moves the adjacent leaf to the remaining endpoint, using
the existing analytic arc or NURBS endpoint editor. In the captures, picks whose
closest parameter is exactly a junction select a control in the preceding
leaf's local chart using the parent parameter, with a first-control fallback
outside that chart.
Composite container geometry text is preserved along with object metadata.

The [validation receipt](../remove-control-point-validation.json) records both
backends, UI/API checks, known differences, and proof hashes.
Qualification uses actual Rhino 8 commands and source 3dm files in
`tools/rhino_oracle/observations/remove_control_point_command.json`, including
polynomial and rational curves, periodic curves and tensors, circle and ellipse
seams, open polylines, transformed inputs, custom domains, negative weight
gauges, surface control grids, trims, postselection, no-ops and history replay.
Of 133 captures, 122 agree, one covers an unsupported polysurface, and 10
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

An additional 22 composite command captures cover reversed and transformed
curves, custom parent charts, shared junctions and nearby picks, empty input,
postselection, cancellation and history. All agree in both UI and API replays
with both backends. Nineteen public parameter traces reproduce the same native
results. A separate 120-case surface neighborhood capture reproduces sensitivity
to coordinate changes as small as `1e-13`; it is diagnostic evidence, not 120
additional agreement passes. Ten surface and isocurve parameter-query checks
agree within `1e-8`, while the ten command getter differences remain unresolved.

Polysurfaces are explicitly rejected. Closed composite seam removal and edits
that would disconnect an interior junction are also rejected. General nested/composite junctions,
non-quadratic closed seams, all periodic degrees and knot layouts, high-DPI and
all camera configurations, edited-file roundtrips and general performance are
not qualified by this matrix. Projected control hulls must lie in front of the
camera. The existing numeric API remains unchanged.

Rhino reference: [McNeel command help](https://docs.mcneel.com/rhino/8/help/en-us/commands/removeknot.htm).
The captured Rhino version ends this command after a single point, even though
the help page also mentions pressing Enter to complete it.
