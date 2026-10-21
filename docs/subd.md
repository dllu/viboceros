# SubD geometry

Viboceros stores Catmull–Clark subdivision surfaces as `Geometry::SubD`.
The public OpenNURBS implementation owns topology, crease/corner tags,
subdivision evaluation and serialization. Display meshes remain derived data.

Create standalone faces through the command line or the interactive point getter:

```text
3DFace Output=SubD Interpolated=No 0,0,0 4,0,0 4,3,2 0,3,0
3DFace Output=SubD PolygonType=Triangle 0,0,0 4,0,0 2,3,0
```

`PolygonType=Ngon`, `InPlane=Yes`, `Mode=MultipleFaces` and point Undo work as
for mesh faces. Exactly coincident corners in adjacent faces share vertices;
interior edges are smooth and boundary edges are creased. Boundary vertices
normally move inward on the limit surface: picked corners define the control
net, rather than interpolation targets. `Interpolated=Yes` and appending to an
existing SubD are not implemented.

SubDs draw and support object picking in wireframe, shaded and ghosted modes.
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
The largest sampled surface difference is about `1.8e-15`. Separate tests cover
3DM round trips, units, transforms, control edits, display and picking. General
SubD parity, texture/material channels and comparative performance remain open.
