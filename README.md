# Viboceros

Viboceros is an early-stage, cross-platform CAD application written in Rust. It
is organized around independent geometry, document, drafting, command, and
file-format crates, with an egui interface rendered by wgpu.

The current foundation supports finite 3D points, vectors, line segments,
analytic circles, circular arcs, and ellipses, validated open and closed
polylines, planes, bounding boxes, rational NURBS curves with analytic first
and second derivatives and exact knot refinement, splitting, and interval trimming,
rational NURBS surfaces with analytic partial derivatives and exact tensor
splitting, knot refinement, and rectangular domain trimming,
validated shared-topology B-reps with exact rational parameter-space trims,
validated mixed triangle/quad polygon meshes, layers, groups, and bounded
undo/redo.
Native point clouds preserve point order and duplicates, cache finite bounds,
and use a balanced XY spatial index for snapping and picking. The UI opens with
Rhino's usual four-view layout: Top, Perspective, Front, and Right. Each
viewport has independent pan, zoom, projection, and wireframe, shaded, or
ghosted display settings. The layer sidebar creates, renames, recolors, shows,
locks, activates, and safely deletes layers while reporting their object
counts; combined edits remain one undo step. The command line currently accepts:

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
PlanarSrf DeleteInput=No
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

Enter `Point`, `Line`, `Circle`, `Arc`, `Ellipse`, `Polyline`, `Curve`,
`InterpCrv`, `Rectangle`, `Polygon`, `SrfPt`, `Sphere`, or `Ellipsoid` without
coordinates to pick points in the viewport; press Enter to finish a polyline or curve. `Curve`
creates an open control-point curve, defaults to degree three, accepts degrees
through 11, and lowers the degree when too few controls are supplied.
`Close=Smooth` creates a periodic seam (degree one remains linear and closed);
`Close=Sharp` repeats the first control to create a kinked, non-periodic seam.
`InterpCrv` defaults to an open, degree-three, chord-knot curve through the
input points.
It also supports `Degree=1`, `Knots=Uniform|SqrtChrd`, smooth periodic or sharp
`Close=` modes, and start/end tangent directions on open cubics.
`CurveThroughPt` orders selected point objects into a nearest-neighbor chain,
ignores coincident locations, and creates one control-point or
interpolated curve; `Closed=Yes` requests a smooth closed seam (degree-one
output remains linear and non-periodic). `CurveThroughPolyline`
creates one curve through each selected polyline's vertices, inheriting whether
each source is open or closed. Both commands support control-point degrees 1
through 11 (lowered when necessary), while interpolated output currently
supports degrees 1 and 3 with uniform, chord, or square-root-chord spacing.
Polyline sources are retained by default; `DeleteInput=Yes` replaces their
geometry in place while preserving identity and attributes.
`TweenCurves` requires exactly two selected curves, uses their selection order,
and leaves both sources selected. `MatchMethod=None` connects corresponding
NURBS control locations and weights, including Rhino's last-control rule when
the counts differ. `MatchMethod=SamplePoints` supports 2 through 9999
equal-length divisions with bounded, linear-memory interpolation.
`OutputLayer=` accepts `CurrentLayer`, `StartCrv`, or `EndCrv`; `FlipStart=Yes`
and `FlipEnd=Yes`
reverse either temporary source direction. `MatchMethod=Refit` adaptively
rebuilds both sources as non-rational cubics on one bounded shared
structure, retaining source-span boundaries and qualifying kinks while meeting
the document tolerance before their corresponding controls are blended.
`FitCrv` adaptively approximates every selected line, analytic curve,
polyline, or NURBS curve with a non-rational NURBS curve of degree 1 through
11. Output parameters approximate source arc length; endpoints, endpoint
tangents, and polyline/NURBS kinks above `AngleTolerance` (in degrees) are
preserved. `Tolerance` defaults to the document absolute tolerance.
`DeleteInput=Yes` and `OutputLayer=InputObject` are the Rhino-compatible
defaults and replace each result in place; `DeleteInput=No` retains the source,
while `OutputLayer=CurrentLayer` puts fresh results on the current layer.
`Rebuild` reconstructs selected curves as uniform, non-rational NURBS curves
with a fixed `PointCount` (default 10) and `Degree` from 1 through 11 (default
3). Open results are clamped and closed results use Rhino-compatible periodic
structure; both interpolate equal-arc-length source stations. Positive point
counts below the structural minimum are raised automatically, as in Rhino.
`PreserveTangents=Yes` aligns eligible open-curve end handles. The
`DeleteInput` and `OutputLayer` options follow `FitCrv`; surface rebuilding is
not yet represented.
`ChangeDegree degree|u_degree,v_degree Deformable=Yes|No` changes selected
curves and untrimmed NURBS surfaces in place and defaults to `Deformable=No`.
A scalar applies to curves and both surface directions; with a pair, the first
degree also applies to curves. Non-deformable elevation preserves exact
geometry and parameterization while raising knot multiplicities. Lowering, or
`Deformable=Yes`, keeps distinct knot breaks as simple knots and interpolates
the source in homogeneous space at the new Greville abscissae. Degree changes
support values 1 through 11 and can turn periodic directions into clamped
ones, matching Rhino.
`MakeUniform` replaces selected curves and untrimmed NURBS surfaces in place
with the same degree, control locations, and rational weights but
Rhino-compatible unit-spaced knots. Start and end clamping are retained
independently, periodic topology stays periodic, and supported analytic curves
are first converted to their exact NURBS form. Both surface directions are
changed. `MakeUniformUV Direction=U|V` performs the same operation on selected
untrimmed NURBS surfaces in one direction only and defaults to U. Changing knot
spacing can change the object shape.
`MakePeriodic Smooth=Yes|No DeleteInput=Yes|No` converts selected closed
degree-two-or-higher curves and one eligible closed direction of each selected
untrimmed NURBS surface in place; smoothing defaults to Yes. U is chosen first
when both surface directions are eligible, so running the command again
converts V. `Smooth=Yes` retains active knot breaks and solves the seam in
homogeneous space, while `Smooth=No` retains existing controls as closely as
Rhino permits and redistributes seam knots. Active domains are preserved, and
rational inputs are solved without discarding their weights. `DeleteInput=Yes`
replaces inputs in place by default; No retains the selected sources and adds
unselected copies with their attributes and group memberships. Both modes
support undo.
`MakeNonPeriodic` converts every selected periodic NURBS curve or surface to
the equivalent clamped form in place. It preserves the active domains,
parameterization, shape, object identity, attributes, and selection; surfaces
are clamped in every periodic direction.
`InsertControlPoint point Direction=U|V|Both Midpoint=Yes|No` requires exactly
one selected curve or untrimmed NURBS surface. The model-space point is
projected to the object, then a unit-weight control is inserted between the
bracketing control-point Greville parameters, matching Rhino and generally
changing shape. `Midpoint=Yes` snaps the new control and knot to the middle of
that interval; omit the point to pick it in a viewport. On surfaces, Direction
names the row orientation as it does in Rhino (a U row adds a V-axis control);
U is the default. Rational and periodic inputs, object identity, attributes,
selection, and undo are preserved.
`InsertKnot` requires exactly one selected curve or untrimmed NURBS surface and
refines it in place without changing its parameterization or shape. Curves take
one parameter; surfaces take `u,v`, with a scalar accepted when `Direction=U`
or `V`. `Multiplicity` is the target knot multiplicity and may range from one
through the relevant degree. `Symmetrical=Yes` also inserts the parameter
mirrored across the active domain. Rational refinement and periodic repair
follow Rhino/OpenNURBS behavior; identity, attributes, selection, and undo are
preserved.
`RemoveKnot parameter|u,v Direction=U|V` requires exactly one selected curve
or untrimmed NURBS surface and removes one knot in place. On curves, the
parameter identifies a curve point and the knot with the nearest model-space
point is selected, matching Rhino. On surfaces, U is the default and the knot
value nearest the chosen coordinate is removed consistently across the whole
control net. Remaining homogeneous controls are interpolated at Greville
abscissae; rational weights, object identity, attributes, selection, and undo
are preserved. Periodic directions are rejected, while non-clamped directions
are first clamped without changing their active domain.
`RemoveControlPoint index Direction=U|V` requires exactly one selected curve
or untrimmed NURBS surface and removes a zero-based control-point grip in
place. On a surface, the complete row at that index is removed in the chosen
direction, which defaults to U. Remaining controls are retained, with
Rhino-compatible knot updates, endpoint-weight normalization, single-span
degree lowering, and periodic topology repair. The operation generally changes
shape while preserving object identity, attributes, selection, and undo.
`RemoveMultiKnot RemoveFullyMultipleKnots=Yes|No MaxKinkAngle=0..180`
reduces qualifying stacked knots on every selected curve and untrimmed NURBS
surface. By default, only non-full multiple knots are collapsed to simple
knots. Enabling full removal also merges kinks or surface creases whose tangent
angle is strictly below `MaxKinkAngle` in degrees (default 1 degree),
and merges eligible degree-one spans into a single linear span. Surface U and V
directions are both processed using Rhino/OpenNURBS continuity samples.
Rational weights, object identity, attributes, selection, and one-step undo
are preserved; periodic directions are rejected atomically. Non-clamped inputs
are first clamped to their existing active domains so the output knot vectors
remain valid.
`Polygon`
defaults to four sides, or accepts a side count such as `Polygon 6`. With
objects selected, enter `Move` or `Copy`
to pick a base and destination point, `Scale`, `Scale1D`, `Scale2D`, or
`Rotate` to pick center/reference/target points, `Mirror` to pick a two-point
axis, or `ArrayLinear 4` to pick its two spacing references. `ScaleNU` accepts
independent world x/y/z factors; all scale variants accept `Copy=Yes`.
`Rotate3D` picks an axis start/end followed by angle reference/target points;
`Rotate`, `Rotate3D`, and `Mirror` also accept `Copy=Yes`.
`Shear` picks a fixed origin, reference direction, and target angle in the top
view; its third argument can instead be a numeric angle, and it accepts
`Copy=Yes`.
`ProjectToCPlane` flattens onto the current construction plane (World XY in the
current UI). It retains the inputs by default; use `DeleteInput=Yes` to project
them in place.
`ToNURBS` exactly converts selected lines, circles, arcs, ellipses, and
polylines. It retains inputs and creates unselected copies in the same groups
by default; use `DeleteInputObjects=Yes` to preserve object identities and
replace the inputs in place. Line, polyline, circle, arc, and ellipse parameter
domains follow Rhino's chord-length, arc-length, and angular conventions.
`PlanarSrf` creates exact single-face B-reps from selected closed planar curves.
Wholly contained curves become inner trim loops, alternating nested regions
become independent islands, and partially overlapping curves remain separate
surfaces. Every original rational curve remains an exact boundary edge and
parameter-space trim, so curved, concave, and holed regions shade and export
without filling excluded areas. Inputs are retained by default;
`DeleteInput=Yes` removes each successfully used boundary.
`Mesh` creates editable polygon meshes from selected NURBS surfaces and B-reps
while retaining the selected sources. The new meshes copy source names, layers,
colors, and display attributes, remain unselected, and do not inherit source
groups, matching Rhino's derived-object behavior. `Density=0..1` selects a
bounded per-knot-span sampling level; `SimplePlanes=Yes` minimizes entirely
planar inputs, and `JaggedSeams=Yes` disables shared-edge snapping on B-reps.
Regular surface cells remain quadrilaterals, singular sides and planar trim
regions use triangles, and smooth-seam closed solids must remain watertight.
Unsupported nonplanar general trims fail atomically rather than being silently
meshed as untrimmed surfaces.
`MeshBox` draws an unselected closed quadrilateral mesh from two opposite
World-XY base corners and a signed height or height point. `XCount`, `YCount`,
and `ZCount` set the side divisions and default to 1. Each of Rhino's bottom,
top, front, right, back, and left grids retains independent raw vertices while
exact-location topology forms one outward-oriented solid; one box is bounded
to one million faces and invalid extents fail atomically.
`MeshCone` draws a polygonal cone from a base center, numeric or picked radius,
and signed apex height. `VerticalFaces` and `AroundFaces` default to 10,
`Solid=Yes` adds a topology-joined base cap, and `CapFaceStyle=Tri|Quad` selects
triangle fans or Rhino's even-sided quad fan; odd side counts fall back to
triangles. `Axis=x,y,z` orients typed commands, the three-pick toolbar workflow
uses World Z, and one cone is bounded to one million faces.
`MeshTruncatedCone` draws a polygonal frustum from a base center, numeric or
picked base radius, signed height, and positive end radius. Radii interpolate
linearly through the height-major wall rings. `VerticalFaces` and `AroundFaces`
default to 10; `Solid=Yes` adds independent, topology-joined end caps, and
`CapFaceStyle=Tri|Quad` follows Rhino's even-sided quad fans and odd-count
triangle fallback. `Axis=x,y,z` orients typed commands, including the winding
direction for negative heights. The four-pick toolbar workflow uses World Z,
and one truncated cone is bounded to one million faces.
`MeshCylinder` draws a polygonal cylinder from a center, numeric or picked
radius, and signed height. `VerticalFaces` and `AroundFaces` default to 10,
`Solid=Yes` adds independently stored but topology-joined caps, and
`CapFaceStyle=Tri|Quad` chooses triangle fans or Rhino's even-sided quad fan.
Odd side counts always fall back to triangles. `BothSides=Yes` mirrors the
height about the base plane, `Axis=x,y,z` orients typed commands, and the
three-pick toolbar workflow uses World Z. Output is consistently oriented and
bounded to one million faces.
`MeshPlane` draws an unselected quadrilateral grid from two opposite top-view
corners. `XCount` and `YCount` specify face counts and default to 10; corner
order is normalized, the second corner is projected to the first corner's
elevation, and vertices/faces follow Rhino's x-fastest row-major ordering.
One grid is bounded to one million faces and tolerance-degenerate cells are
rejected atomically.
`MeshSphere` creates all three Rhino polygon-sphere styles from a center and
numeric or picked equator radius. `Style=UV` uses a latitude-major quad grid
with shared triangle-fan poles; `VerticalFaces` and `AroundFaces` default to
10. `Style=Quads` applies Catmull-Clark cube refinement before radial
projection, while `Style=Triangles` recursively refines an icosahedron; both
use `Subdivisions=3` by default and preserve Rhino's welded indexing. The
command limits subdivisions to 6 quads or 5 triangles, `Axis=x,y,z` orients
typed commands, and the two-pick toolbar workflow uses World Z. One sphere is
bounded to one million faces.
`MeshEllipsoid` creates a closed mesh from three positive semi-axis radii or
from a center and three axis-radius points. `VerticalFaces` divides the first
axis from pole to pole, `AroundFaces` divides each rationally parameterized
ring, and both default to 10. `CapFaceStyle=Tri|Quad` selects triangle fans or
Rhino's paired quad pole faces; odd around counts fall back to triangles.
Vertices and faces preserve Rhino's NURBS-parameter sampling and welded index
order. The four-pick toolbar workflow retains these options, and one ellipsoid
is bounded to one million faces.
`MeshTorus` creates a closed quadrilateral ring torus from a center, numeric or
picked major radius, and a positive minor radius smaller than the major radius.
`VerticalFaces` divides the tube circle, `AroundFaces` divides the major circle,
and both default to 10. Vertices and seam-wrapped faces retain Rhino's periodic
row-major ordering. `Axis=x,y,z` orients typed commands; the three-pick toolbar
workflow uses World Z and measures the tube radius from the major-radius point.
One torus is bounded to one million faces and invalid radii fail atomically.
`MeshToNURB` duplicates every selected mesh polygon as an exact degree-one
NURBS face. Quads remain potentially warped bilinear surfaces; triangles are
trimmed planar parallelograms by default, or untrimmed patches with a collapsed
side when `TrimTriangularFaces=No`. Exact-location edges become shared B-rep
topology and edge-disconnected pieces become separate, unselected objects.
Sources and their selections are retained, derived attributes are copied, and
source groups are not inherited. `UseNgons` is accepted for Rhino script
compatibility; the current native mesh model contains triangles and quads only.
`Sphere` creates Rhino/OpenNURBS' exact 9-by-5 rational quadratic surface from
a center and numeric radius or point on the sphere. Its longitude domain is
`[0, 2π]`, its latitude domain is `[-π/2, π/2]`, and entering it without
arguments starts the two-pick viewport workflow.
`Ellipsoid` creates the exact closed 9-by-5 rational surface obtained by
scaling that sphere along three orthogonal semi-axes. Supply three numeric
radii in World XY, or center and three axis-radius points for an oriented
ellipsoid; entering it without arguments starts the four-pick viewport workflow.
`Box` follows Rhino's default two-opposite-base-corners and height workflow in
World XY. The height may be a signed number or a point, and the result is one
closed B-rep with eight shared vertices, twelve shared edges, six outward
bilinear faces, and exact rational parameter-space trims.
`BoundingBox` creates one cumulative World-coordinate enclosure of the selected
objects by default; `Cumulative=No` creates one per object. `Output=` supports
exact B-rep solids, shared-vertex triangle meshes, six grouped rectangle curves,
or report-only `None`. World-plane selections produce a rectangle or mesh plane.
`CoordinateSystem=CPlane` is accepted and currently shares the World XY basis;
analytic bounds are tight, while NURBS bounds use their control geometry and
can be non-conservative for negative-weight projective inputs.
`DupBorder` duplicates the open boundaries of selected NURBS surfaces, B-reps,
and triangle meshes. Surface borders are exact rational isocurves, including
at non-clamped domain ends; closed seams and collapsed singular sides are
omitted. B-reps preserve each exact naked edge, while mesh edges are welded by
exact location into boundary polylines. Multi-edge borders are grouped until
the document gains an exact polycurve primitive. Results use the current layer
and become selected by default; `OutputLayer=Input` uses each source layer.
`DupEdge` duplicates the exact edge nearest a model-space point, or accepts an
ordered zero-based `Edges=0,2`/`Edges=All` selector for every selected NURBS
surface, B-rep, or mesh. Standalone closed-surface seams remain selectable,
collapsed singular sides are omitted, B-rep rational edge curves stay exact,
and mesh indices follow exact-location-welded topology order. Omit the selector
for a one-pick viewport workflow. Fresh selected results default to the current
layer; `OutputLayer=Input` uses each source layer.
`DupMeshEdge` duplicates the logical edge nearest a model-space point on the
selected polygon meshes. `BreakAngle=90` is the default: incident faces below
the angle are locally one smooth region, while creases at the angle, naked
borders, and unwelded seams remain boundaries. Omit the point for a one-pick
viewport workflow. `All` instead duplicates every naked/unwelded edge, with
`Output=Polylines` joining edge-exact trails or `Output=Lines` retaining
individual segments. Fresh selected results use the current layer and sources
are deselected.
`DupMeshHoleBoundary` duplicates the closed naked loop nearest a model-space
point on selected polygon meshes, or accepts ordered zero-based
`Boundaries=0,2`/`Boundaries=All` selectors for every selected mesh. Boundaries
use exact-location-welded topology, remain closed polylines in deterministic
topology order, and fresh selected results use the current layer. Omit the
selector for a one-pick viewport workflow.
`DupFaceBorder` duplicates the exact non-seam border of the nearest selected
surface or B-rep face, or accepts ordered zero-based `Faces=0,2`/`Faces=All`
selectors. Omit the selector for a one-pick viewport workflow. Linear edge
chains become one closed polyline; curved multi-edge chains retain exact NURBS
segments in a group until polycurves are available. Holes and disconnected
borders remain separate, singular and seam trims are omitted, and fresh
selected results default to the current layer (`OutputLayer=Input` is also
supported).
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
unchanged. Both parts compact unused vertices in Rhino source order.
`DeleteFaces` removes an ordered zero-based face list from every selected mesh
or B-rep (`Faces=All` is supported), or omits the selector for a one-pick
viewport workflow. A partial edit keeps the unselected source object's
identity, attributes, groups, and surviving source face order. Mesh results
compact unused vertices in source order; deleting every face removes the
object. SubD input awaits a native SubD geometry type.
`TriangulateMesh` splits every quad on selected meshes along its shortest 3D
diagonal, choosing A-C on exact ties. First triangles replace their source
quads in place and second triangles append in source-quad order; vertices,
object identity, attributes, groups, and selection remain unchanged.
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
`SplitMeshEdge` divides a selected topology edge at a normalized parameter in
its deterministic wireframe direction. Use `Edge=1 Parameter=0.25`, provide an
edge point followed by a split point, or enter the bare command for the same
two-pick viewport workflow. Affected triangles become two triangles and quads
become three; unaffected faces stay first and replacements append in Rhino
order. Welded faces share one appended split vertex, while unwelded replacement
triangles remain fully separated. Exact endpoint parameters preserve Rhino's
coincident topology behavior. Object identity, attributes, groups, selection,
and undo are retained; tolerance-degenerate results are rejected atomically.
`FillMeshHole` follows the closed naked boundary containing a picked topology
edge and fills it with a constrained-Delaunay triangle patch. Use `Edge=1`,
pick near an edge, or enter the bare command for a one-pick viewport workflow.
`JoinMesh=Yes` (the default) keeps the source identity, attributes, groups, and
selection; `JoinMesh=No` creates a separately selected patch with the source
attributes. The joined representation preserves Rhino's duplicated raw
boundary storage while exact-location topology closes the seam. Patch winding
is made consistent with the source, tilted and mildly nonplanar boundaries are
projected stably, and ambiguous branched or self-crossing boundaries are
rejected atomically.
`FillMeshHoles` fills every simple closed naked boundary on every selected
mesh, including outer borders, and keeps each repaired mesh's identity,
attributes, groups, and selection. It stages the full selection atomically,
leaves already-closed meshes unchanged, and rejects ambiguous branched or
self-crossing boundary topology rather than guessing a repair.
`ExtractControlPolygon` fits degree-one polylines through the Euclidean controls
of selected curves and creates mixed triangle/quad meshes through selected
untrimmed NURBS surface control nets. Periodic control windows align to the
active domain, closed seams remain explicit, and singular surface sides become
triangles without artificial quad diagonals. Results default to the current
layer; `OutputLayer=Input` and `TargetObject` use each source object's layer.
`ExtractSrf` separates the nearest exact face at a model-space point, or accepts
an ordered zero-based list such as `Faces=0,2` (`Faces=All` is also supported).
The face list applies to every selected NURBS surface or B-rep; entering the
command without a selector starts a one-pick viewport workflow. `Copy=No` is
the default: an unextracted B-rep remainder keeps the source identity,
attributes, and groups, while a fully extracted source is deleted. Extracted
faces preserve attributes, become selected independent objects, and never
inherit source groups. `Copy=Yes` retains the source. Output defaults to the
input layer; `OutputLayer=Current` changes only the result layer.
`ExtractIsocurve` creates the exact U, V, or both rational isocurves nearest a
model-space point on every selected NURBS surface or B-rep. B-rep results come
from the nearest trimmed face and are split exactly around outer boundaries and
holes; rational p-curve intersections determine the retained parameter
intervals without faceting the output. Extracted curves preserve the varying
direction's degree and parameter values even at non-clamped spans. Omit the
point to pick a surface location in the viewport. `IgnoreTrims=Yes` uses the
full underlying B-rep face instead. `ExtractAll` emits the natural boundaries,
knot wires, and density-dependent wires inside each knot span from all selected
surfaces or B-rep faces; the per-object Rhino wire density survives 3DM I/O.
The same wire-density rules drive viewport display and `ExtractWireframe`.
That command emits each B-rep or exact-location-welded mesh topology edge once,
adds exact trim-clipped interior surface isocurves, selects the results, and can
place them on the current or input layer. `GroupOutput=Yes` forms one output
group; `OutputLayer=TargetObject` currently uses the selected target's layer.
`ConvertToSingleSpans` decomposes selected untrimmed NURBS surfaces at their
exact knot spans in `Direction=U`, `V`, or `Both`. Rational weights, source
parameter values, attributes, and group membership are preserved. Inputs are
retained by default; `DeleteInput=Yes` replaces them in one undoable edit, and
surfaces already single-span in the requested direction are left untouched.
`ConvertToBeziers` decomposes selected NURBS curves and untrimmed NURBS surfaces
exactly at every nonempty knot span. It preserves rational weights, source
parameters, attributes, and group membership. Inputs are retained by default;
`DeleteInput=Yes` replaces them with fresh pieces in one undoable edit. A
single-span input still produces a fresh Bezier object.
`Cylinder` creates an exact 9-by-2 rational NURBS wall from a center, radius
(or base-circle point), and signed height. `Axis=` accepts an arbitrary axis,
while `BothSides=Yes` makes the height symmetric about the base center.
`Solid=Yes` adds exact rational polar-disk caps and shared rim/seam topology as
a closed B-rep; `Solid=No` retains the single open wall surface.
`Cone` uses the same center, numeric radius or base-point radius, signed height,
and `Axis=` conventions. It produces Rhino/OpenNURBS' exact 9-by-2 rational
wall with a weighted collapsed apex. `Solid=Yes` adds an exact polar-disk base
and shared rim/seam topology as a closed B-rep; `Solid=No` leaves the wall open.
`Conic` creates Rhino's exact normalized rational quadratic from a start, end,
apex, and either a rho value or through-point. `Apex` switches the documented
pick order; off-plane through-points are projected into the control triangle's
plane before their unique positive weight is recovered. Typed rho input also
preserves Rhino's collinear or coincident control-point degeneracies.
`Parabola` supports Rhino's default `Focus focus direction-point end-point` and
`Vertex vertex focus end-point` constructions. It creates the exact normalized
quadratic NURBS curve, projects the picked end point perpendicular to the focus
axis in vertex mode, and supports `Half=Yes` and `MarkFocus=Yes`.
`Parabola3Pt` creates the exact normalized quadratic from two endpoints and a
focus, through-point, or vertex. It matches Rhino's `ThroughPoint` default,
all three `PickOrder` forms, the additional through-point opening-direction
pick, and `MarkFocus=Yes`.
`Hyperbola` supports Rhino's center, coefficient, two-focus, and vertex input
modes. Each branch is one exact normalized rational quadratic span;
`BothBranches=Yes` and `MarkFoci=Yes` preserve Rhino's object ordering.
`ShowAsymptotes=Yes` is accepted as a preview-only option.
`Helix` creates a non-rational uniform cubic along an explicit axis. Numeric or
picked radii, arbitrary axis directions, fractional `Turns=`, `Mode=Pitch`,
and `ReverseTwist=Yes` are supported. `Spiral` extends the same workflow with
independent numeric or picked start and end radii. `Flat` creates a planar
spiral with an optional `Axis=` normal. `AroundCurve` follows a whole named or
last-selected line, analytic curve, polyline, or NURBS rail. Its first radius
must be a point to seed the radial direction; the end radius can be numeric or
picked. It supports Turns/Pitch, reverse twist, and `PointsPerTurn=` values of
at least five (12 by default).
The axial linear-time tridiagonal constructor
uses Rhino's 24-span density for long forward constant-radius helices and 36
spans otherwise, interpolates every analytic sample, and has a one-million-
control resource ceiling. Its C2 endpoint rule differs from Rhino's legacy C1
endpoint perturbation by less than `3e-5` in the permanent `helix.json` and
`spiral.json` NURBS-control oracle fixtures. Swept spirals use equal-arc-length
rail stations and rotation-minimizing frame transport; their complete cubic
control layout agrees with Rhino within `2e-7` across straight, planar curved,
and spatial curved rails in `swept_spiral.json`.
`Catenary` constructs the physical hanging-chain curve from a through point,
analytic length, catenary parameter, or apex height. The third point fixes the
gravity direction from the start point. `Output=Smooth` creates Rhino's
chord-parameterized cubic approximation with exact analytic endpoint tangents;
`Output=Polyline` samples uniform horizontal stations. Both use 20 points by
default, accept `PointCount=`, and can add the exact analytic apex with
`MarkApex=Yes`. Overflow-safe hyperbolic evaluation and bounded root solvers
cover asymmetric and arbitrarily oriented endpoint frames. All nine permanent
smooth/polyline oracle cases agree with Rhino within `2e-8` (the observed
maximum difference is below `2e-10`).
`Paraboloid` supports Rhino's `Focus focus direction-point end-point` and
`Vertex vertex focus end-point` constructions; `Focus` is the default. The
vertex form projects the end point perpendicular to the focus axis and derives
the height from that radius. Both produce Rhino's exact 9-by-3 rational
quadratic wall with an arc-length meridian domain and singular/shared seam
topology. `Solid=Yes` adds the exact planar rim cap, while `MarkFocus=Yes` adds
a point object at the focus in the same undo step.
`TruncatedCone` creates an exact rational frustum from base and end radii. Its
linear V domain is the physical slant length, matching Rhino; negative heights
reverse the construction direction while preserving the base seam. `Solid=Yes`
adds outward planar caps with Rhino-compatible shared rim and seam topology.
`Pyramid` and `TruncatedPyramid` create regular polygonal joined B-reps with
Rhino-compatible vertex, edge, face, trim, and planar-surface parameterization.
They accept a side count, numeric or picked base radius, signed height,
arbitrary `Axis=`, and optional exact planar caps through `Solid=Yes`.
`Tube` creates a closed exact B-rep with concentric rational cylinder walls and
annular planar caps. The two radii may be entered in either order;
`WallThickness=` expands outward from the first radius, and `BothSides=Yes`
centers the full doubled height on the starting point.
`Torus` creates an exact closed 9-by-9 rational quadratic surface from a center,
major radius (or point on the major circle), and minor radius. `Axis=` orients
the torus; the major radius must exceed the positive minor radius. Its U and V
domains are the major- and minor-circle circumferences, matching OpenNURBS.
`ExtrudeCrv` creates exact rational NURBS surfaces from selected analytic,
polyline, and NURBS curves. A numeric distance uses World Z; two points define
an arbitrary direction; entering it without either starts the two-pick viewport
workflow. `BothSides=Yes` sweeps equally in both directions and
`DeleteInput=Yes` removes the profiles. Matching Rhino, outputs are unselected,
ungrouped objects with fresh attributes on the current layer. `Solid=Yes`
turns each closed planar profile into an exact closed B-rep with a ruled wall,
two planar trimmed caps, shared rational rim curves, and one shared wall seam;
open and nonplanar profiles remain exact NURBS surfaces.
`ExtrudeCrvToPoint` creates exact rational NURBS surfaces that taper selected
curves to one apex. Its U direction runs from the profile to the apex while V
preserves the source curve's degree, knots, and weights, matching Rhino's NURBS
form. Enter it without an apex to pick one in the viewport. Inputs are retained
by default; `DeleteInput=Yes` removes them. `Solid=Yes` turns each closed planar
profile into an exact closed B-rep with a singular-apex ruled wall, planar
trimmed cap, shared rational rim, and one twice-used wall seam; open and
nonplanar profiles remain surfaces. `Output=Surface` and `Solid=No` are accepted
explicitly; SubD output is not yet represented.
`ExtrudeCrvAlongCrv` creates an exact fixed-orientation sum surface from each
selected profile and a curve path. Name the path with `PathName=`, or select it
last; the toolbar uses the last-selected convention. The path is retained and
deselected. `DeleteInput=Yes` removes only successfully extruded profiles.
Profile degree, knots, and weights become U; path data becomes V; tensor weights
are multiplied exactly. With an open path whose endpoints leave the profile
plane, `Solid=Yes` turns each closed planar profile into an exact capped B-rep
with translated planar trims, shared rational rims, and an exact copy of the
path as the twice-used wall seam. Open and nonplanar profiles, or profiles swept
along a closed path, remain surfaces. `Output=Surface` and `Solid=No` are
accepted explicitly.
`Revolve` creates exact rational NURBS surfaces around an arbitrary two-point
axis. Supply a signed sweep from -360 through 360 degrees, or use
`FullCircle=Yes`; `StartAngle=` rotates the beginning of a partial sweep.
Entering `Revolve` without an axis starts the two-pick viewport workflow, which
defaults to a full turn and also accepts `Angle=`, `StartAngle=`, and
`DeleteInput=`. The exact surface keeps the profile in V and uses fully
multiple quadratic quadrant knots in U. `Output=Surface` and `Deformable=No`
are supported; deformable and SubD revolves are not yet represented.
`ArrayPolar 6` picks a
center for a 360-degree top-view array; an optional angle and `Rotate`/`ZOffset`
settings can follow the count. `Array 3 2` picks two top-view corners for a
rectangular array. A third count adds Z levels and requires `ZDistance=...`;
`Mode=Fill` treats the picked rectangle as the outside array span. `Join`
connects unambiguous line/polyline endpoint chains within the document
tolerance. `Explode` turns polylines into line segments, frees point-cloud
members as points, duplicates polysurface faces as exact trimmed B-reps, and
splits meshes at disconnected or unwelded edges. Parts are emitted in Rhino's
reverse component order, inherit attributes, and replace their source in every
existing group. `Length` measures
analytic, polyline, and NURBS curves with controlled accuracy; `Area` measures
circles, ellipses, closed planar polylines, exact NURBS surfaces, B-reps, and
meshes. Full-domain NURBS faces are integrated per knot-span rectangle, while
planar trimmed B-rep faces use their exact boundary integrals, including inner
holes; general nonplanar trims remain explicit errors.
`Volume` reports the accumulated signed volume of selected closed triangle
meshes and exact B-rep solids; outward orientation is positive and reversed
orientation is negative. Meshes use translation-stable tetrahedral
accumulation. Full-domain NURBS B-rep faces are integrated directly over each
knot-span rectangle with adaptive quadrature, without measuring a display
tessellation. Planar trimmed caps use an exact-edge boundary-area integral;
general nonplanar trims are rejected until constrained trimmed-domain mass
properties are implemented. Measurement does not alter history.
`Divide` creates equal arc-length points on selected curves by segment count or
requested segment length; add `MarkEnds` to include open-curve endpoints.
`CrvStart` and `CrvEnd` place attribute-preserving point objects at the natural
ends of every selected curve. `CloseCrv` currently closes selected polylines:
nearby ends are made identical, while wider gaps gain a straight segment unless
`CloseWideGapsWithLine=No` is set. Lines that cannot form a valid three-segment
loop remain unchanged. Open arcs and NURBS curves will be enabled once the
document has an exact polycurve representation.
`CrvSeam point` relocates exactly one selected closed curve's start/end seam to
the closest curve location; omit the point to pick that location in a viewport,
or use `CrvSeam Parameter=value` for an exact parameter. Supported analytic
curves, closed polylines, and NURBS curves are converted to their exact NURBS
representation as needed. Shape and parameter-span length are preserved, and
the output domain starts at the chosen parameter. Smooth periodic seams remain
periodic and gain a control point when required, while an existing
multiple-knot seam becomes Rhino's equivalent clamped form. Rational geometry,
object identity, attributes, groups, selection, and undo are preserved.
`SrfSeam point Direction=U|V|Both` performs the corresponding exact edit on
one selected untrimmed NURBS surface; omit the point for a one-click viewport
pick. `Parameter=value` targets one axis directly, while `Parameter=u,v` also
supports `Direction=Both`. Without `Direction`, the only closed axis is chosen,
or U is used when both axes close. The implementation follows OpenNURBS by
flattening the stored homogeneous control net, so rational and periodic tensor
structure, parameter-span lengths, surface orientation, identity, attributes,
groups, selection, and undo are preserved. A projectively coincident boundary
whose stored homogeneous seam controls do not close is rejected as Rhino does.
`Reparameterize start end` changes one selected curve's domain, while four
values set the U and V domains of one selected untrimmed NURBS surface. Commas
between values are also accepted. `Reparameterize Automatic` uses curve length
or OpenNURBS' longest-control-polygon surface size. Analytic curves are changed
to exact NURBS form so the new domain can be retained; geometry, orientation,
periodicity, object identity, attributes, groups, selection, and undo are
preserved.
`Extend Length=value Side=Start|End|Both` extends one selected open curve;
`Both` adds the full requested model-space length independently at each end.
`Type=Natural` selects the source end's natural line, same-radius arc, or smooth
continuation, while `Type=Smooth` explicitly requests curvature-continuous
free-form extrapolation. `Type=Arc` follows the endpoint's exact osculating
circle and falls back to a degree-matched tangent line at zero curvature;
requests beyond one revolution use Rhino's full-circle cap while retaining the
requested parameter-domain interval. `Type=Line` uses Rhino's straight tangent extension.
With `Join=Merge`, line and polyline ends merge into their terminal span,
higher-degree straight curves collapse to one line, and other curves retain
Rhino's exact degree-matched span, knot, and rational-weight rules. A merged
same-radius arc is rebuilt as one canonical rational arc. `Join=Yes` retains an
explicit segment boundary, including Rhino's unit-span polyline
parameterization and exact full-multiplicity free-form seam. `Join=No` leaves
the source untouched and creates one or two native line, arc, or smooth
extension curves with copied attributes but without copying source group
membership. Rhino's post-command deselection and undo behavior are retained.
`Extend point Type=Natural|Arc|Line|Smooth Join=Merge|Yes|No` extends one of
the selected open curves to the nearest intersection with the other selected
curve, surface, or trimmed B-rep boundaries. The point chooses the closest
source endpoint; in the UI, omit it and pick that endpoint in a viewport after
preselecting the source and boundaries. Exact span subdivision handles
transverse and tangent curve-surface hits, while finite coplanar overlaps stop
at their entry edge and B-rep trim holes are excluded. Intersections already
on the source are ignored, multiple boundaries resolve to the first forward
hit, and the source identity, attributes, groups, undo state, and Rhino's
separate-output behavior are preserved.
`Extend Domain=start,end` exposes exact analytic NURBS-domain extension.
Analytic curves and polylines are promoted to exact NURBS form as needed while
identity is preserved. The permanent length-command fixture covers all join
modes for explicit Smooth; its `2e-8` threshold contains Rhino's approximately
`1.2e-8` cubic length-solver variation while Viboceros resolves that case to
machine precision. The boundary-command fixture covers every continuation
style, all join modes, both-end extension, nearest-boundary selection,
standalone surfaces, solid and trimmed B-reps, curved and tangent surface hits,
coplanar overlap, and trim holes to `1e-10`. Arc cases compare a canonical
knot parameterization because Rhino leaks its temporary boundary-search extent
into geometrically equivalent segment domains. Arc geometry and commands
otherwise agree to floating-point roundoff except for the same pre-existing
Smooth length-solver variation.
`ExtendSrf Edge=West|South|East|North Distance=value` performs Rhino's
physical-distance operation on one selected untrimmed NURBS surface. Positive
distances use Rhino's homogeneous-control-curve RMS scaling, including
rational weights and non-clamped ends. Negative distances trim inward by true
arc length along an on-surface isocurve, use the selected edge's parameter
midpoint by default, and restore the original surface domain; `At=value`
chooses the edge parameter explicitly. A model point in place of `Edge=` picks
the closest non-singular open natural boundary and supplies that path parameter
for shrinking. In the UI, enter `ExtendSrf Distance=value` with any type and
merge options to pick this boundary in a viewport. `Direction=U|V
Domain=start,end` exposes the corresponding analytic-domain extension.
`Type=Smooth` analytically extrapolates the edge span, while `Type=Line` joins
an exact degree-matched straight tangent span. `Merge=No` retains the source
and creates the extension as a separate patch with matching attributes and
group membership. Tensor structure, identity, attributes, groups, selection,
and undo are preserved.
`SubCrv start_point end_point` replaces one selected curve with the exact
directed portion between the closest curve locations; omit both points for two
viewport picks, or use `Parameter=start,end` for exact parameters. Reversing
the point order reverses an open result and crosses the existing seam on a
closed curve, matching Rhino/OpenNURBS trim behavior. `Copy=Yes` retains the
source and adds an attribute-preserving result to its groups. Lines, analytic
curves, polylines, and NURBS curves are converted to exact rational NURBS form
as needed; replacement preserves identity, attributes, groups, selection, and
undo.
`Split point [point ...]` divides one selected curve at the closest curve
locations; omit the points to collect viewport picks and press Enter, or use
`Split Parameter=value[,value...]` for exact parameters. Open-curve pieces
remain in natural order. Closed curves produce cyclic pieces between the split
locations, including Rhino's clamped full-loop result for a single location.
Lines, analytic curves, polylines, and NURBS curves are supported. The first
piece retains the source identity; every piece retains its attributes and group
membership, becomes selected, and participates in one undo step. This is the
curve `Point` path; cutting-object and surface splitting remain future
extensions.
`Intersect` compares every supported pair of selected curve-compatible objects,
untrimmed NURBS surfaces, and B-reps. Isolated and tangent contacts create
current-layer point objects, while finite shared intervals create exact NURBS
subcurves. Curve/B-rep contacts are clipped against exact face trim regions and
deduplicated across shared edges and vertices. Transverse planar surface pairs
produce exact, arc-length-parameterized lines clipped to both finite patches;
selection order determines their orientation as in Rhino. Coincident
nonsingular convex non-rational bilinear patches produce an exact shared edge
or closed overlap perimeter, including Rhino's distinct edge orientation and
loop-domain rules.
Curve/curve overlaps use the later curve's orientation and parameterization,
matching Rhino. Pairwise duplicates are intentionally retained when three or
more source objects meet at one location. Inputs remain in the document and are
deselected, outputs are selected, and all output creation is one undo step. A
no-hit run still clears the input selection but creates no undo record.
Non-planar and more general coincident surface/surface intersections,
surface/B-rep, and B-rep/B-rep intersection curves remain future extensions.
`Trim point` treats the selected curve nearest the point as the target and all
other selected curves as cutters; omit the point in the UI to pick the interval
to remove in a viewport. Only the nearest cutting intersection on either side
of the pick bounds the removed interval, so unused intersections do not split
the retained geometry. `ApparentIntersections=Yes` is the Rhino-compatible
default and projects orthogonally along world Z unless `ViewNormal=x,y,z` is
supplied; the UI passes the active view direction (an orthogonal approximation
in the perspective viewport). Use `ApparentIntersections=No` for actual 3D
curve intersections. End trims and closed-curve trims retain the source
identity. Removing a middle interval creates two new exact NURBS objects and
deletes the source, matching Rhino; both pieces inherit its attributes and
groups. Results replace the cutter selection and the complete edit is one undo
step. Curve cutters are implemented; surface, B-rep, and mesh cutters remain
future extensions.
`ExtractPt` duplicates curve controls, surface control nets, and every raw mesh
vertex (including unused and coincident vertices). Closed seams and periodic
NURBS control rings follow Rhino ordering. Point results default to each input
layer; `OutputLayer=Current` puts them on the active layer with fresh
attributes. `Output=PointCloud` merges the locations into one cloud in selection
order, and selects the result while deselecting the sources. Input-layer output
copies the first selected source's attributes when that source contributes
locations; otherwise it uses fresh current-layer attributes.
`Hide` and `Lock` change selected objects; `Show` and `Unlock` restore every
object with the corresponding object-level state. Hidden objects neither render
nor snap. Locked objects render in gray and remain available to osnap, but
cannot be selected or edited. Layer visibility and locking remain independent.
`HideSwap` exchanges normal and hidden object modes, while `LockSwap` exchanges
normal and locked modes. Like Rhino, both swaps affect only objects on visible,
unlocked layers and leave the third object mode unchanged.
`Isolate` hides ordinary objects outside the selection and `IsolateLock` locks
them; objects already hidden or locked and objects on hidden or locked layers
are unchanged. Their `Unisolate` counterparts restore only modes introduced by
the matching isolate command, with provenance preserved through undo and redo.
Rhino-compatible curve, line, polyline, point, point-cloud, surface, and
polysurface, and open/closed mesh filters add visible, unlocked objects of the
requested type to the current selection. `SelPtCloud` is separate from `SelPt`,
matching Rhino. `SelSrf` includes both untrimmed NURBS surfaces and single-face
trimmed B-reps while excluding multi-face B-reps. `SelPolysrf` (alias
`SelPolysurface`) and its open/closed variants classify only multi-face B-reps
by shared-edge topology.
`SelPlanarCrv` uses document tolerance. `SelLine` also recognizes
exactly straight, single-span higher-degree NURBS curves, while excluding
multi-span curves and polylines as Rhino does. `SelPolyline` includes native
polylines and multi-segment degree-one NURBS curves, but excludes line objects
and two-control-point degree-one NURBS curves. `SelShortCrv` takes an explicit
positive maximum length and includes curves exactly on that boundary; it uses
the same controlled length calculation as `Length`. Mesh closure uses exact
location-welded polygon-edge topology, so quad meshes, indexed triangle meshes,
and STL-style triangle soup classify consistently; quad diagonals are used only
when an operation explicitly needs triangles.
`SelLast` selects every object changed by the latest object-editing transaction,
including multi-object imports and command outputs. `SelPrev` swaps the current
and previous selection sets. Both replace by default, matching Rhino; set
`DeselectOthersBeforeSelect=No` to add instead.
`SelName` and `SelLayer` add case-insensitive `*`/`?` wildcard matches without
expanding overlapping groups; `SelName ""` selects unnamed objects. `SelGroup`
uses Rhino's exact, case-sensitive group names. Matching hidden or locked layers
with `SelLayer` makes those layers visible and unlocked outside undo history,
while object-level hidden and locked states remain untouched.
`SelColor r,g,b` adds visible, unlocked, ungrouped objects with that resolved
display color; as in Rhino, objects contained in groups are skipped. ByLayer
objects use their layer color; material- and parent-sourced objects currently
use the same documented fallback as the viewport.
`SelDup` adds every visible, unlocked geometric copy except one deterministic
document-order original per class; `SelDupAll` includes those originals.
Equality is independent of object properties, groups, and document tolerance,
and direction-independent for lines, open polylines, analytic circles and
arcs, and compatible NURBS curves. Points and indexed meshes compare exact
stored values; curves use Rhino/OpenNURBS' scale-aware fixed zero policy.
Closed piecewise-linear and NURBS seams remain significant, while mesh vertex
indexing, face order, and winding must match.
`SetObjectName` assigns one shared name to the selection. Add
`AppendCounter=Yes` for Rhino's zero-based suffixes in document order, or use
`SetObjectName ""` to clear names. Unnamed `Group` calls receive Rhino-style
`Group01`, `Group02`, ... names; explicit group names are case-sensitive.
`SetObjectColor r,g,b` assigns Rhino-style per-object display color to the
selection; `SetObjectColor ByLayer` restores layer-driven display while
retaining the stored object color. Selection and locked-object colors still
take visual precedence. Material- and parent-sourced colors are preserved in
3DM files and currently fall back to layer color until materials and instance
definitions are implemented.
`ChangeLayer` moves selected objects without changing their identities, groups,
or the current layer. `CopyToLayer` skips selected objects already on the target,
copies each remaining group subset into a fresh automatic group, and leaves the
original selection unchanged. Hidden and locked target layers are supported.
`Orient` maps two reference points to two target points with Rhino's shortest
3D rotation. `Scale=No` preserves size, `Scale=1D` changes only the reference
axis, and `Scale=3D` scales uniformly. `Orient3Pt` maps right-handed frames
defined by three reference and target points; its `Scale=Yes` factor comes from
the first point pair. Both commands default to in-place, unscaled transforms;
`Copy=Yes` preserves the original selection, attributes, and group topology.
`OrientOnSrf` uses a source base/reference pair and a point nearest the target
NURBS surface. Name that surface with `SurfaceName=`, or make it the last
selected object. `Rigid=Yes` (the default) maps a frame without deformation;
`Rigid=No` applies Rhino's plane-to-surface morph; lines and polylines become
cubic deformable curves, analytic and NURBS curves receive tolerance-driven
adaptive cubic fits, and meshes map per vertex. `Scale=` must be positive,
`Rotation=` is in degrees, `Flip=Yes` reverses surface Y and Z, and
`SourceNormal=` defaults to world Z. In deformable mode, `ConstrainNormal=Yes`
keeps normal offsets parallel to `SourceNormal`, the command-line stand-in for
Rhino's placement-viewport construction-plane normal.
This command defaults to `Copy=Yes`; originals remain selected and copied group
topology is preserved. Use `Copy=No` for an identity-preserving in-place morph.
`Array` takes X/Y/Z counts followed by signed world-axis distances. Its default
`UnitCell` mode uses those values as successive spacing. `Mode=Fill` treats them
as outside dimensions and accounts for the selected geometry's bounding-box
extent, matching Rhino's fit-within-span behavior. Counts of one create no
copies on that axis.
`ArrayLinear` takes Rhino's total item count followed by two reference points;
their vector is the spacing between successive items. It retains the originals
as the selection, preserves object attributes, and recreates every selected
group topology independently for each copy as one bounded, atomic undo step.
`ArrayCrv` places the selected sources at equal arc-length positions on a line,
analytic curve, polyline, or NURBS rail. Name a unique rail with the single-token
`PathName=` option, or omit it and make the rail the last selected object.
An item count includes both endpoints of an open rail and omits a duplicate
closed seam; `Distance=` uses a fixed spacing and leaves any shorter remainder.
`Freeform` is the default rotation-minimizing orientation. `Roadlike` keeps its
Y axis parallel to the world XY construction plane, `Stairlike` applies yaw
only, and `NoRotation` translates without rotating. `BasePoint=` uses an
explicit source anchor and, as in Rhino, retains the originals in addition to
all requested rail items. Without it, the originals are the first item. Use
`Flip` on the rail to reverse its travel direction.
`ArraySrf` copies every selected source into a U-by-V grid over an untrimmed
NURBS surface. `Mode=UV` divides normalized parameters; `Mode=Isocurve` divides
the U and V domain-start isocurves by arc length before forming the grid. Counts
of one use the corresponding domain start. `BasePoint=` is required, `Up=`
defaults to world Z, and `SurfaceName=` can be omitted when the target is the
last selected object. Surface normals determine orientation, while originals,
attributes, selection, and one copied group topology per cell are preserved.
`ArrayPolar` uses the same total-item and preservation rules around a top-view
center. Exactly 360 degrees omits a duplicate endpoint; other positive,
negative, and multi-turn sweeps include both endpoints. `Rotate=No` keeps object
orientation by orbiting the combined selection-bounds center, while `ZOffset`
adds a cumulative height per item.
`UnifyMeshNormals` repairs inconsistent face winding across exact
location-welded manifold edges, including STL-style triangle soup, and rejects
non-orientable constraints atomically. `Dir UReverse|VReverse|SwapUV` reverses
either parameter direction or transposes selected untrimmed NURBS surfaces
exactly; `Mode=FlipU|FlipV|SwapUV` is also accepted. Rational weights,
non-clamped domains, periodic directions, identities, attributes, groups,
selection, and undo are preserved. `Dir Flip` reverses the same selected curve
and mesh types as `Flip`; standalone surface-normal orientation is not yet a
separate property in the native surface model. `Flip` (aliases `Reverse` and `Rev`)
reverses selected curve directions or every face in selected meshes without
changing object identities, attributes, groups, or closed-curve seams.
`Weld` merges exactly coincident endpoints only where mesh faces share a whole
edge and their normal angle is within the supplied 0-to-180-degree tolerance.
It matches Rhino's survivor ordering, compacts unused vertices, never merges a
vertex-only contact, and preserves object identity, metadata, groups, and
selection. The default is 180 degrees; `Angle=90` is also accepted.
`WeldEdge` merges the raw endpoint sets incident to selected exact-location
mesh topology edges. Use `Edges=0,2`, `Edges=All`, or omit the selector to pick
one edge in the viewport. The earliest source vertex survives; unrelated
coincident fan components remain separate, including at half-welded and
non-manifold seams. Naked or already-welded selections still perform
Rhino-compatible unused-vertex compaction while preserving the object.
`WeldVertices` welds every joined seam incident to selected topology vertices,
including both endpoints of each seam. Use `Vertices=0,2`, `Vertices=All`, or
omit the selector for one viewport pick. It preserves Rhino's later-source
vertex ordering, ignores coincident vertex-only contacts, limits non-manifold
edges to their first two face uses, and performs unused-vertex compaction.
`Unweld` performs the inverse topology edit at edges whose face-normal angle is
greater than or equal to its tolerance. It preserves smoother face regions,
rebuilds affected vertices in OpenNURBS radial order, and compacts unused
vertices. The zero-degree default separates every adjacent face region;
`ModifyNormals=Yes|No` is accepted for Rhino script compatibility while mesh
normals remain derived data.
`UnweldEdge` adds seams at selected exact-location mesh topology edges. Use
`Edges=0,2`, `Edges=All`, or omit the selector to pick one edge in the viewport.
It partitions closed and high-valence radial face fans, handles non-manifold
edges, and preserves existing seams; naked or already-unwelded selections only
trigger Rhino-compatible vertex compaction. `ModifyNormals=Yes|No` is accepted,
but normals remain derived data.
`UnweldVertex` gives every face incident to each selected topology vertex its
own raw mesh vertex. Use `Vertices=0,2`, `Vertices=All`, or omit the selector
for one viewport pick. Closed, high-valence, already-unwelded, and non-manifold
fans follow Rhino's radial ordering, and unused raw vertices are compacted.
`ModifyNormals=Yes|No` is accepted while normals remain derived data.
`CombineIdenticalMeshVertices` turns exactly coincident raw vertices into shared
indices without culling unused vertices, preserving object identity, metadata,
face order, and winding.
`CullUnusedMeshVertices` removes raw vertices not referenced by any face while
preserving the source order and identity of every referenced vertex, including
coincident vertices, as well as object metadata, face order, and winding.
`SplitDisjointMesh` separates exact-location edge-connected components (a lone
shared vertex does not connect them), retaining the first object identity and
copying attributes and group membership to additional pieces.
`ExtractDuplicateMeshFaces` separates all but one face from each duplicate
class. Face equality uses exact vertex locations and ignores raw indices,
cyclic ordering, and winding; attributes and group membership are preserved.
`ExtractNonManifoldMeshEdges` removes faces around edges shared by three or
more faces into attribute- and group-preserving mesh objects. Options can limit
the operation to hanging faces or raise the minimum incident-face count.
Osnap captures visible Point, End, Mid, Center, and Quad features, including
indexed members of point clouds and features on locked objects and layers;
SmartTrack captures horizontal and vertical alignment from the first picked
point. Grid Snap rounds construction-plane picks to the unit grid. Right-drag
pans parallel views and rotates the Perspective view; Shift-right-drag pans the
Perspective view, middle-drag pans any view, and the mouse wheel zooms. A plain
right-click acts as Enter. Outside a drafting command, left-drag from left to
right selects only fully enclosed objects, while right-to-left makes a crossing
selection. Click geometry to replace the selection, Shift-click/drag to add,
and Ctrl-click/drag or Command-click/drag to remove. Click empty space or press
Esc to clear the selection; press Delete to remove selected objects.

