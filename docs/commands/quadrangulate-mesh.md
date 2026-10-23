# QuadrangulateMesh

[Meshes and SubDs](meshes-index.md) · [Mesh editing](mesh-editing.md)

Start `QuadrangulateMesh`, select meshes, and press Enter to open its text
options. With meshes already selected, the options open immediately. Change
`Planarity` or `Rectangularity`, then press Enter to accept. Each option also
accepts a separate numeric response. Escape or Cancel discards the staged edit
and clears selection.

Complete preselected scripts execute directly:

```text
QuadrangulateMesh Planarity=5 Rectangularity=2
_-QuadrangulateMesh _Planarity 5 _Rectangularity 2
```

Fresh defaults are 5 degrees and 2. Accepted values persist across documents and
Undo/Redo within the same command registry; cancelled or invalid edits do not
replace them. Geometry, tolerance, selection or editability changes while the
options are open reject acceptance.

Conversion uses public OpenNURBS. Adjacent triangles must share raw vertex
indices and consistent winding. On meshes with more than two faces, the shared
edge must be strictly longest in both triangles; equal-length ties remain
triangular. Rectangularity bounds the diagonal-length ratio. The kernel accepts
angles through 180 degrees; OpenNURBS resets values above 90 degrees to 2.5 and
caps its effective normal-angle limit at 60 degrees. Native command validation
of extreme option values remains unqualified.

Success skips non-mesh objects, moves processed meshes to the table end, clears
selection, and records one Undo step even if no triangle pair merges. Object
IDs, attributes, groups, stored vertex channels and valid n-gons survive.
Invalid n-gon overlays produced by native conversion are discarded explicitly.
All selected mesh results are staged before document mutation.

The [command record](../quad-command-qualification.json) covers twenty native
scripted workflows, eighteen registered command replays and twenty application
getters. The [SDK record](../quad-conversion-qualification.json) adds fourteen
independent conversion cases and both option syntaxes. Native dialog preview,
spinner controls, physical postselection, restricted objects, grip behavior,
multi-mesh processing order and comparative performance remain unqualified.
