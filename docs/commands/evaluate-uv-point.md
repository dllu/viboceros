# EvaluateUVPt

[Command index](README.md)

Select a surface or polysurface, then enter `EvaluateUVPt` and pick/type locations.
Without preselection, the command first opens object selection; select one eligible
object and press Enter to continue to the point prompt. Enter or Esc finishes the
session, keeping any created markers. A complete scripted invocation evaluates one
location:

```text
EvaluateUVPt Normalized=No CreatePoint=No 1,1,3
```

Coordinates in a complete command are world coordinates. Interactive input also
supports normal world/CPlane/relative point modes and snaps. Viewport clicks use
the visible surface or B-rep face and its surface hit point; typed locations use
the closest underlying surface. For a polysurface, typed locations search all
component surfaces and use the nearest, retaining face order on ties. Scripts
can fix a zero-based component with `EvaluateUVPt 1,1,3 Face=0`.
Trim loops are deliberately ignored, matching the documented underlying-surface
behavior of [Rhino's command](https://docs.mcneel.com/rhino/8/help/en-us/commands/evaluateuvpt.htm).
A location over a trim hole can therefore be evaluated.

Options, accepted before or after the point:

- `Normalized=Yes` maps each native domain to `[0,1]`. Initial default: `No`.
- `CreatePoint=Yes` creates a point at the projected surface location, not at the
  original off-surface input. Initial default: `No`.

Both options can also be changed during object selection or at the point prompt.
Invalid options leave the current point phase and previous choices intact. Accepted
options persist across invocations, documents using the same command registry,
and Undo/Redo; restarting the application resets them. Prompt cancellation does not
reset accepted options. Startup displays the remembered choices. Scripted commands
use the same preferences; specify both options for reproducible independent runs.

Without point creation, evaluation preserves geometry, selection, and model
undo/redo. A created point uses the current layer and fresh attributes; it does not
inherit the source's groups or replace its selection. All markers in one interactive
session form one undoable operation; Enter, Esc, or another command finishes it.
Query-only sessions do not open a model transaction or discard redo history.
Failed evaluations leave the prompt and previous markers available for correction.
The accepted object selection is retained when point input finishes.

Tests cover non-unit native domains, normalized values, off-surface projection,
point undo/redo, underlying evaluation inside a trim hole, nearest component
selection, UI handoff, option edits, and failed-pick recovery. Parameter normalization
handles finite domain endpoints even when their difference overflows.
The shared [closest-point solver](../surface-closest-point.md) also has regressions
for strongly stretched UV domains, derivative overflow, and distant query points.

The [Rhino 8.32 capture](../evaluate-uv-rhino-reference.json), generated from this
[fixture](../../tools/rhino_oracle/fixtures/evaluate-uv-command.json), covers all four
combinations of these options on a planar surface with U domain `[-2,6]` and V
domain `[10,14]`. The off-surface input `(1,1,3)` reports native `(0,12)` or
normalized `(0.25,0.5)` and creates `(1,1,0)` only when requested. All cases retain
the source geometry checksum. Native command tests replay the saved reports and
point locations to `1e-8`; these particular UV values are exact despite Rhino's
three-decimal display. Run the Rhino-only probe with:

```sh
tools/rhino_oracle/run_headless.sh rhino tools/rhino_oracle/fixtures/evaluate-uv-command.json --timeout 240
```

The [two-face RhinoCommon reference](../surface-face-uv-rhino-reference.json)
uses [this probe](../../tools/rhino_oracle/fixtures/surface-face-uv-api.json)
to evaluate four points against explicitly indexed underlying faces. Each point
lies nearer the other face. Native reports, normalized UV values, projected
points, and distances agree within `1e-10` (replay maximum absolute error
`4.44e-16`). This checks the public geometry API;
it does not measure Rhino's interactive face click behavior. The probe ran on a
separate Xvfb display and released its Rhino process afterward.
The [curved-face reference](../surface-face-uv-curved-rhino-reference.json)
adds a rational cylindrical face and a quadratic face with non-unit domains.
Its six indexed queries agree within `1e-10` (maximum absolute replay
difference `3.13e-12`), including command-level normalized and projected-point
checks. It also used a separate Xvfb display.

The [session capture](../evaluate-uv-session-rhino-reference.json) and
[session fixture](../../tools/rhino_oracle/fixtures/evaluate-uv-session.json) add
repeated picks, option changes between points, Enter/Esc completion, and subsequent
invocations with no explicit options. App tests replay all reports and created
point locations to `1e-8`. Esc retains the measured markers, and both options carry
over after Enter and Esc. The captured Undo/Redo attempts reported “Nothing to
undo/redo” inside the Python probe: their unchanged point arrays are diagnostic,
not evidence of Rhino undo grouping. Native tests separately verify one undo/redo
step for the complete marker session and preservation of redo during pure queries.

These captures are not timing benchmarks (`elapsed_ns` is zero). Trim holes have
native analytic coverage; viewport face selection has native tests, but no live
Rhino click comparison. Typed closest-surface lookup uses
model-space distance. Rhino parity for interactive picking, initial defaults,
and report text formatting is not established. The worker deletes its temporary
geometry and restores the previous
selection, including on failure. The session fixture must run in order to measure
option inheritance; its first operation explicitly seeds both choices.
