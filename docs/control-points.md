# Control-point display and selection

Select a curve, surface, mesh or SubD and run `PointsOn`. Its control points appear as
purple squares; selected points appear orange. `PointsOff` hides all control
points and clears grip selection while retaining object selection. These display
commands preserve model geometry, Undo and Redo.

[`PtOffSelected`](commands/pt-off-selected.md) closes displays on selected owners
only, preserving unrelated object and grip picks. It also supports command-first
owner selection. Starting this command discards Redo, including on Cancel;
it adds one unchanged-command Undo step. Replaying that step leaves the point
display unchanged.

`HidePt` hides selected visible grips, or opens a grip picker when none are
selected. `ShowPt` reveals hidden grips on enabled displays. Hidden grips keep
their original indices and locations; they do not draw or participate in mouse,
window, SelPt or ordinary transform input. Logical U/V navigation can still
select them. See [point visibility](commands/point-selection.md#hidept-and-showpt)
for the command, mode-switch and Undo rules.

[`EditPtOn`](commands/edit-points.md) displays curve Greville points or SubD limit stations through
the same picking and transform workflow. `PointsOn` switches back to control
points; switching modes clears the owner's selected indices. `PointsOff` hides
either mode. Grip mode is stored with the display snapshot and history.

[`Delete`](commands/delete-grips.md) removes selected visible control or edit
points. Complete surface rows can be removed in either parameter direction;
mesh picks dissolve their incident face sets. Mixed grip and object deletion
shares one Undo step. Selecting every grip removes its owner.

The [point-selection commands](commands/point-selection.md) select, invert or
step through displayed controls from the keyboard, including U/V rows and
additive neighbors. They also work during grip-capable
source selection, including SetPt, Move, Smooth and Circle FitPoints.

Click or drag a selection window to select visible control points. Shift adds
and Ctrl removes picks. Global `SelAll` includes displayed control points and
their parent objects; FitPoints ignores parent geometry as a fitting input.
`SelNone` clears picks while retaining display. Escape at the Circle picker
cancels its definition and clears picks while retaining display.
At `Circle FitPoints`, ordinary picks add to the definition until Enter accepts
it. Point objects and control points can be mixed in one fit. Duplicate mesh
vertices retain separate identities and equal contributions; rational NURBS
weights do not weight their Euclidean grip locations.

The document stores transient point identities as `(object ID, grip index)`.
Control locations share an immutable geometry snapshot, so display and fitting
use the same definition. Geometry replacements refresh locations and invalidate
old index picks. Hidden or locked owners do not contribute visible/selectable
control points. This state is not serialized as model geometry.

Preselected grips remain selected after fitting and through Undo/Redo. Grips
picked during the Circle command are deselected when the getter finishes;
their display remains enabled. Displays can survive removing and restoring a
source, including cancelled and collinear no-output fits. `PointsOff` keeps
displays closed through replay. Circle's preselected object peers retain their
selection through Undo/Redo.

[Curve closure and shared seam grips](curve-closure.md) describes near-coincident
endpoints, point extraction, composite edits and representation-preserving no-ops.

## Evidence and remaining work

Sixteen Rhino 8.32.26160.13001 workflows, captured in private Xvfb, cover curve,
rational curve, surface and mesh grips, mixed point objects, insufficient picks,
cancellation, collinearity, duplicate vertices, and `PointsOn`, `SelAll` and
`PointsOff`. Application tests replay source locations, selection, Circle locus
and Undo/Redo through direct and incremental input. Mouse tests exercise clicks
and a window containing both grips and point objects.
The captures retain both the Circle EndCommand state and the final macro state:
postselection cleanup runs between them. Completed fits receive no trailing
Escape tokens, which would otherwise interfere with display and selection.

[Affine grip transforms](grip-transforms.md) add partial edits, copies and
display-state history with bounded closed/periodic seam support. Native overlap
menus, complete periodic/closed/composite grip behavior,
idle Escape display cleanup, arbitrary source replacements, and large control-net display performance need
further implementation or verification. Oriented Circle frames remain a
[separate compatibility gap](circle-fit-input.md).

[Provenance](circle-fit-grips-provenance.json) · [Command reference](commands/README.md)

`Weight` edits rational curve and surface control weights with a live preview,
command-line input, and Undo/Redo. See [Weight](commands/weight.md).

[InsertEditPoint](commands/insert-edit-point.md) adds edit stations without
changing the curve shape, with repeated picking, temporary portions and Undo/Redo.
