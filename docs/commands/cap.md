# Planar hole capping

[Command reference](README.md) · [Surfaces and solids](surfaces.md)

`Cap` fills planar openings on selected NURBS surfaces, B-reps, meshes, and
SubDs. Select objects first, or enter the command and select them at the prompt.
Nonplanar openings remain open. Already closed objects and single planar sheets
are unchanged.

```text
Cap [DeleteInput=Yes|No] [Crease=Yes|No] [Triangles=Yes|No]
```

These remembered options apply to meshes and SubDs:

| Option | Default | Behavior |
| --- | --- | --- |
| `DeleteInput` | `Yes` | Replace the source; `No` creates an unselected, ungrouped copy with the source attributes. |
| `Crease` | `Yes` | Retain a hard cap boundary. `No` welds mesh boundaries or smooths SubD boundaries. |
| `Triangles` | `No` | Group a simple mesh cap into an n-gon, or create one convex SubD face. `Yes` creates separate triangles. |

Concave SubD caps and caps with inner openings use triangles in either mode.
Convex SubD triangle fans use the control-loop bounding-box center. Original
SubD component IDs and face data are retained; closing an inward SubD reverses
its orientation outward. Tag reclassification can relax a former corner.

The document absolute tolerance decides boundary planarity. Narrow mesh facets
retain nonzero edges and area. SubD capping works on control boundaries; it does
not rebuild the object through a mesh. Ambiguous coplanar regions remain open.

In-place edits retain object IDs, names, layers, colors, and groups. Copies leave
source groups and selection unchanged. Geometry-attached user text is cleared
on changed geometry. Mixed selections cap B-reps and NURBS surfaces in place.
Preselection is retained; command-first selection clears on success. All changes
form one undo step; a no-op adds no geometry history. Invalid input and exhausted
resource limits reject the complete command.

Implementation and retained differences are documented separately:

- [B-rep construction, validation, and oracle evidence](../geometry/cap-brep.md)
- [Mesh comparisons and remaining layout/scale differences](../geometry/cap-mesh-native.md)
- [SubD comparisons, triangle layouts, and limits](../geometry/subd-cap-native.md)

For nonplanar mesh repair, see [meshes](meshes.md).
