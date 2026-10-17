# Flow

Select source objects and enter `Flow`. Pick the base and target curves near
corresponding ends. The preview appears after both guides are selected; Enter
applies it and Escape cancels without editing the document. Change an option
before applying, or enter `Base` / `Target` to select that guide again.

With no preselection, Flow first asks for source objects. Enter continues to the
guide getter. Guides must be separate from the sources. Hidden, locked, deleted,
or replaced guides invalidate the preview and must be selected again.

Supported options are `Copy`, `Rigid`, `RigidGroups`, `Stretch`,
`PreserveStructure`, `ReverseBase`, and `ReverseTarget`, with `Yes` / `No` values.
Entering an option name alone toggles it during interactive input. Initial
settings are Copy=Yes, Rigid=No, RigidGroups=Yes, Stretch=No and
PreserveStructure=No. Successful commands remember deformation settings; Copy
uses the application's `RememberCopyOptions` policy. Failed or cancelled edits
do not save deformation settings.

For scripted use, supply both guide object IDs:

```text
Flow Base=12 Target=17 Copy=No Stretch=Yes
Flow BaseEdge=12,0 Target=17 Rigid=Yes RigidGroups=No
```

Use the actual IDs from the current document. `BaseEdge` and `TargetEdge` refer
to a parent object and a zero-based edge index. During interactive selection,
`SelID 12` picks a curve and `Edge=12,0` picks an edge. Typed guide references
use the direction options; mouse picks choose the nearer projected endpoint.
Explicit reversals can then change either direction.

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

The implementation supports straight guides in 3D and planar curved guides.
General spatial curved guides, Line, Local, Roadlike, Stairlike and
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
