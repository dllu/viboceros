# Mesh extraction and editing

[Polygon meshes](meshes.md) · [Meshes and SubDs](meshes-index.md)

`PolygonCount` reports the selected objects' triangle and quadrilateral mesh
face counts, plus the triangle count after splitting every quad into two.
N-gon overlays do not change these counts. The command is read-only and matches
the tested explicit mesh, rectangular planar surface, and box B-rep
[Rhino PolygonCount](https://docs.mcneel.com/rhino/8/help/en-us/commands/polygoncount.htm)
reports. Curved surfaces and B-reps remain unsupported because Rhino counts
their render meshes, whose polygon counts depend on the meshing settings.

`OffsetMesh 2` copies selected meshes by moving each topological vertex along
the average of its raw vertex normals. Coincident unwelded copies move together.
`DirectionMethod=UserSelectedDirection` with
`Direction=0,0,1` uses one specified vector instead. `AverageNormals=Yes`
moves all vertices opposite their common average normal, falling back to the
opposite of the active construction plane normal when that average cancels.
`FlipAll=Yes` reverses the side and the winding of open output skins.
`BothSides=Yes` creates two offset skins; `Solid=Yes` also
includes the original skin for a one-sided offset and joins naked boundaries
with quadrilateral walls. Without `Solid`, `AllowDisjoint=No` returns only the
offset skin, or two separate skins for `BothSides=Yes`. `AllowDisjoint=Yes`
combines the original and offset skins in one result, or combines the two
offset skins when using both sides. Each skin keeps any disconnected source
components in the same mesh. Solid offsets retain one object per source,
including closed inner and outer shells.
`DeleteInput=Yes` removes the originals. Output keeps source attributes and
groups, and the edit is undoable. Offsets that collapse faces are rejected.
If the offset skin merges formerly distinct topology vertices, Rhino returns
two skins without walls; this command follows that behavior. The retained
[Rhino fixture](../../tools/rhino_oracle/fixtures/mesh_offset.json) and
[observations](../../tools/rhino_oracle/observations/mesh_offset.json) cover
normal and average-normal offsets, both sides, `FlipAll`, negative distances,
`AllowDisjoint`, solid raw wall order for triangles, quads, and rings with holes,
boundary ordering across a triangulated square, an unwelded seam, a bent
collapsed skin, and a closed tetrahedron. Manually picked vector directions
and other mesh arrangements still need live comparison.
The geometry kernel also exposes `offset_mesh_rhinocommon_normal` and
`offset_mesh_rhinocommon_vector` for the public RhinoCommon `Mesh.Offset`
overloads. RhinoCommon's normal overload moves opposite vertex normals, and
its solid overload uses different skin winding from the interactive command.
The retained [vector API fixture](../../tools/rhino_oracle/fixtures/mesh_offset_vector_api.json)
and [observations](../../tools/rhino_oracle/observations/mesh_offset_vector_api.json)
cover eight open and solid explicit-vector offsets.

`AddNgonsToMesh PlanarTolerance=0.01` adds logical n-gon overlays to connected
coplanar mesh faces that share welded, oppositely oriented raw edges. The
default planarity distance is the document's absolute tolerance. A group uses
its first face's plane; groups with holes, disconnected boundaries, or fewer
than two faces are skipped. Existing n-gons remain. `DeleteMeshNgons` removes
all n-gon overlays from selected meshes. Both commands keep stored faces,
vertices, colors, object identity, attributes, groups, and selection, with
undo support. Interactive n-gon conversion policy still needs Rhino oracle
comparison.

`ExtractMeshEdges` creates fresh current-layer curves from selected polygon
meshes. `ExtractBy=Unwelded` (the default) includes both naked edges and seams
whose coincident endpoints use distinct raw mesh vertices; `ExtractBy=Naked`
keeps only one-face edges. `ExtractBy=BreakAngle` accepts strict
`GreaterThanAngle=`/`LessThanAngle=` bounds in degrees. `JoinResults=Yes`
combines each filtered edge network into deterministic edge-exact polylines,
including branched Euler trails; the default emits individual lines. Sources
and results remain selected, matching Rhino's extraction workflow.

`ExtractMeshFaces` separates an ordered zero-based face list from every
selected mesh (`Faces=All` is supported), or omits the selector for a one-pick
viewport workflow. The unselected remainder keeps its source identity; the
selected result inherits attributes and group membership. Extracting every
face reuses the source identity. `MakeCopy=Yes` instead leaves each source
unchanged. Both parts compact unused vertices in Rhino source order. Viewport
clicks pick the visible selected-mesh face by screen coverage and depth.
`Face=index Object=selected-uuid` targets one mesh in a script; an unqualified
face index applies to every selected mesh. Typed points retain model-space
nearest-face selection.

`DeleteFaces` removes an ordered zero-based face list from every selected mesh
or B-rep (`Faces=All` is supported), or omits the selector for a one-pick
viewport workflow. A partial edit keeps the unselected source object's
identity, attributes, groups, and surviving source face order. Mesh results
compact unused vertices in source order; deleting every face removes the
object. Viewport clicks use depth-aware picking for both meshes and B-reps;
B-rep display triangles retain their source face indices across seam repairs.
When a B-rep cannot be meshed for display, the viewport falls back to a
construction-plane point pick. Typed points use model-space nearest-face
selection. Scripts can use
`Face=index Object=selected-uuid` to target one selected mesh or B-rep. SubD
input awaits a native SubD geometry type.
Mesh extraction and deletion preserve n-gon overlays in any result containing
every member face of the n-gon. A partial face group has no retained overlay.

`ExtractMeshFacesByArea SmallerThan=2` extracts stored triangle and quad faces
whose unsigned areas are at or below the limit. `LargerThan=area` selects faces
at or above its limit and can be used alone or together with
`SmallerThan=area`. Equal bounds are accepted. Face thresholds use Heron's
formula to match Rhino's observed rounding, so a mathematically exact area
may round just below its bound. `MinArea` and `MaxArea` are accepted aliases.
`MakeCopy=Yes` retains the input faces. `BorderOnly=Yes`
creates boundary polylines and leaves the input mesh intact. The command
stages all selected meshes before editing, preserves output attributes and
group memberships, and rejects a selection containing non-mesh objects.

`ExtractMeshFacesByEdgeLength EdgeLength=0.1 Select=Shorter` extracts faces
with any boundary edge at or below the given length. `Select=Longer` uses the
longest boundary edge and includes equality. Zero is accepted; negative
thresholds are rejected. Quad diagonals are excluded. It shares the area
command's `MakeCopy` and `BorderOnly` behavior, atomic staging, attributes,
groups, selection, and undo.

`ExtractMeshFacesByAspectRatio AspectRatio=9` extracts faces whose aspect ratio
is at least the threshold. A triangle's ratio is its longest edge divided by
the opposite altitude. A quad uses the larger ratio of the two triangles
formed by its shorter diagonal. A square therefore has ratio 2. Collinear
constituent triangles count as infinite ratio.
`MakeCopy` and `BorderOnly` use the same output policy as the other metric
extraction commands. The retained [face metric fixture](../../tools/rhino_oracle/fixtures/mesh_face_metrics.json),
[API observations](../../tools/rhino_oracle/observations/mesh_face_metrics.json),
[command fixture](../../tools/rhino_oracle/fixtures/mesh_aspect_command.json), and
[command observations](../../tools/rhino_oracle/observations/mesh_aspect_command.json)
compare Rhino's quad ratio and inclusive extraction threshold.

`ExtractMeshFacesByDraftAngle StartAngle=0 EndAngle=45 ViewDirection=0,0,1`
extracts stored faces whose oriented polygon normal makes an angle in the
inclusive range with the direction from the model toward the viewer. The app
supplies its active viewport direction when `ViewDirection` is omitted;
standalone command calls supply it explicitly. Omitted angles default to
`StartAngle=0 EndAngle=89`, matching Rhino's initial command values.
`MakeCopy=Yes` and `BorderOnly=Yes` follow the other face-filter commands.
Rhino's older `StartAngleFromCameraDir`, `EndAngleFromCameraDir`, and
`GetBorder` names are accepted. A reversed range extracts no faces. The retained
[command fixture](../../tools/rhino_oracle/fixtures/mesh_draft_angle_command.json)
and [Rhino observations](../../tools/rhino_oracle/observations/mesh_draft_angle_command.json)
record Top and Bottom view selections and angle boundaries.

`ExtractConnectedMeshFaces Face=0 Angle=0 Compare=Less` extracts the region
reachable from stored face 0 across topological edges. At the default 0° angle,
Rhino extracts the whole connected region regardless of face normals. At
positive angles, neighboring faces must meet the inclusive `Less` or `Greater`
normal-angle comparison. `Face` is a zero-based index on each
selected mesh; the same seed index is used for every selected mesh. Unwelded
vertices at identical positions count as connected. `MakeCopy=Yes` and
`BorderOnly=Yes` preserve the source mesh and use the other extraction
commands' attributes, groups, result selection, and undo behavior. Viewport
clicks select the visible face by screen coverage and depth, including stacked
meshes and quadrilateral faces. Scripts can supply `Face`, `FacePoint=x,y,z`
(nearest in model space), or `Face=index Object=selected-uuid`. A bare face index
applies to every selected mesh.

The command and RhinoCommon `Mesh.Faces.GetConnectedFaces` both check the first
other incident face on a nonmanifold edge at positive angles. The geometry
module implements this traversal in `rhinocommon_connected_faces_by_angle`.
The [API fixture](../../tools/rhino_oracle/fixtures/mesh_connected_faces_api.json)
and [Rhino observations](../../tools/rhino_oracle/observations/mesh_connected_faces_api.json)
cover folded, unwelded, and nonmanifold meshes. The
[command fixture](../../tools/rhino_oracle/fixtures/mesh_connected_command.json)
and [command observations](../../tools/rhino_oracle/observations/mesh_connected_command.json)
cover the same boundary behavior through Shaded viewport face picks.

`ExtractMeshPart Face=0` extracts the region reachable from stored face 0
without crossing naked or unwelded topology edges. `Faces=0,2`
selects multiple seed regions, and `Faces=All` selects every region. The script
option `ExtractToNonManifoldEdges=Yes` stops at nonmanifold edges, while
`ExtractWholeDisjointParts=Yes` crosses both nonmanifold and unwelded edges,
stopping only at naked edges. An unwelded edge has distinct
raw vertex indices on both ends for each incident face. `JoinOutput=Yes`
combines selected faces into one mesh per source; `JoinOutput=No` emits one
mesh per selected region and is the default. `MakeCopy=Yes` retains the source;
`BorderOnly=Yes` emits a line
segment for each boundary edge and leaves the source unchanged. Viewport
clicks select the visible face by screen coverage and depth, including stacked
meshes and quadrilateral faces. Scripts can supply `Face`, `Faces`,
`FacePoint=x,y,z` (nearest in model space), or `Face=index Object=selected-uuid`.
Bare face indices apply to every selected mesh.
The [live command fixture](../../tools/rhino_oracle/fixtures/mesh_part_command.json)
and [Rhino 8 observations](../../tools/rhino_oracle/observations/mesh_part_command.json)
show that Rhino's default crosses a three-face nonmanifold edge but stops at
an unwelded edge. The Rhino 8.32 build used for those probes does not accept
`ExtractToNonManifoldEdges` as a script option, although Viboceros accepts it
as an explicit boundary control. The same fixture checks that `JoinOutput=No`
keeps two picked regions separate while `JoinOutput=Yes` combines them.
Multi-region extraction uses a batched mesh subset path; the
[local benchmark](../mesh-face-groups-performance.json) records its kernel
throughput separately from command and Rhino timings.

[`RebuildMeshNormals`](rebuild-mesh-normals.md) reconstructs stored normals and
removes unused vertices, preserving raw seams, channels and n-gon overlays.
It supports preselection, command-first selection and document Undo/Redo.
[`RebuildMesh`](rebuild-mesh.md) also removes n-gons and provides independent
UV/color preservation options with remembered accepted settings.

[`TriangulateMesh`](triangulate-mesh.md) uses OpenNURBS to split quads while
retaining vertex channels. Its command removes all n-gon overlays; the geometry
API retains and remaps them. The command page describes selection and history.

[`TriangulateNonPlanarQuads`](triangulate-nonplanar-quads.md) uses OpenNURBS
planarity tests and six diagonal policies. With `Mode=Both`, exceeding either
active threshold splits a quad. N-gons containing split quads are removed;
unaffected overlays and stored vertex channels survive.

[`QuadrangulateMesh`](quadrangulate-mesh.md) merges triangles through OpenNURBS.
Start with selected meshes or select them after starting the command, adjust
Planarity and Rectangularity, then press Enter. Fresh defaults are 5 degrees
and 2; accepted options persist. Cancel discards option edits.

`SwapMeshEdge` replaces a welded interior edge shared by exactly two
consistently oriented triangle faces with their opposite diagonal. Use
`Edge=1` for the deterministic exact-location topology index, or omit it to
pick one edge in the viewport. Vertex storage, face slots, object identity,
attributes, groups, and selection are preserved. Swaps that would create a
degenerate face are rejected to retain Viboceros's validated-mesh invariant.

`CollapseMeshEdge` follows RhinoCommon's deterministic API behavior by moving
both topology endpoints to their midpoint. Use `Edge=1` for the topology index,
or omit it to pick one selected-mesh edge in the viewport. Collapsed triangles
are removed, collapsed quad sides become triangles, independent unwelded seam
components remain distinct, and surviving faces and vertices retain source
order. Surviving objects keep identity, attributes, groups, and selection; an
empty result deletes the object. A collapse that would leave a zero-area face
is rejected atomically to preserve the validated-mesh invariant.
N-gon overlays follow their surviving face groups; an overlay whose entire
face group is removed disappears.

`SplitMeshEdge` divides a selected topology edge at a normalized parameter in
its deterministic wireframe direction. Use `Edge=1 Parameter=0.25`, provide an
edge point followed by a split point, or enter the bare command for the same
two-pick viewport workflow. Affected triangles become two triangles and quads
become three; unaffected faces stay first and replacements append in Rhino
order. Welded faces share one appended split vertex, while unwelded replacement
triangles remain fully separated. Exact endpoint parameters preserve Rhino's
coincident topology behavior. Object identity, attributes, groups, selection,
and undo are retained; tolerance-degenerate results are rejected atomically.
N-gon overlays are rebuilt over retained and replacement faces when they still
form one valid raw-edge-connected region.

`FillMeshHole` follows the closed naked boundary containing a picked topology
edge and fills it with a constrained-Delaunay triangle patch. Use `Edge=1`,
pick near an edge, or enter the bare command for a one-pick viewport workflow.
`JoinMesh=Yes` (the default) keeps the source identity, attributes, groups, and
selection; `JoinMesh=No` creates a separately selected patch with the source
attributes. The joined representation preserves Rhino's duplicated raw
boundary storage while exact-location topology closes the seam. Patch winding
is made consistent with the source, tilted and mildly nonplanar boundaries are
projected stably, and ambiguous branched or self-crossing boundaries are
rejected atomically. Existing source n-gons are retained in the joined mesh.

`FillMeshHoles` fills every simple closed naked boundary on every selected
mesh, including outer borders, and keeps each repaired mesh's identity,
attributes, groups, and selection. It stages the full selection atomically,
leaves already-closed meshes unchanged, and rejects ambiguous branched or
self-crossing boundary topology rather than guessing a repair.
Existing source n-gons remain attached through each filled boundary.

`PatchSingleFace Vertices=0,1,2`, `PatchSingleFace Vertex=2 Edge=0`, and
`PatchSingleFace Edges=0,5` create one triangle or quad from selected mesh
topology components. The chosen face must use at least one naked edge and must
oppose existing boundary winding; nonmanifold sides and crossing quads are
rejected. `JoinMesh=Yes` appends the patch to the source while preserving its
identity, attributes, vertex colors, and n-gons. `JoinMesh=No` creates a
separate mesh with copied object attributes. The indices are deterministic
exact-location topology indices, matching the mesh edge and vertex APIs.
