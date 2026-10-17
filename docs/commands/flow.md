# Flow

Select source objects and enter `Flow`. Pick the base and target curves near
corresponding ends. The preview appears after both guides are selected; Enter
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

Roadlike currently supports straight guides and planar curved guides whose
axes have a component perpendicular to the curve plane. A straight guide's
axis must have a component perpendicular to its direction. Unsupported axes
are rejected before editing. Native Roadlike `Automatic` is still unimplemented;
the `CPlaneZ` convenience explicitly chooses a direction instead.

The implementation supports straight guides in 3D and planar curved guides.
General spatial curved guides, Local, Stairlike and
TargetSurface remain unfinished. Unsupported modes fail
before editing. The kernel exposes a separate mathematical spatial transport
policy, but the Flow command rejects it because it differs from Rhino.

The native comparison covers 76 point maps and 43 actual grouped point-cloud
commands at `1e-6`, including Copy, Rigid, Stretch, direction reversals and mixed
supporting planes. The command also uses the shared curve/surface/B-rep fitting
backend and cached display cages; planar NURBS guides, fitted owner geometry,
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
All match within `1e-6`; maximum coordinate error is below `6e-11`. Twelve
additional native Automatic/in-plane-axis observations remain visible as
unsupported diagnostics and are rejected locally before editing.
