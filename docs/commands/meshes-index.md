# Meshes and SubDs

[All commands](README.md) · [Command input](../command-line.md)

- [PolylineOnMesh drawing](polyline-on-mesh.md)
- [MeshPatch from curves, points and point clouds](mesh-patch.md)
- [MeshFromLines from connected line networks](mesh-from-lines.md)
- [MeshPolyline from closed polylines](mesh-polyline.md)
- [3DFace from corner points and boundary edges](3dface.md)
- [SubDBox primitives and rectangle construction modes](subd-box.md)
- [SubDPlane grids and rectangle construction modes](subd-plane.md)
- [SubDSphere styles, sphere construction and reference inputs](subd-sphere.md)
- [SubDCone: rounded cones, caps, angle and reference inputs](subd-cone.md)
- [SubDCylinder](subd-cylinder.md): circle bases, caps, face counts and spatial axes.
- [PlanarMesh from planar curve regions](planar-mesh.md)
- [CullDegenerateMeshFaces](cull-degenerate-mesh-faces.md)
- [Polygon meshes](meshes.md)
- [Mesh conversion to NURBS](mesh-to-nurb.md)

- [Mesh curve pulling and qualifications](pull-mesh.md)

- [Mesh projection and qualification](project-mesh.md)

## Examples

These are independent snippets. Prepare the required sources and selection first;
replace object IDs, names and paths with values from your document. See each
command page for supported options and limitations.

```text
Mesh Density=0.5 JaggedSeams=No SimplePlanes=No
MeshBox 0,0,0 8,5,0 3 XCount=4 YCount=3 ZCount=2
MeshCone 0,0,0 3 8 VerticalFaces=4 AroundFaces=16 Solid=Yes CapFaceStyle=Quad
MeshTruncatedCone 0,0,0 3 8 1.5 VerticalFaces=4 AroundFaces=16 Solid=Yes CapFaceStyle=Quad
MeshCylinder 0,0,0 3 8 VerticalFaces=4 AroundFaces=16 Solid=Yes CapFaceStyle=Quad
MeshPlane 0,0,0 8,5,0 XCount=8 YCount=5
MeshSphere 0,0,0 3 Style=UV VerticalFaces=12 AroundFaces=24
MeshSphere 0,0,0 3 Style=Quads Subdivisions=3
MeshSphere 0,0,0 3 Style=Triangles Subdivisions=3
MeshEllipsoid 0,0,0 5 3 2 VerticalFaces=12 AroundFaces=24 CapFaceStyle=Quad
MeshTorus 0,0,0 5 1.5 VerticalFaces=12 AroundFaces=24
OffsetMesh 1 Solid=Yes DeleteInput=No
AddNgonsToMesh PlanarTolerance=0.01
DeleteMeshNgons
MeshToNURB TrimTriangularFaces=Yes UseNgons=Yes
DupMeshEdge All Output=Polylines
DupMeshHoleBoundary Boundaries=All
ExtractMeshEdges ExtractBy=Unwelded JoinResults=Yes
ExtractMeshFaces Faces=2,0 MakeCopy=No
ExtractConnectedMeshFaces Face=0 Angle=0 Compare=Less MakeCopy=Yes
ExtractMeshPart Face=0 ExtractToNonManifoldEdges=Yes MakeCopy=Yes
ExtractMeshPart Faces=0,2 JoinOutput=No
ExtractMeshFacesByArea LargerThan=0 SmallerThan=2 MakeCopy=Yes
ExtractMeshFacesByAspectRatio AspectRatio=9 BorderOnly=Yes
ExtractMeshFacesByDraftAngle StartAngle=0 EndAngle=45 ViewDirection=0,0,1 MakeCopy=Yes
ExtractMeshFacesByEdgeLength EdgeLength=0.1 Select=Shorter BorderOnly=Yes
TriangulateMesh
QuadrangulateMesh Planarity=1 Rectangularity=2
SwapMeshEdge Edge=1
CollapseMeshEdge Edge=1
SplitMeshEdge Edge=1 Parameter=0.25
FillMeshHole Edge=1 JoinMesh=Yes
FillMeshHoles
UnifyMeshNormals
CombineIdenticalMeshVertices
CullUnusedMeshVertices
SplitDisjointMesh
ExtractDuplicateMeshFaces
ExtractNonManifoldMeshEdges
ExtractNonManifoldMeshEdges ExtractHangingFacesOnly=Yes MinimumFaceCount=3
SubDSphere Style=Quad Subdivisions=1 0,0,0 3
SubDSphere Style=UV AroundFaces=8 VerticalFaces=4 0,0,0 3
```
