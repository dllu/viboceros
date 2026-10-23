# TriangulateNonPlanarQuads

[Meshes and SubDs](meshes-index.md) · [Mesh editing](mesh-editing.md)

Select meshes and run, for example:

```text
TriangulateNonPlanarQuads Mode=Both Distance=0.01 Angle=1 SplitMethod=ShortestDiagonal
```

Options accept `Name=value` or separate `Name value` tokens, including English
underscore prefixes. `Mode` selects `Distance`, `Angle`, or `Both`. Distance is
in model units; Angle is in degrees. Zero is allowed. The local defaults remain
Distance mode, document absolute tolerance, one degree and ShortestDiagonal;
native command defaults and interactive input have not been qualified.

The public OpenNURBS converter measures the range of distances to its face plane
and checks both pairs of opposite corner normals. With Both enabled, exceeding
either limit causes conversion. The distance comparison is strict: equality alone does not require splitting.
Angular tests use the native normal/cosine comparison. Qualification is
independent of the split method.
This replaces the earlier fourth-point/ABC-plane test and combined AND rule.

SplitMethod also accepts `LongestDiagonal`, `MinimizeArea`, `MaximizeArea`,
`MinimumAngle`, and `MaximumAngle`; native tie-breaking applies. First triangles
replace their source quad slots; second triangles append in source order.
Vertices and stored color, UV and normal channels remain unchanged. N-gons
containing a split quad are removed; unaffected overlays survive. Empty removed
slots in native n-gon tables are omitted from the model overlay vector.

Geometry validation uses the mesh validation tolerance, preserving valid small
features below document tolerance. Selected results are staged atomically and
moved into the document without an extra geometry clone. Local command identity,
attributes, groups, selection and Undo/Redo invariants remain unchanged.

The [qualification record](../nonplanar-quad-qualification.json) covers 42
independent RhinoCommon inputs and local scripted command replays. It includes
all six methods, angular holdouts straddling diagonal-normal angles, exact and
adjacent representable distance thresholds, zero
thresholds, independent distance/angle failures, concave and tiny geometry,
channels, and n-gons with reordered owners. Native command workflow, option
memory, degenerate repair, extreme coordinates and performance remain
unqualified. An earlier failed import is retained with zero qualified geometry.

[Public API documentation](https://mcneel.github.io/rhino3dm/javascript/api/classes/MeshFaceList.html#convertnonplanarquadstotriangles)
