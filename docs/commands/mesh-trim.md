# MeshTrim

`MeshTrim` removes a picked region of a polygon mesh. Select cutting meshes,
surfaces, polysurfaces or curves, press Enter, then click the regions to remove.
Preselected cutters skip the first selection. Enter or Escape finishes; accepted
picks remain. `Undo` inside the command reverses the last pick. One Undo after
finishing restores the entire command, including original geometry storage.

Build with `native-smlib`. MeshTrim shares the
[usd-brep/SMLib partition kernel](../mesh-split-kernel.md), cutter conversion and
[vertex-channel transfer](../mesh-split-channels.md) with [MeshSplit](mesh-split.md).
Curves extrude along the active CPlane Z axis. `IP` opens the shared temporary
plane getter. MeshTrim uses document tolerance and has no tolerance or n-gon
options. Surviving regions form one fresh mesh object, inherit the target's
attributes, layer and groups, and have no logical n-gons. Cutters and results are
unselected. A pick on an unsplit mesh deletes it, even for a disjoint cutter.
The getter [retains original cutters and descendant identities](../mesh-trim-continuations.md),
so a cutter that is itself trimmed continues cutting other targets without
cutting its own surviving pieces. Each pick finishes computation before editing the document; an error preserves
earlier accepted picks and their history.

Scripts supply an explicit target and world-space point:

```text
MeshTrim Cutters=cutter-id Target=mesh-id Point=1,1,0
MeshTrim InfinitePlane=WorldYZ Target=mesh-id Point=-1,1,0
```

The [qualification](../mesh-trim-qualification.json) retains 25 immutable Rhino
captures and replays 13 backend and 14 interactive workflows at `1e-7`, including
vertex channels, attributes, groups, selection, source identity and Undo/Redo.
Twelve backend and thirteen interactive workflows match all checks. The remaining
temporary plane capture leaves Rhino's mesh unchanged while Viboceros trims it.
Preselected native macros did not deliver productive picks, so they remain
discovery evidence; application preselection and local Undo have separate tests.
Repeated picks after trimming a cutter itself have a separate continuation
qualification. Perspective views with curve cutters, curved meshing, extreme
scales, near contacts and comparative performance remain unqualified.
See [Rhino's reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/meshtrim.htm).
