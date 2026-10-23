# Meshing and mesh primitives

[Polygon meshes](meshes.md) · [Meshes and SubDs](meshes-index.md)

`Mesh` creates editable polygon meshes from selected NURBS surfaces and B-reps
while retaining the selected sources. The new meshes copy source names, layers,
colors, and display attributes, remain unselected, and do not inherit source
groups, matching Rhino's derived-object behavior. `Density=0..1` selects a
bounded per-knot-span sampling level; `SimplePlanes=Yes` minimizes entirely
planar inputs, and `JaggedSeams=Yes` disables shared-edge snapping on B-reps.
Regular surface cells remain quadrilaterals, singular sides and planar trim
regions use triangles. [Smooth-seam boundary auditing](../brep-meshing.md) checks
open shells as well as closed solids; incompatible shared grids trigger a
conforming triangle fallback without capping intended openings.
General nonplanar trims are constrained-triangulated in parameter space with
interior knot-span grid samples; outer and inner boundaries remain exact mesh
constraints instead of being silently filled.

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

[`MeshToNURB`](mesh-to-nurb.md) converts selected triangle/quad meshes to exact
NURBS faces, splitting edge-disconnected components into separate unselected
B-reps. Outputs retain source attributes but not group memberships. Other
selected geometry is ignored, and triangle-trimming choices survive undo.
See its reference for selection paths, option memory, budgets, and n-gon limits.