Commands are case-insensitive. Typing while another non-text UI element or a
viewport is active moves the text to the command line automatically. Matching
command names appear below the input; press Tab or click a match to complete it.

## Build and run

Install a current stable Rust toolchain (Rust 1.95 or newer), CMake, and a C++17
compiler. Initialize the pinned OpenNURBS submodule after cloning, then run:

```sh
git submodule update --init --recursive
cargo run
```

On Linux, wgpu uses Vulkan when available and the window supports both Wayland
and X11. Run all workspace tests and checks with:

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

## Rhino compatibility oracle

The versioned Python oracle API runs identical JSON geometry and document-state
batches in a native release build of Viboceros and Rhino 8, recursively checks
results, and reports per-operation timings. With Rhino installed through the
configured Wine/FEX launcher, run the core fixture with:

```sh
python3 -m tools.rhino_oracle compare \
  tools/rhino_oracle/fixtures/core.json \
  --absolute-epsilon 2e-12 --relative-epsilon 1e-12
```

To keep Wine/Rhino completely off the active desktop, use the isolated Xvfb
runner (requires `Xvfb`, `xvfb-run`, and `i3`):

```sh
tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/parabola_three_point.json \
  --absolute-epsilon 2e-12 --relative-epsilon 1e-12

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/helix.json \
  --absolute-epsilon 2e-5 --relative-epsilon 1e-12

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/spiral.json \
  --absolute-epsilon 3e-5 --relative-epsilon 1e-12

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/swept_spiral.json \
  --absolute-epsilon 2e-7 --relative-epsilon 1e-12

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/catenary.json \
  --absolute-epsilon 2e-8 --relative-epsilon 1e-12

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_through.json \
  --absolute-epsilon 2e-12 --relative-epsilon 1e-12

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_tween.json \
  --absolute-epsilon 5e-8 --relative-epsilon 1e-12

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_tween_short_samples.json \
  --absolute-epsilon 2e-12 --relative-epsilon 1e-12

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_fit.json \
  --absolute-epsilon 2e-12 --relative-epsilon 1e-12

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_rebuild.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_make_uniform.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/surface_make_uniform.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/make_uniform_commands.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_insert_knot.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/surface_insert_knot.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/insert_control_point.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_change_seam.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/surface_change_seam.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/reparameterize.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/reparameterize_automatic.json \
  --relative-epsilon 1e-8

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/surface_extend.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/surface_extend_length.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/surface_extend_line.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_extend_line.json \
  --absolute-epsilon 5e-14 --relative-epsilon 1e-12

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_extend_command.json \
  --absolute-epsilon 2e-8 --relative-epsilon 1e-11

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_extend_arc.json \
  --absolute-epsilon 2e-11 --relative-epsilon 2e-12

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_extend_arc_command.json \
  --absolute-epsilon 2e-8 --relative-epsilon 1e-11

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_extend_boundary_command.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/surface_shrink.json \
  --absolute-epsilon 3e-8 --relative-epsilon 1e-10

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_subcurve.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_split.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_intersect_command.json \
  --absolute-epsilon 1e-9 --relative-epsilon 1e-11

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_surface_intersect_command.json \
  --absolute-epsilon 1e-9 --relative-epsilon 1e-11

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_brep_intersect_command.json \
  --absolute-epsilon 1e-9 --relative-epsilon 1e-11

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/surface_surface_intersect_command.json \
  --absolute-epsilon 1e-9 --relative-epsilon 1e-11

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_trim_command.json \
  --absolute-epsilon 1e-9 --relative-epsilon 1e-11

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/surface_direction_edit.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/remove_knot.json \
  --absolute-epsilon 2e-11 --relative-epsilon 2e-12

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/remove_control_point.json \
  --absolute-epsilon 1e-10 --relative-epsilon 1e-10

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/remove_multi_knot.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/make_non_periodic.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_make_periodic.json \
  --absolute-epsilon 2e-11 --relative-epsilon 2e-12

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_make_periodic_degrees.json \
  --absolute-epsilon 2e-11 --relative-epsilon 2e-12

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/surface_make_periodic.json \
  --absolute-epsilon 2e-11 --relative-epsilon 2e-12

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_change_degree.json \
  --absolute-epsilon 2e-11 --relative-epsilon 2e-12

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/surface_change_degree.json \
  --absolute-epsilon 2e-11 --relative-epsilon 2e-12
```

