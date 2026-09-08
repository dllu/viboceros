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

`SelNonManifold` adds selectable meshes and B-reps with edges used by more than
two faces. Open boundaries alone do not qualify. Geometry, visibility, locking,
and undo/redo history remain unchanged. Tests cover a tetrahedron with an extra
face sharing an edge, its B-rep conversion, open and closed manifold controls,
preselection, hidden/locked exclusions, and invalid arguments. This follows
the documented [Rhino selection command](https://docs.mcneel.com/rhino/8/help/en-us/commands/selection_commands.htm#SelNonManifold);
no live Rhino comparison has yet been recorded for this command.

`SelPlanarCrv` uses document tolerance. `SelLine` also recognizes
exactly straight, single-span higher-degree NURBS curves, while excluding
multi-span curves and polylines as Rhino does. `SelPolyline` includes native
polylines and multi-segment degree-one NURBS curves, but excludes line objects
and two-control-point degree-one NURBS curves. `SelShortCrv` takes an explicit
positive maximum length and uses a comparison limit of `maximum × 1.000001`;
[40 retained Rhino line measurements](../short-curve-selection-measurement.json)
verify this relative allowance, including adjacent floats at its boundary,
three length scales, and two document tolerances. The enlarged comparison limit
is capped at the largest finite value. Nonlinear curves use a separate
[shortness predicate](../curve-shortness.md), not the controlled arc-length
measurement used by `Length`. Its adaptive three-point integration can reject
on a coarse estimate, matching all 96 retained circle/arch classifications.
Knot refinement can change its answer without changing the locus; this is
selection compatibility behavior, not a certified geometric length bound.
Mesh closure uses exact
location-welded polygon-edge topology, so quad meshes, indexed triangle meshes,
and STL-style triangle soup classify consistently; quad diagonals are used only
when an operation explicitly needs triangles.

`SelLast` recalls selectable changed objects without group expansion and remembers
its Yes/No option. Idle Move, option memory, empty-layer creation, and recall after
pure deletion/undo/redo are verified. Object-state selection replay is described
in [history selection](../history-selection.md).
`SelPrev` recalls recorded selectable objects without group expansion. Replacement
swaps with a nonempty current set; additive recall leaves previous memory intact.
It remembers `DeselectOthersBeforeSelect=Yes|No` per registry. See
[selection recall](../selection-recall.md) for verified behavior and limitations.

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
`Ungroup` removes each selected object's last membership; `UngroupAll` removes
every membership on those objects. Both retain empty definitions. See
[ordered groups](../groups.md) for nested membership, copied groups, 3DM order,
and the separate named/global definition-deletion extensions.

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
