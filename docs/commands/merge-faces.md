# MergeFaces

`MergeFaces` merges connected mesh or SubD faces selected through faces, edges,
or vertices. Ctrl+Shift-select components before entering the command, or run it
first and pick components, then press Enter. Esc cancels the getter.

`Results=Selected` highlights merged faces; `Results=NotSelected` clears the
processed component selections. The option is remembered within the application
session, including after cancellation. Enter `Results`, then `Selected` or
`NotSelected`, or supply `Results=NotSelected` directly.

Whole-object selection opens the component getter. It does not select every face.
Multiple mesh and SubD owners can participate. A successful edit preserves owner
IDs, attributes, layers and groups and creates one Undo entry. Unchanged regions
clear their component picks without adding an Undo entry. Undo clears processed
picks. With Results=Selected, Redo restores only original result faces whose
automatic selection was never cleared. Clearing and explicitly reselecting the
same face does not restore that replay status. Whole-object picks made on the
output before Undo return on Redo. Surviving SubD input edge/vertex picks can
remain immediately after the edit but are not restored by Redo.

## Scripts

For selected mesh/SubD owners, explicit selectors provide a script extension:

```text
MergeFaces Faces=0,1 Results=Selected
MergeFaces Edges=2 Vertices=0 Results=NotSelected
MergeFaces Faces=All Object=selected-uuid
```

Indices are zero-based: stored mesh face indices, welded mesh topology edge or
vertex ordinals, and SubD control-table ordinals sorted by native component ID.
Faces, Edges and Vertices accept `All` or comma-separated indices. `Object`
restricts selected owners to one UUID. Invalid or stale targets abort the whole
edit before document changes.

## Geometry and validation

New regions need a single simple boundary; holes and closed regions without a
boundary stay unchanged. See the [mesh](../geometry/mesh-face-merge.md) and
[SubD](../geometry/subd-face-merge.md) kernel notes for priority, seam and cleanup
rules. Mesh outputs retain the existing fixed-triangle validation gate.

The [validation receipt](merge-faces-validation.json) binds 77 actual Rhino command
cases, including four acknowledged physical postselection cases, to core and UI
replays. Geometry uses `1e-7`; identities, attributes and component selections are
exact. Fixed-UUID repeats establish that Rhino's insertion order among changed
meshes varies with Results=NotSelected. Only that replacement run is compared by
identity; Viboceros uses deterministic document order. Selected result order,
other owner order and Undo order remain strict.

[Sequential SubD edits](../geometry/subd-edit-history.md) additionally qualify
allocation through global/local SubDivide and public SDK/archive copies. Other
allocating operations, nonmanifold regions and custom face textures remain
outside the qualified corpus. An earlier 14-case capture is retained as a
diagnostic: four postselection records contain invalid source archive fields.
The corrected capture supplies the admitted archive seeds.

[SubD face-color validation](../geometry/subd-face-properties.md) additionally
covers color clearing, retained neighbors, sequential merges and Undo/Redo.
