# Selection, attributes, layers, and groups

[Command reference](README.md) · [Project overview](../../README.md)

## Object organization

`Hide` and `Lock` change selected objects; `Show` and `Unlock` restore every
object with the corresponding object-level state. Hidden objects neither render
nor snap. Locked objects render in gray and remain available to osnap, but
cannot be selected or edited. Layer visibility and locking remain independent.

`HideSwap` exchanges normal and hidden object modes, while `LockSwap` exchanges
normal and locked modes. Like Rhino, both swaps affect only objects on visible,
unlocked layers and leave the third object mode unchanged.

`Isolate` hides ordinary objects outside the selection and `IsolateLock` locks
them; objects already hidden or locked and objects on hidden or locked layers
are unchanged. Their `Unisolate` counterparts restore only modes introduced by
the matching isolate command, with provenance preserved through undo and redo.
Rhino-compatible curve, line, polyline, point, point-cloud, surface, and
polysurface, and open/closed mesh filters add visible, unlocked objects of the
requested type to the current selection. `SelPtCloud` is separate from `SelPt`,
matching Rhino. `SelSrf` includes both untrimmed NURBS surfaces and single-face
trimmed B-reps while excluding multi-face B-reps. `SelPolysrf` (alias
`SelPolysurface`) and its open/closed variants classify only multi-face B-reps
by shared-edge topology.

`SelPlanarCrv` uses document tolerance. `SelLine` also recognizes
exactly straight, single-span higher-degree NURBS curves, while excluding
multi-span curves and polylines as Rhino does. `SelPolyline` includes native
polylines and multi-segment degree-one NURBS curves, but excludes line objects
and two-control-point degree-one NURBS curves. `SelShortCrv` takes an explicit
positive maximum length and includes curves exactly on that boundary; it uses
the same controlled length calculation as `Length`. Mesh closure uses exact
location-welded polygon-edge topology, so quad meshes, indexed triangle meshes,
and STL-style triangle soup classify consistently; quad diagonals are used only
when an operation explicitly needs triangles.

`SelLast` selects every object changed by the latest object-editing transaction,
including multi-object imports and command outputs. `SelPrev` swaps the current
and previous selection sets. Both replace by default, matching Rhino; set
`DeselectOthersBeforeSelect=No` to add instead.

`SelName` and `SelLayer` add case-insensitive `*`/`?` wildcard matches without
expanding overlapping groups; `SelName ""` selects unnamed objects. `SelGroup`
uses Rhino's exact, case-sensitive group names. Matching hidden or locked layers
with `SelLayer` makes those layers visible and unlocked outside undo history,
while object-level hidden and locked states remain untouched.

`SelColor r,g,b` adds visible, unlocked, ungrouped objects with that resolved
display color; as in Rhino, objects contained in groups are skipped. ByLayer
objects use their layer color; material- and parent-sourced objects currently
use the same documented fallback as the viewport.

`SelDup` adds every visible, unlocked geometric copy except one deterministic
document-order original per class; `SelDupAll` includes those originals.
Equality is independent of object properties, groups, and document tolerance,
and direction-independent for lines, open polylines, analytic circles and
arcs, and compatible NURBS curves. Points and indexed meshes compare exact
stored values; curves use Rhino/OpenNURBS' scale-aware fixed zero policy.
Closed piecewise-linear and NURBS seams remain significant, while mesh vertex
indexing, face order, and winding must match.

`SetObjectName` assigns one shared name to the selection. Add
`AppendCounter=Yes` for Rhino's zero-based suffixes in document order, or use

`SetObjectName ""` to clear names. Unnamed `Group` calls receive Rhino-style
`Group01`, `Group02`, ... names; explicit group names are case-sensitive.

`SetObjectColor r,g,b` assigns Rhino-style per-object display color to the
selection; `SetObjectColor ByLayer` restores layer-driven display while
retaining the stored object color. Selection and locked-object colors still
take visual precedence. Material- and parent-sourced colors are preserved in
3DM files and currently fall back to layer color until materials and instance
definitions are implemented.

`ChangeLayer` moves selected objects without changing their identities, groups,
or the current layer. `CopyToLayer` skips selected objects already on the target,
copies each remaining group subset into a fresh automatic group, and leaves the
original selection unchanged. Hidden and locked target layers are supported.
