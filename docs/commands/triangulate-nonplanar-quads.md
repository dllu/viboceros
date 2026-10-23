# TriangulateNonPlanarQuads

[Meshes and SubDs](meshes-index.md) · [Mesh editing](mesh-editing.md)

Start `TriangulateNonPlanarQuads`, select meshes and press Enter to open its
text options. Preselected meshes open the options immediately. Change options,
then press Enter to apply; Escape or Cancel discards the edit and clears selection.
Geometry, tolerance, selection or editability changes reject staged acceptance.

Complete preselected scripts execute directly:

```text
TriangulateNonPlanarQuads SelectBy=Distance Distance=0.01 SplitMethod=ShortestDiagonal
_-TriangulateNonPlanarQuads _SelectBy=_Angle _Angle 1 _SplitMethod _MinimumArea
```

Fresh defaults are Distance selection, zero distance, zero angle and
ShortestDiagonal. Accepted settings persist across documents and Undo/Redo
within one registry; cancelled or invalid updates preserve the accepted values.
Distance uses model units and Angle uses degrees. Zero is allowed.
Bare `SelectBy` toggles Distance and Angle; `Distance`, `Angle` and `SplitMethod`
also accept separate responses. Local scripts additionally accept `Name=value`.

Split methods are `ShortestDiagonal`, `LongestDiagonal`, `MinimumArea`,
`MaximumArea`, `MinimumAngle` and `MaximumAngle`. The older `MinimizeArea` and
`MaximizeArea` names remain aliases. `Mode` aliases `SelectBy`; local `Mode=Both`
uses both SDK criteria. Rhino's scripted SelectBy rejected Both and
DistanceAndAngle in the retained discovery captures.

OpenNURBS measures the range of distances to its face plane and checks both
opposite corner-normal pairs before choosing a diagonal. With Both enabled,
exceeding either limit splits the quad. Distance equality alone does not split;
angular tests use native normal/cosine comparisons and native tie-breaking.
First triangles replace source quad slots; second triangles append.

Vertices and stored color, UV and normal channels remain unchanged. N-gons
containing a split quad are removed; unaffected overlays survive. Empty removed
native slots are omitted from the model overlay vector. Mesh validation tolerance
preserves valid small features below document tolerance.

Success skips non-mesh selections, clears selection, moves processed meshes to
the object table end and records one Undo step even when no quad splits. IDs,
attributes and groups survive. All mesh results are staged before mutation and
moved into the document without an extra geometry clone.

The [SDK record](../nonplanar-quad-qualification.json) covers 42 independent
inputs. The [command record](../nonplanar-command-qualification.json) covers 30
native workflows, 26 registered command replays and 30 application getters.
Five discovery workflows with rejected or misplaced options remain excluded;
a failed import retains zero qualified geometry. Native face-based threshold
sampling, dialogs/previews, spinner controls, physical postselection, restricted
objects, grips, multi-mesh processing order, extreme coordinates, degenerate
repair and comparative performance remain unqualified.

[Public API documentation](https://mcneel.github.io/rhino3dm/javascript/api/classes/MeshFaceList.html#convertnonplanarquadstotriangles)
