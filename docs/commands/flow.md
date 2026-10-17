# Flow

Select source objects and enter `Flow`. Pick the base and target curves near
corresponding ends. The preview appears after both guides and any Roadlike axes
are supplied; Enter
applies it and Escape cancels without editing the document. Change an option
before applying, or enter `Base` / `Target` to select that guide again.

At either guide prompt, enter `Line` and pick two endpoints instead of selecting
a curve. The standard point input supports snapping, world coordinates and
relative coordinates such as `r10,0,0` for the second endpoint. The drawn line
is temporary: preview, commit, Undo and Redo never add guide objects to the model.
Its endpoint order determines its direction. A short or coincident pair asks for
another endpoint; Escape cancels the command.

With no preselection, Flow first asks for source objects. Enter continues to the
guide getter. Guides must be separate from the sources. Hidden, locked, deleted,
or replaced guides invalidate the preview and must be selected again.

Supported options are `Copy`, `Rigid`, `RigidGroups`, `Stretch`,
`PreserveStructure`, `ReverseBase`, `ReverseTarget`, and `Roadlike`, with `Yes` / `No` values.
Entering an option name alone toggles it during interactive input. Initial
settings are Copy=Yes, Rigid=No, RigidGroups=Yes, Stretch=No and
PreserveStructure=No. Successful commands remember deformation settings; Copy
uses the application's `RememberCopyOptions` policy. Failed or cancelled edits
do not save deformation settings.

For scripted use, supply both guide object IDs:

```text
Flow Base=12 Target=17 Copy=No Stretch=Yes
Flow BaseEdge=12,0 Target=17 Rigid=Yes RigidGroups=No
Flow BaseLine=0,0,0;10,0,0 TargetLine=5,3,0;5,23,0 Copy=No
Flow Line 0,0,0 10,0,0 Target=17 Stretch=Yes
Flow Base=12 Target=17 Roadlike=Yes BaseUp=0,0,1 TargetUp=0,1,0
```

Use the actual IDs from the current document. `BaseEdge` and `TargetEdge` refer
to a parent object and a zero-based edge index. During interactive selection,
`SelID 12` picks a curve and `Edge=12,0` picks an edge. Typed guide references
use the direction options; mouse picks choose the nearer projected endpoint.
Explicit reversals can then change either direction.
`BaseLine` and `TargetLine` each take two coordinate triples separated by a
semicolon. A scripted `Line` followed by two points supplies the next missing
guide; it can be used twice. Drawn guides can be combined with selected curves
or surface edges.

Flow maps distance along the base onto the target, carrying the two transverse
coordinates. Stretch scales distance and the along-tangent residual at source
endpoints. It preserves transverse size. Rigid maps each object's bounds center
and rotates it without deforming its geometry. RigidGroups uses one pose for
selected members of each top group. Copy retains attributes and creates
corresponding groups. Both guides stay unchanged. One Undo reverses the edit.

Select control points (`PointsOn`) or curve edit points (`EditPtOn`) before Flow,
or pick them in its initial source getter. Only selected grips move in their
current basis; selecting their parent as well does not deform the whole owner.
Rigid applies to ordinary objects, while each grip follows the point map.
RigidGroups includes grip-owner bounds when placing ordinary group peers.
Copies of grip owners join their source groups, retain the grip mode, and leave
copied grips unselected. Unselected grips and the source knots, degrees and
weights remain unchanged. Undo/Redo restores owner geometry and the native
selection state, including grips picked during the command.

With Roadlike enabled, pick an axis start and direction for each guide after
selecting both guides. Enter or `CPlaneZ` at either axis prompt uses the active
construction plane's Z direction as an explicit axis. Standard snapping, world
and relative point input apply. Enter `BaseUp` or `TargetUp` to pick that axis
again. Scripted commands require both `BaseUp=x,y,z` and `TargetUp=x,y,z` in
world coordinates. Preview and commit use the same chosen axes. A new
interactive command asks for axes again.

NURBS curves and polycurves can supply either or both guides. For spatial
curved guides, enable Roadlike and choose both axes; `CPlaneZ` supplies a
convenient explicit direction. Planar curved guides also work with Roadlike off.

Roadlike axes may lie in a curved guide's plane or be parallel to a straight
guide. At an exact pole, ordinary Flow collapses both transverse coordinates
onto the guide while retaining the along-tangent residual. Rigid keeps the
original object pose if the pole cannot supply a placement frame. Both behaviors
share the command's staged owner/grip edit and preview paths. Native Roadlike
`Automatic` remains unimplemented; `CPlaneZ` explicitly chooses a direction.

The registered command rejects automatic orientation of general spatial curved
guides. Local, Stairlike and TargetSurface remain unfinished. Unsupported modes
fail before editing.

The native comparison covers 76 point maps and 43 actual grouped point-cloud
commands at `1e-6`, including Copy, Rigid, Stretch, direction reversals and mixed
supporting planes. The command also uses the shared curve/surface/B-rep fitting
backend and cached display cages; further fitted owner geometry,
multiple-object rigid groups, native history details, preview appearance and performance still need
Flow-specific native qualification. Raw Rhino table enumeration order is not
part of the current comparison. See [development evidence](../flow-development.md)
and [validation record](../flow-command-validation.json).

The [grip validation record](../flow-grips-validation.json) covers 72 additional
native command workflows: rational curve controls/edit points, weighted surface
controls and mesh vertices, with Copy, Rigid/RigidGroups, PreserveStructure,
selected parents, ordinary peers and command-first grip selection. It compares
complete owner definitions, grips, groups and real Undo/Redo at `1e-6`.

The [Line validation record](../flow-line-validation.json) adds 148 actual native
commands and four SDK orientation diagnostics. It covers either or both drawn
guides, mixed supporting planes, rational control/edit grips, rigid curve owners,
parallel and opposite guides, temporary-guide lifetime and real Undo/Redo.
All match within `1e-6`; the largest coordinate difference is below `9e-11`.

The [Roadlike validation record](../flow-roadlike-validation.json) adds 40 actual
native commands covering explicit axes, CPlane-Z presets, mixed planes,
direction reversals, Copy/Rigid/Stretch, rational grips and full Undo/Redo.
All match within `1e-6`; maximum coordinate error is below `6e-11`. The eight
formerly unsupported in-plane cases now match; four native Automatic cases
remain guarded.

The [pole validation record](../flow-poles-validation.json) qualifies another
36 native workflows and the eight formerly blocked in-plane cases at `1e-6`.
It covers exact/near poles, other planes, both reversals, closed circles,
polyline corners, source/target parallel axes, Rigid and control/edit grips.
The four rational NURBS workflows and two SDK lookup diagnostics now match too.
Their former precision differences are resolved by the independent incremental
lookup described in the [NURBS validation record](../flow-nurbs-validation.json).

That record additionally covers 72 native point maps and 132 complete command
workflows. NURBS and composite guides appear as the base, target and both guides,
with direction reversal, Copy, Stretch, groups and real Undo/Redo. Spatial NURBS
guides use explicit Roadlike axes; the spatial matrix also checks Rigid poses.
All comparisons retain the document's `1e-6` tolerance. Automatic spatial
orientation and native Roadlike Automatic remain open requirements.
