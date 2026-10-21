# Surfaces, solids and B-reps

[All commands](README.md) · [Command input](../command-line.md)

- [ShortPath surface geodesics](short-path.md)
- [InterpCrvOnSrf surface interpolation](interp-crv-on-srf.md)
- [DivideAlongCreases](divide-along-creases.md)
- [Planar surface offsets](offset-srf.md)
- [Polysurface edge chamfers](chamfer-edge.md)
- [Curvature-continuous edge blends](blend-edge.md)
- [Blend complete surface edges](blend-srf.md)
- [Rebuild surface control nets](rebuild-surfaces.md)
- [Surfaces and solids](surfaces.md)
- [Fit a rectangular plane through points, clouds and grips](plane-through-points.md)
- [Fit a sphere through points and grips](sphere-fit-points.md)
- [Tapered curve extrusion](extrude-crv-tapered.md)
- [Surface and face extrusion](extrude-srf.md)
- [Surface extrusion along curves](extrude-srf-along-crv.md)
- [Surface extrusion to a point](extrude-srf-to-point.md)
- [Tapered surface extrusion](extrude-srf-tapered.md)
- [Shell solid walls and face openings](shell.md)
- [Intermediate surface construction and limits](tween-surfaces.md)
- [Apply World-XY curves and points onto surfaces](apply-curves.md)
- [Create UV curves from a surface and spatial inputs](create-uv-curves.md)
- [Project curves and points directionally onto surfaces, B-reps and meshes](project.md)
- [Parallel-view silhouettes of surfaces, B-reps and meshes](silhouette.md)
- [Intersect objects with an infinite plane](iplane.md)
- [Pull curves and points toward surfaces, B-reps and meshes](pull.md)
- [BooleanUnion of solids](boolean-union.md)
- [BooleanIntersection of solids and sets](boolean-intersection.md)
- [BooleanDifference with solid cutters](boolean-difference.md)
- [BooleanSplit with multiple cutters](boolean-split.md)
- [Cycle two-object Boolean results](boolean-two-objects.md)
- [Planar surface Union, Difference and Intersection](planar-booleans.md)
- [Extract individual surface faces](extract-srf.md)
- [Circular pipe surfaces and solids](pipe.md)
- [Cap planar surface, B-rep, and mesh holes](cap.md)
- [Shrink underlying trimmed surfaces](shrink-trimmed-surfaces.md)
- [Restore selected surface trim runs or holes](untrim.md)
- [Remove all surface trims](untrim-all.md)
- [Restore exterior surface boundaries](untrim-border.md)
- [Remove interior holes by face or edge](untrim-holes.md)
- [Flip curve, surface, and mesh directions](flip.md)
- [Merge redundant surface/B-rep edges](merge-edges.md)
- [Merge a selected edge or its immediate neighbors](merge-edge.md)
- [Merge all coplanar mesh or B-rep faces](merge-all-coplanar-faces.md)
- [Merge a selected face pair or connected planar region](merge-coplanar-face.md)
- [Split a selected surface/B-rep edge](split-edge.md)
- [Separate joined surface/B-rep edges](unjoin-edge.md)
- [OffsetCrvOnSrf surface-distance offsets](offset-crv-on-srf.md)
- [Splitting curves and surfaces](split.md)
- [Joining curves, meshes, and surfaces; JoinCopy](join.md)
- [Plane sections](section.md)
- [Equally spaced contours](contour.md)
- [FilletEdge: circular solid-edge rounding and component input](fillet-edge.md)
- [FilletSrf: surface picks, trim/split, joining and previews](fillet-srf.md)
- [Reparameterize face domains](reparameterize.md)
- [RailRevolve usage, options and qualification](rail-revolve.md)

- [Sweep cross sections along two rails](sweep2.md)

- [Infinite-plane projection and input](project-infinite-plane.md)

- [Loose projection and pulling through edit points](projection-loose.md)

## Examples

These are independent snippets. Prepare the required sources and selection first;
replace object IDs, names and paths with values from your document. See each
command page for supported options and limitations.

