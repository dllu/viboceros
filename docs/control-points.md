# Control-point display and selection

Select a curve, surface or mesh and run `PointsOn`. Its control points appear as
purple squares; selected points appear orange. `PointsOff` hides all control
points and clears grip selection while retaining object selection. These display
commands preserve model geometry, Undo and Redo.

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

This adds display/picking and Circle input. Editing grips with transforms or
Delete, native overlap menus, complete periodic/closed/composite grip behavior,
idle Escape display cleanup, arbitrary source replacements, and large control-net display performance need
further implementation or verification. Oriented Circle frames remain a
[separate compatibility gap](circle-fit-input.md).

[Provenance](circle-fit-grips-provenance.json) · [Command reference](commands/README.md)