The same workflow is importable for instrumentation and tests:

```python
from tools.rhino_oracle import OracleClient, load_request

report = OracleClient().compare(load_request("tools/rhino_oracle/fixtures/core.json"))
assert report.passed
```

Set `VIBOCEROS_RHINO_LAUNCHER` to use another launcher. McNeel's documented
`/runscript` startup interface is tried first; this project's Wine path also
has an owned-window fallback that requires `wmctrl` and `xdotool` and never
targets a pre-existing Rhino process. Set `VIBOCEROS_RHINO_UI_FALLBACK=0` to
disable it. The `viboceros` and `rhino` modes run either side independently.
Timings cover repeated API calls after one warm-up and exclude process startup,
fixture construction, and JSON I/O; Rhino timings include its public
Python/RhinoCommon bridge.

Both ASCII and binary STL are supported. 3DM import/export uses McNeel's
OpenNURBS toolkit and preserves points, point-cloud locations, lines, NURBS
curves, untrimmed NURBS surfaces, mixed triangle/quad meshes, and editable
rational NURBS B-reps. Mesh faces retain their arity in 3DM round trips. B-rep
interchange retains shared vertices and edges, exact edge and
parameter-space trim curves, face surfaces and orientation, outer and inner
loops, boundary/mated/seam/singular trims, and modelling tolerances. Layer and
object state are also preserved, including the raw RGB display color, its
layer/object/material/parent source, and surface wire density. Named group
definitions and ordered membership survive round trips, including overlapping
and empty groups.
Circles, arcs, ellipses, and polylines are exported without approximation as
rational NURBS curves; canonical degree-one curves return as editable
polylines. Unsupported object types and specialized B-rep trim forms are
counted and reported during import.

Initial STEP interchange uses the Apache-2.0 Monstertruck kernel to read
solid/shell B-reps and assemblies, apply instance transforms, and robustly
tessellate exact trimmed surfaces into validated display meshes. Parser,
topology, and unsupported-representation losses are reported instead of being
silent. STL and STEP export tessellate visible NURBS surfaces plus full-domain
and trimmed planar B-rep faces; exact outer and inner p-curves are sampled into
a constrained triangulation so holes remain open. STEP writes the results as
faceted shells with shared topology and planar faces. Nonplanar general trims
remain explicit errors rather than being silently filled. Editable STEP B-rep
interchange and production surface and solid modelling are not implemented yet.