```text
SrfPt 0,0,0 8,0,0 8,5,2 0,5,2
SrfPtGrid DegreeU=1 2 DegreeV=1 3 0,0,0 0,1,1 0,2,0 3,0,0 3,1,2 3,2,0
SrfControlPtGrid Degree=1 2 Degree=1 3 0,0,0 0,1,1 0,2,0 3,0,0 3,1,2 3,2,0
PlanarSrf DeleteInput=No
Cap DeleteInput=No Crease=No Triangles=No
Loft Type=Normal Closed=No
Sweep1 RailName=Rail Parameters=0 RefitRail=No
Sweep2
RailRevolve 0,0,0 0,0,1 ScaleHeight=No SplitAtTangents=Yes
Pipe 1.5 2.0 Cap=Flat ShapeBlending=Global
Pipe 1 WallThickness=0.25 Cap=Flat
EdgeSrf
Box 0,0,0 8,5,0 3
BoundingBox CoordinateSystem=World Cumulative=Yes Output=Solids
DupBorder OutputLayer=Current
DupEdge Edges=2,0 OutputLayer=Input
DupFaceBorder Faces=2,0 OutputLayer=Input
Sphere 0,0,0 5
Ellipsoid 0,0,0 5 3 2
Cylinder 0,0,0 5 10 Solid=Yes
Cone 0,0,0 5 10 Solid=Yes
Paraboloid Vertex 0,0,0 0,0,1 4,0,0 MarkFocus=Yes Solid=Yes
TruncatedCone 0,0,0 5 10 2.5 Solid=Yes
Pyramid 5 0,0,0 5 10 Solid=Yes
TruncatedPyramid 5 0,0,0 5 10 2.5 Solid=Yes
Tube 0,0,0 3 1 10
Torus 0,0,0 5 1.5
ExtrudeCrv 5 BothSides=No DeleteInput=No
ExtrudeCrvToPoint 0,0,10 DeleteInput=No
ExtrudeCrvAlongCrv PathName=Rail DeleteInput=No
Revolve 0,0,0 0,0,1 270 StartAngle=0 DeleteInput=No
Join
Join JoinDisjointMeshes=Yes
JoinCopy
ShrinkTrimmedSrf
ShrinkTrimmedSrfToEdge
MergeAllCoplanarFaces
MergeAllEdges
MergeEdge object-id edge-index [Edge|EdgeA|EdgeB|Both|All]
SplitEdge object-id edge-index parameter [parameter ...]
UnjoinEdge object-id edge-index[,edge-index...] [object-id edge-index[,edge-index...] ...]
Explode
ExtractSrf 2,1,0 Copy=No OutputLayer=Input
ExtractSrf Faces=0,2 Copy=Yes OutputLayer=Current
ExtractIsocurve 2,1,0 Direction=Both
ExtractIsocurve ExtractAll Direction=Both IgnoreTrims=No
ExtractWireframe OutputLayer=Current GroupOutput=No
SrfSeam 5,0,0 Direction=U
Split 4,0,0 7,0,0
Split CuttingObjects=1,0,0
Split Isocurve=4,6,0 Direction=Both Shrink=Yes
Intersect
IntersectSelf
IntersectTwoSets first-id[,id...] second-id[,id...] OutputLayer=FirstSet
IntersectTwoSets Selected second-id
Trim 5,0,0
ExtendSrf Direction=U Domain=-1,2 Type=Smooth Merge=Yes
ExtendSrf Edge=East Distance=3 Type=Smooth Merge=Yes
ExtendSrf Edge=West Distance=2 Type=Line Merge=Yes
ExtendSrf Edge=East Distance=-2 At=0.5
ExtendSrf Edge=East Distance=2 Type=Smooth Merge=No
ExtendSrf Distance=-2 Type=Smooth Merge=Yes
Reparameterize -4 6
Reparameterize Automatic
Reparameterize Face=2 -2,8,10,30
Reparameterize Surface=object-id Face=2 Automatic
Dir SwapUV
Dir Mode=FlipU
Flip
```
