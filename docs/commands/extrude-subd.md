# ExtrudeSubD

[Meshes and SubDs](meshes-index.md) · [Command input](../command-line.md)

`ExtrudeSubD` moves selected faces or naked edges and adds quad walls. Whole
SubDs must be selected before starting. Alternatively, start the command, click
faces or naked edges, and press Enter to accept the component selection.
Ctrl/Command removes picks; window and crossing selection are supported.
Naked edges take priority over faces near the cursor.

Enter distance and direction options to update the control-cage preview, then
Enter to commit. Escape cancels without editing the document or consuming Redo.
A complete typed invocation with a distance and selected inputs executes immediately:

```text
ExtrudeSubD 1.5
ExtrudeSubD -1.5 Faces=0,2 Basis=UVN Direction=N
ExtrudeSubD 1.5 Edges=0,1 Basis=UVN Direction=V
ExtrudeSubD 2 Basis=WCS Direction=Z
ExtrudeSubD 2 Basis=WCS Direction=1,2,3
```

`Faces` and `Edges` use zero-based control-component ordinals on each selected
object. The geometry and command APIs also accept individual component targets
on unselected parents. Whole-object targets override component targets on that
object. Interior edges and mismatched geometry are rejected before committing.

The initial options are distance 1, Basis=UVN, Direction=N. WCS accepts X/Y/Z
or a nonzero vector; vector length is normalized. UVN accepts N, or V for naked
edges alone. Switching basis chooses Z or N. Successful option changes are
remembered outside model history, including after cancellation.

Face N directions use the control mesh's stored float vertex normals, normalized
in double precision. Selected crease sectors are averaged without normalizing
the result again. Edge N directions use incident control-face normals. V uses
the oriented edge tangent crossed with that normal, then averages contributions
at shared vertices. Mixed picks also average face and edge contributions.
These directions differ from OffsetSubD's limit-sector normals.
Closed whole-object extrusion moves its existing net without adding walls.
A branching selection that would create nonmanifold walls follows measured Rhino
behavior: WCS reports success with an unchanged cage; UVN moves controls without
adding topology.

Outputs remain editable SubDs with source IDs, layers, attributes and groups.
Whole-object picks remain selected at completion; component picks clear. One
Undo/Redo step restores geometry and native whole-object selection behavior.
Previews reject changed sources, selection, attributes, groups or tolerance.

Three frozen private-display Rhino runs cover 128 whole, face, naked-edge and mixed cases,
positive and negative distances, smooth and creased sectors, warped faces,
disconnected regions, triangles and closed nets. Replays compare raw topology,
tags, controls and limit points at `1e-10`, metadata, selection and Undo/Redo.
Application tests also cover both preselection and command-first component picks,
viewport pointer input in all three display modes, cancellation and stale previews.
Refined nets and asymmetric soft sharpness undergo native 3DM roundtrips.

Inputs and outputs are bounded to 100,000 controls/faces, 400,000 edges/corners
and 64 MiB per archive, with a 64 MiB aggregate staged-archive limit. Source
edges must have one or two incident faces; growth is checked before building
topology. Free point dragging, SetBasePoint, EdgeLoop, native option persistence,
symmetry, face packing, texture/material parity and native mouse-gesture parity
remain to be qualified.

```sh
cargo test -p viboceros-geometry --lib subd_extrude
cargo test -p viboceros-command --lib extrude_subd
cargo test -p viboceros-oracle --test subd_extrude
cargo test -p viboceros --bin viboceros subd_extru
python3 -m unittest tools.rhino_oracle.test_subd_extrude
```

Reference: [Rhino ExtrudeSubD](https://docs.mcneel.com/rhino/8/help/en-us/commands/extrudesubd.htm).
