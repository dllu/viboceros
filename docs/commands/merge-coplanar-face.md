# MergeCoplanarFace

Run `MergeCoplanarFace`, then click two adjacent coplanar faces on the same
mesh or polysurface. The first face stays highlighted while you choose the
second. A mesh n-gon is one logical face: clicking any underlying facet selects
that whole n-gon, including its hole boundaries. A nonadjacent or noncoplanar
pair leaves the first pick available for retry. Esc or `Cancel` releases the
getter. A successful merge finishes the command.

Enter `MergeAllCoplanar` after the first pick to merge its entire connected
planar region. Alternatively start with `MergeCoplanarFace MergeAllCoplanar=Yes`
and click one seed face. `MergeAllCoplanar=No` restores pair selection.
Disconnected regions remain untouched, even when they share the same plane.

Scripts can address original face indices explicitly:

```text
MergeCoplanarFace Object=id Face=0,1
MergeCoplanarFace id 0,1
MergeCoplanarFace MergeAllCoplanar Object=id Face=4
```

Replace `id` with the object's ID. Mesh indices address the underlying facet
table; any member facet identifies its complete n-gon. B-rep indices address
the original face table. The getter also accepts `Object=id Face=index` for
the first pick and a single integer for the second pick. Indices apply to the
current source before this merge; obtain them again after changing topology.

Meshes use OpenNURBS in all builds. B-reps require `--features native-smlib`
and use usd-brep's protected topology remover. Both paths share the existing
[whole-object merge](merge-all-coplanar-faces.md) admission, resource limits and
tolerance policy. Holes and surviving boundary vertices are retained. Copies
are staged before replacement; object identity, attributes, groups and selection
survive one Undo/Redo step. Failures and no-ops preserve Redo. A getter rejects
changed source geometry or document tolerances.

SubD, generic face preselection/window selection, repeated merges within one
getter invocation and full Rhino getter parity remain pending. A standalone
surface has no adjacent face; an explicit region request is a no-op. Mesh
seams without existing shared topology are not sewn across gaps. Native welding
can choose one color at a seam with differently colored duplicate vertices.

The [qualification record](../selected-coplanar-faces-qualification.json) links
local command observations and sanitizer checks. Public B-rep SDK probes use
the selected-pair and seed overloads of `Brep.MergeCoplanarFaces` on owned copies.
Selected mesh native getter captures remain pending; no whole-mesh SDK merge
stands in for that operation. Fresh licensed captures remain blocked by the
recorded [Wine startup issue](../oracle-sspi.md). Local checks establish neither
Rhino parity nor comparative performance.

References: [Rhino command](https://docs.mcneel.com/rhino/8/help/en-us/commands/mergecoplanarface.htm),
[B-rep overloads](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/Overload_Rhino_Geometry_Brep_MergeCoplanarFaces.htm).
