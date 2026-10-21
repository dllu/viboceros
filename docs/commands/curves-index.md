# Points, curves and control points

[All commands](README.md) · [Command input](../command-line.md)

- [Divide by count, arc length and chord length; Split and output groups](divide.md)
- [Ellipse construction, AroundCurve, deformable output and focus markers](ellipse.md)
- [MarkFoci and conic axes](mark-foci.md)
- [Deformable Circle, construction modes and FitPoints](circle-deformable.md)
- [Circle AroundCurve, size getters and guide picking](circle-around-curve.md)
- [Circle Tangent, picked contacts, radii and free points](circle-tangent.md)
- [Arc Tangent, picked contacts and final arc choice](arc-tangent.md)
- [Arc Tilted, spatial planes, endpoint dragging and length measurements](arc-tilted.md)
- [Arc Deformable, editable NURBS output and degree/count settings](arc-deformable.md)
- [Arc Extension, curve/edge continuation and constrained centers](arc-extension.md)
- [Conic curves, endpoint constraints and temporary portions](conic.md)
- [Rectangle construction modes and numeric dimensions](rectangle.md)
- [Curve creation and editing](curves.md)
- [Simplify lines, arcs and rational curve structures](simplify-crv.md)
- [Change arc and circle sizes](modify-radius.md)
- [Standalone subcurves and numeric confirmation](subcurve.md)
- [Delete portions of curves](delete-subcurve.md)
- [Analytic curve offsets](offset.md)
- [Two-curve fillets](fillet.md)
- [Two-curve chamfers](chamfer.md)
- [Connect two curves](connect.md)
- [Blend two curves](blend.md)
- [Match curve ends](match.md)
- [Polyline corner fillets](fillet-corners.md)
- [Single and repeated point placement](points.md)
- [Rectangular point grids](point-grid.md)
- [Point-cloud creation](point-cloud.md)
- [NURBS structure and parameterization](nurbs.md)
- [InsertKnot repeated input, Automatic refinement and staged Undo](insert-knot.md)
- [InsertKink on curves, surfaces and temporary portions](insert-kink.md)
- [Rebuild curve previews](rebuild-curves.md)
- [Smooth controls and mesh vertices](smooth.md)
- [EditPtOn and curve edit-point transforms](edit-points.md)
- [Grip visibility and selection: HidePt, ShowPt, directional rows, SelPt and InvertPt](point-selection.md)
- [Delete selected grips and objects with Undo/Redo](delete-grips.md)
- [Weight: rational curve and surface controls with a live preview](weight.md)
- [InsertEditPoint: shape-preserving edit-point insertion and SubCrv](insert-edit-point.md)
- [SoftEditCrv: local curve falloff and repeated copies](soft-edit-curve.md)
- [Object conversion to NURBS](to-nurbs.md)
- [Bézier conversion](beziers.md)
- [Single-span surface conversion](single-spans.md)
- [Circle FitPoints input and limitations](../circle-fit-input.md)
- [Control-point display and selection](../control-points.md)
- [Transforming selected control points](../grip-transforms.md)

- [parabolas](parabola.md)

- [hyperbolas](hyperbola.md)

- [Interactive catenaries](catenary.md)

- [Interactive helices and spirals](helix-spiral.md)

- [Reduce polyline vertices with tolerance and Copy options](reduce-polyline.md)

- [RemoveKnot picking, shape previews, cyclic removal and staged Undo](remove-knot.md)

- [RemoveControlPoint viewport grips, composite curves and surface rows](remove-control-point.md)

- [InsertControlPoint repeated picks, shape previews, Midpoint and surface-net extension](insert-control-point.md)

## Examples

These are independent snippets. Prepare the required sources and selection first;
replace object IDs, names and paths with values from your document. See each
command page for supported options and limitations.

```text
Point 1,2,0
PointGrid 0,0,0 6,4,0 XCount=7 YCount=5 ZCount=1
ReducePointCloud Percent=25
Line 0,0,0 10,5,0
Circle 0,0,0 5
Circle 2Point -4,0,0 4,0,0
Circle 3Point 4,0,0 0,4,0 -4,0,0
Circle FitPoints
Arc StartPoint 5,0,0 ThroughPoint 0,5,0 -5,0,0
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
ReducePolyline 0.01 Copy=No
MarkFoci DrawAxis=Yes
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
Conic 0,0,0 10,0,0 5,5,0 0.4
Parabola Vertex 0,0,0 0,0,1 4,0,0 Half=No MarkFocus=Yes
Parabola3Pt -1,0,0.25 1,0,0.25 3,0,2.25 3,0,3.25 Mode=ThroughPoint PickOrder=Default MarkFocus=Yes
Hyperbola 0,0,0 5,0,0 3.75,3,0 BothBranches=Yes MarkFoci=Yes
Helix 0,0,0 0,0,10 2 Turns=3 ReverseTwist=No
Spiral 0,0,0 0,0,6 1 4 Turns=2 ReverseTwist=No
Spiral AroundCurve 1,0,0 2 PathName=Rail Turns=3 PointsPerTurn=12
Catenary 0,0,0 10,0,0 0,0,-1 4 Mode=Parameter PointCount=20
ToNURBS DeleteInputObjects=Yes
Offset 2 0,5,0
Offset 2 BothSides=Yes OutputLayer=Input
Offset 1 1,2,0 Corner=Round
Offset 1 1,2,0 Corner=None
Offset ThroughPoint=2,2,0
OffsetMultiple 1 6,0,0 OffsetCount=3
Divide 8
Divide Length 2.5 MarkEnds
ExtractPt OutputLayer=Input Output=Points
ExtractPt OutputLayer=Input Output=PointCloud
ExtractControlPolygon OutputLayer=Current
ExtractSubCrv Segments=0,2 Copy=Yes Join=No OutputLayer=Input
DeleteFaces Faces=2,0
ConvertToSingleSpans Direction=Both DeleteInput=No
ConvertToBeziers DeleteInput=No
CloseCrv
CloseCrv CloseWideGapsWithLine=No Tolerance=0.01
CrvSeam 4,1,0
SubCrv 8,0,0 2,0,0 Copy=Yes
Extend Length=5 Side=End Type=Natural Join=Merge
Extend Length=2 Side=Both Type=Line Join=Merge
Extend Length=2 Side=Both Type=Smooth Join=Merge
Extend Length=2 Side=End Type=Arc Join=Yes
Extend Length=2 Side=Both Type=Line Join=Yes
Extend Length=2 Side=Both Type=Line Join=No
Extend 5,0 Type=Line Join=Merge
Match Continuity=Tangency PreserveOtherEnd=Position
Match Continuity=Tangency AverageCurves=Yes PreserveOtherEnd=Position
Match Continuity=Position AverageCurves=Yes PreserveOtherEnd=Curvature
Match Continuity=Curvature AverageCurves=Yes PreserveOtherEnd=Position
Match Pick1=0,0,0 Pick2=4,0,0 Continuity=Curvature PreserveOtherEnd=Curvature
MatchCrvDir
MatchCrvDir Reference=Rail
Weld 180
WeldEdge Edges=0,2
WeldVertices Vertices=0,2
Unweld 45 ModifyNormals=Yes
UnweldEdge Edges=0,2 ModifyNormals=Yes
UnweldVertex Vertices=0,2 ModifyNormals=Yes
```
