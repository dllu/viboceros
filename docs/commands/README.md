# Command reference

Commands are case-insensitive. Enter `Help` in the application to list registered
commands. These pages describe implemented behavior and known limitations;
Rhino's complete command set is still a work in progress.

- [Curve creation and editing](curves.md)
- [NURBS structure and parameterization](nurbs.md)
- [Surfaces and solids](surfaces.md)
- [Polygon meshes](meshes.md)
- [Transforms and arrays](transforms.md)
- [Splitting curves and surfaces](split.md)
- [Extraction, measurement, and intersections](editing.md)
- [Selection, attributes, layers, and groups](document.md)

See [viewport controls](../interface.md) for picking and interactive input, and
[file formats](../file-formats.md) for import/export capabilities.

## Command-line examples

```text
Point 1,2,0
Line 0,0,0 10,5,0
Circle 0,0,0 5
Arc 5,0,0 0,5,0 -5,0,0
Ellipse 0,0 6,0 0,3
Polyline 0,0 4,0 4,3 7,3
Rectangle 0,0 8,5
Polygon 6 0,0 5
Curve 0,0 2,3 5,3 8,0 Degree=3
ControlPointCurve 3 0,0 2,3 5,3 8,0
InterpCrv 0,0 1,2 4,-1 6,0 Knots=Chord Close=Open
CurveThroughPt CurveType=Interpolated Knots=Chord Closed=No
CurveThroughPolyline Degree=5 CurveType=ControlPoint DeleteInput=No
TweenCurves Number=3 MatchMethod=SamplePoints SampleNumber=100 OutputLayer=CurrentLayer
FitCrv Degree=3 Tolerance=0.001 AngleTolerance=1 DeleteInput=No OutputLayer=CurrentLayer
Rebuild PointCount=10 Degree=3 PreserveTangents=No DeleteInput=Yes OutputLayer=InputObject
ChangeDegree 5,3 Deformable=No
MakeUniform
MakeUniformUV Direction=U
MakePeriodic Smooth=Yes DeleteInput=Yes
MakeNonPeriodic
InsertControlPoint 5.2,1.1,0 Direction=U Midpoint=No
InsertKnot 0.52,3.1 Multiplicity=2 Direction=Both Symmetrical=No
RemoveKnot 0.52,3.1 Direction=V
RemoveControlPoint 3 Direction=U
RemoveMultiKnot RemoveFullyMultipleKnots=Yes MaxKinkAngle=5
SrfPt 0,0,0 8,0,0 8,5,2 0,5,2
SrfPtGrid DegreeU=1 2 DegreeV=1 3 0,0,0 0,1,1 0,2,0 3,0,0 3,1,2 3,2,0
SrfControlPtGrid Degree=1 2 Degree=1 3 0,0,0 0,1,1 0,2,0 3,0,0 3,1,2 3,2,0
PlanarSrf DeleteInput=No
Loft Type=Normal Closed=No
Sweep1 RailName=Rail Parameters=0 RefitRail=No
EdgeSrf
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
MeshToNURB TrimTriangularFaces=Yes UseNgons=Yes
Box 0,0,0 8,5,0 3
BoundingBox CoordinateSystem=World Cumulative=Yes Output=Solids
DupBorder OutputLayer=Current
DupEdge Edges=2,0 OutputLayer=Input
DupMeshEdge All Output=Polylines
DupFaceBorder Faces=2,0 OutputLayer=Input
DupMeshHoleBoundary Boundaries=All
Sphere 0,0,0 5
Ellipsoid 0,0,0 5 3 2
Cylinder 0,0,0 5 10 Solid=Yes
Cone 0,0,0 5 10 Solid=Yes
Conic 0,0,0 10,0,0 5,5,0 0.4
Parabola Vertex 0,0,0 0,0,1 4,0,0 Half=No MarkFocus=Yes
Parabola3Pt -1,0,0.25 1,0,0.25 3,0,2.25 1,0,1.25 MarkFocus=Yes
Hyperbola 0,0,0 5,0,0 3.75,3,0 BothBranches=Yes MarkFoci=Yes
Helix 0,0,0 0,0,10 2 Turns=3 ReverseTwist=No
Spiral 0,0,0 0,0,6 1 4 Turns=2 ReverseTwist=No
Spiral AroundCurve 1,0,0 2 PathName=Rail Turns=3 PointsPerTurn=12
Catenary 0,0,0 10,0,0 0,0,-1 4 Mode=Parameter PointCount=20
Paraboloid Vertex 0,0,0 0,0,1 4,0,0 MarkFocus=Yes Solid=Yes
TruncatedCone 0,0,0 5 10 2.5 Solid=Yes
Pyramid 5 0,0,0 5 10 Solid=Yes
TruncatedPyramid 5 0,0,0 5 10 2.5 Solid=Yes
Tube 0,0,0 3 1 10
Torus 0,0,0 5 1.5
Layer New Construction
Layer Hide Construction
Layer Show Construction
Layer Current Default
ChangeLayer Construction
CopyToLayer Default
SelAll
SelLast
SelPrev
SelLast DeselectOthersBeforeSelect=No
SelCrv
SelOpenCrv
SelClosedCrv
SelPlanarCrv
SelLine
SelPolyline
SelShortCrv 1.0
SelPt
SelPtCloud
SelSrf
SelPolysrf
SelOpenPolysrf
SelClosedPolysrf
SelMesh
SelOpenMesh
SelClosedMesh
SelColor 12,34,56
SelName "Fastener *"
SelLayer "Construction *"
SelGroup Assembly
SelDup
SelDupAll
Invert
Move 0,0,0 5,0,0
Copy 5,0,0 5,5,0
Orient 0,0,0 1,0,0 5,5,0 5,8,0 Scale=1D Copy=Yes
Orient3Pt 0,0,0 1,0,0 0,1,0 5,5,0 5,8,0 4,5,1 Scale=No
OrientOnSrf 0,0,0 1,0,0 5,5,2 Rigid=No SurfaceName=Panel
Array 3 2 2 4 -3 5
Array 3 2 1 20 12 0 Mode=Fill
ArrayCrv 8 Orientation=Freeform PathName=Rail
ArrayCrv Distance=2.5 Orientation=Roadlike BasePoint=0,0,0 PathName=Rail
ArraySrf 4 3 BasePoint=0,0,0 Up=0,0,1 Mode=Isocurve SurfaceName=Panel
ArrayLinear 4 0,0,0 2,1,0
ArrayPolar 6 0,0,0 360 Rotate=Yes ZOffset=0
Scale 0,0 2
Scale1D 0,0 2 1,0
Scale2D 0,0 2
ScaleNU 0,0,0 2 .5 1 Copy=Yes
Rotate 0,0 45
Rotate3D 0,0,0 0,0,1 90 Copy=Yes
Mirror 0,-5 0,5
Shear 0,0,0 1,0,0 45 Copy=Yes
ProjectToCPlane DeleteInput=Yes
ToNURBS DeleteInputObjects=Yes
ExtrudeCrv 5 BothSides=No DeleteInput=No
ExtrudeCrvToPoint 0,0,10 DeleteInput=No
ExtrudeCrvAlongCrv PathName=Rail DeleteInput=No
Revolve 0,0,0 0,0,1 270 StartAngle=0 DeleteInput=No
Group Assembly
Group All Everything
SetObjectName "Fastener Part" AppendCounter=Yes
SetObjectColor 12,34,56
SetObjectColor ByLayer
Ungroup
Ungroup Assembly
Join
Explode
Length
Curvature MarkCurvature=No 2,0,0
Area
Volume
Divide 8
Divide Length 2.5 MarkEnds
CrvStart
CrvEnd
ExtractPt OutputLayer=Input Output=Points
ExtractPt OutputLayer=Input Output=PointCloud
ExtractControlPolygon OutputLayer=Current
ExtractSrf 2,1,0 Copy=No OutputLayer=Input
ExtractSrf Faces=0,2 Copy=Yes OutputLayer=Current
ExtractMeshEdges ExtractBy=Unwelded JoinResults=Yes
ExtractMeshFaces Faces=2,0 MakeCopy=No
DeleteFaces Faces=2,0
TriangulateMesh
SwapMeshEdge Edge=1
CollapseMeshEdge Edge=1
SplitMeshEdge Edge=1 Parameter=0.25
FillMeshHole Edge=1 JoinMesh=Yes
FillMeshHoles
ExtractIsocurve 2,1,0 Direction=Both
ExtractIsocurve ExtractAll Direction=Both IgnoreTrims=No
ExtractWireframe OutputLayer=Current GroupOutput=No
ConvertToSingleSpans Direction=Both DeleteInput=No
ConvertToBeziers DeleteInput=No
CloseCrv
CloseCrv CloseWideGapsWithLine=No Tolerance=0.01
CrvSeam 4,1,0
SrfSeam 5,0,0 Direction=U
SubCrv 8,0,0 2,0,0 Copy=Yes
Split 4,0,0 7,0,0
Split CuttingObjects=1,0,0
Split Isocurve=4,6,0 Direction=Both Shrink=Yes
Intersect
Trim 5,0,0
Extend Length=5 Side=End Type=Natural Join=Merge
Extend Length=2 Side=Both Type=Line Join=Merge
Extend Length=2 Side=Both Type=Smooth Join=Merge
Extend Length=2 Side=End Type=Arc Join=Yes
Extend Length=2 Side=Both Type=Line Join=Yes
Extend Length=2 Side=Both Type=Line Join=No
Extend 5,0 Type=Line Join=Merge
ExtendSrf Direction=U Domain=-1,2 Type=Smooth Merge=Yes
ExtendSrf Edge=East Distance=3 Type=Smooth Merge=Yes
ExtendSrf Edge=West Distance=2 Type=Line Merge=Yes
ExtendSrf Edge=East Distance=-2 At=0.5
ExtendSrf Edge=East Distance=2 Type=Smooth Merge=No
ExtendSrf Distance=-2 Type=Smooth Merge=Yes
Reparameterize -4 6
Reparameterize Automatic
Dir SwapUV
Dir Mode=FlipU
Flip
UnifyMeshNormals
Weld 180
WeldEdge Edges=0,2
WeldVertices Vertices=0,2
Unweld 45 ModifyNormals=Yes
UnweldEdge Edges=0,2 ModifyNormals=Yes
UnweldVertex Vertices=0,2 ModifyNormals=Yes
CombineIdenticalMeshVertices
CullUnusedMeshVertices
SplitDisjointMesh
ExtractDuplicateMeshFaces
ExtractNonManifoldMeshEdges
ExtractNonManifoldMeshEdges ExtractHangingFacesOnly=Yes MinimumFaceCount=3
Hide
Show
Lock
Unlock
HideSwap
LockSwap
Isolate
Unisolate
IsolateLock
UnisolateLock
Delete
Clear
Undo
Redo
ImportStl path/to/model.stl
ExportStl Binary path/to/model.stl
ExportStl Ascii path/to/model.stl
ImportStep path/to/model.step
ExportStep path/to/model.step
Import3dm path/to/model.3dm
Export3dm path/to/model.3dm
Help
```
