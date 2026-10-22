# SubD geometry

Viboceros stores Catmull–Clark subdivision surfaces as `Geometry::SubD`.
The public OpenNURBS implementation owns topology, crease/corner tags,
subdivision evaluation and serialization. Display meshes remain derived data.

Create standalone faces through the command line or the interactive point getter:

```text
3DFace Output=SubD Interpolated=No 0,0,0 4,0,0 4,3,2 0,3,0
3DFace Output=SubD PolygonType=Triangle 0,0,0 4,0,0 2,3,0
3DFace Output=SubD Interpolated=Yes 0,0,0 4,0,0 4,3,2 0,3,0
```

`PolygonType=Ngon`, `InPlane=Yes`, `Mode=MultipleFaces` and point Undo work as
for mesh faces. Exactly coincident corners in adjacent faces share vertices;
interior edges are smooth and boundary edges are creased. Boundary vertices
normally move inward on the limit surface when `Interpolated=No` (the default):
picked corners define the control net. With `Interpolated=Yes`, vertex limit
stations pass through the picked points. Appending preserves the source's
original vertex limit stations and fits new stations to the entered points;
the source's control positions may move. Exact source control coordinates still
identify vertices to join. InPlane constrains the entered targets before fitting.
Interpolation is remembered and affects only SubD output.

The [mesh and SubD command index](commands/meshes-index.md) lists primitive
construction guides and their native qualifications. Primitive commands reuse
shared rectangle, circle, ellipse and axis getters, with independent settings.
Use [Crease and RemoveCrease](commands/crease.md) to edit hard edges and corners
through typed component indices or selected control grips.

Select a SubD before `3DFace`, choose `Append` in the point getter, or supply
`Append=<id>` to add faces to it. Without an explicit `Output` option, the target
sets the output family. `FromEdge=Yes` picks a control-cage boundary;
`Chain=Yes Mode=MultipleFaces` supports walking along existing or new boundaries,
with corner Undo and Enter to choose another edge. All changes are staged until
completion, and double Escape discards the draft without consuming Redo.

Joining uses exact coincident vertices. Even a tiny gap creates a detached SubD
on the current layer. Detached components in a standalone MultipleFaces draft
also become separate objects. Joined source objects retain their identity, name
and layer. Their original native component data are edited through OpenNURBS;
unrelated creases and corners remain. A joined boundary becomes smooth and its
endpoint tags are recomputed. Shared-vertex fans and a three-face nonmanifold
edge have native comparison coverage.

Interpolation solves a sparse limit-position system with one factorization for
all coordinates. Public OpenNURBS evaluates its masks and certifies each fitted
vertex station against the document absolute tolerance. Unresolved fits return
an error before changing the document. Draft previews show boundaries; fitting
runs at completion. Mask generation permits four million coefficients and 32
million local basis work units. Factor fill and comparative speed are unqualified.

Before entering corners, `Output=Mesh` on a SubD target creates separate mesh
output, while `Output=SubD` on a mesh target creates separate SubD output. The
source geometry stays unchanged. Mixing output families inside a face draft is
not implemented.

SubDs draw and support object picking in wireframe, shaded and ghosted modes.
Shading uses native limit-surface normals for each sector, keeping shallow
creases sharp without averaging normals across them.
`PointsOn` exposes their control vertices; ordinary transforms and control-point
edits retain SubD topology. Creation and edits support Undo/Redo. 3DM Save/Open
preserves SubDs, including native fields such as crease sharpness that are not
yet exposed as editable properties. Import unit conversion scales their control
points. Model bounds use the conservative control hull.

Display meshes sample exact limit-surface points after bounded subdivision.
The facets between samples are approximations without a model-tolerance
certificate. Exact SubD projection/intersections, surface snaps, certified tight
bounds and border extraction are not implemented. Construction is limited to
100,000 vertices and faces, 400,000 face corners and a 64 MiB archive; tessellation
accepts density 1–6 with a one-million-quad budget. Display density adapts to that
budget. Unsupported commands reject SubDs rather than changing their family.

[Qualification report](subd-qualification.json): ten Rhino 8 recipes captured
for control topology and again for limit meshes. Command and interactive getter
tests compare topology, winding, tags, exact vertex limit stations, sampled limit
meshes, current-layer attributes, selection and grouped history at `1e-6`.
[Append qualification](subd-append-qualification.json) adds 32 native recipes:
31 command comparisons and 32 viewport getter comparisons, including physical
edge picks, cancellation, disconnected pieces, reversed faces, late bridges,
source n-gons, crease/corner changes and output-family switches. The largest
sampled difference in those controls is about `3.6e-15`. Seven further captures
compare native sector normals, with a largest vector difference of about
`3.5e-8` against Rhino’s float normal arrays. Separate tests cover
3DM round trips, units, transforms, control edits, display and picking. General
SubD parity, texture/material channels and comparative performance remain open.

[Interpolation qualification](subd-interpolation-qualification.json) adds 36
interpolated recipes and three ordering controls. The 38 command and 39 getter
comparisons cover standalone faces, append, source tags, disconnected pieces,
chains, tilted/concave InPlane faces, Undo and cancellation. Each compares the
native control net, vertex limit stations and sampled density-two surface.
General imported sharpness and arbitrary nonmanifold fits remain unqualified.
