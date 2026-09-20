# EvaluateUVPt

[Command index](README.md)

Select a surface or polysurface, then enter `EvaluateUVPt` and pick/type a location.
Without preselection, the command first opens object selection; select one eligible
object and press Enter to continue to the point prompt. Scripted form:

```text
EvaluateUVPt Normalized=No CreatePoint=No 1,1,3
```

Coordinates in a complete command are world coordinates. Interactive input also
supports normal world/CPlane/relative point modes and snaps. The command reports
the closest underlying surface's native U/V parameters. For a polysurface it
searches component surfaces and uses the nearest, retaining face order on ties.
Trim loops are deliberately ignored, matching the documented underlying-surface
behavior of [Rhino's command](https://docs.mcneel.com/rhino/8/help/en-us/commands/evaluateuvpt.htm).
A location over a trim hole can therefore be evaluated.

Options, accepted before or after the point:

- `Normalized=Yes` maps each native domain to `[0,1]`. Default: `No`.
- `CreatePoint=Yes` creates a point at the projected surface location, not at the
  original off-surface input. Default: `No`.

Both options can also be changed during object selection or at the point prompt.
Invalid options leave the current point phase intact. These defaults apply to each
new invocation; option persistence between invocations is not implemented.

Without point creation, evaluation preserves geometry, selection, and model
undo/redo. A created point uses the current layer and fresh attributes; it does not
inherit the source's groups or replace its selection. Creation is one undoable
operation. Esc cancels point input, preserving the accepted object selection, and
failed evaluations leave the prompt available for correction.

Tests cover non-unit native domains, normalized values, off-surface projection,
point undo/redo, underlying evaluation inside a trim hole, nearest component
selection, UI handoff, option edits, and failed-pick recovery. Parameter normalization
handles finite domain endpoints even when their difference overflows.

These are native analytic tests, not live Rhino captures. Closest-surface lookup
uses model-space distance rather than a screen-space hit aperture; exact interactive
picking, persistent defaults, and reporting-format parity remain to be verified.
