# ArraySrf

`ArraySrf` (`ArraySurface`) copies objects into a U-by-V grid on a surface or
B-rep face. Start the command, select sources, pick the base point and reference
normal endpoint, select the target, and enter the U and V counts. Enter at the
normal prompt uses the construction plane axes. An explicitly picked normal
uses the shared deterministic normal-derived frame.

`Preview=Yes` displays the grid before acceptance. Adjust `Mode=UV|Isocurve`,
`UNumber` and `VNumber`, then press Enter. Escape cancels. The preview shares
source display geometry and tessellation; it creates no document objects,
groups or history. `Preview=No` accepts immediately after the V count. Counts,
mode and the preview flag are remembered across invocations and aliases.

UV mode divides the parameter domains. Isocurve mode divides the U isocurve at
the beginning of V, then divides the V isocurve separately at each chosen U.
Closed directions omit the duplicated seam. Counts of one use the corresponding
domain start. Copies are ordered by U, then V, then source order.

Targets use the complete underlying surface, including portions outside trim
boundaries. Reversed faces reflect the copied up axis while retaining the
surface X and Y axes. Public OpenNURBS surface frames handle singular boundary
limits, including sphere poles, with one owned surface transfer per batch.
An Isocurve layout whose first U isocurve collapses to a point creates no copies,
as in Rhino; a count of one in U avoids that division. Invalid frames or requests
for more than one million copied objects reject before any geometry edit.

Complete script examples:

```text
ArraySrf 4 3 BasePoint=0,0,0 Surface=surface-id Mode=Isocurve
ArraySrf 4 3 BasePoint=1,2,3 Up=0,0,1 Surface=brep-id Face=2
ArraySurface 4 3 BasePoint=0,0,0 SurfaceName=Panel Mode=UV
```

`Up` is a direction vector in complete script arguments. Omit it to use the
construction plane frame supplied by the command context. `SurfaceName` resolves
a unique name; omitting both target arguments uses the last selected object.
`Sources=id,...` and `PickedSources=id,...` freeze source sets. Explicit face
indices are zero based. A single-face B-rep needs no `Face` argument.

Copies retain source attributes. Single-source arrays allocate empty group
definitions; multiple-source arrays reproduce memberships independently at each
cell. Undo removes copies and memberships while retaining created definitions.
Preselected sources remain selected; command-first sources are released on
completion and history replay. Cancellation and no-copy layouts preserve Redo.

The [validation receipt](../array-surface-validation.json) records 46 Rhino 8
workflows, including actual getter prompts, plane/cylinder/sphere/warped targets,
trimmed and reversed faces, nonstandard domains, explicit normals, preview edits,
cancellation, literal group names, selection, and real Undo/Redo. Basis-cloud
comparisons use a `1e-6` model-unit bound. Physical face picks, cross-viewport
normal getters and every source geometry type are not covered by this corpus.

[Rhino command reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/arraysrf.htm).
