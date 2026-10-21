# SubDSphere

`SubDSphere` creates a smooth subdivision sphere with Quad, Triangle or UV
control topology. It defaults to Quad with one subdivision. The requested radius
places the outermost vertex limit stations on the sphere; the smooth surface
approximates an analytic sphere between stations.

```text
SubDSphere 0,0,0 3
SubDSphere Style=Triangle Subdivisions=1 1,2,3 Diameter 6
SubDSphere Style=UV AroundFaces=8 VerticalFaces=4 0,0,0 3
SubDSphere 2Point -3,0,0 3,0,0
SubDSphere 4Point 3,0,0 0,3,0 -3,0,0 0,0,3
SubDSphere Tangent Point 3,0,0 Point 0,3,0 Point -3,0,0
SubDSphere FitPoints
```

Choose the style and counts at the initial center prompt. Quad subdivisions
range from 0 to 6; Triangle from 0 to 5. UV requires at least three faces around
and two vertically. Each style remembers its own counts. Radius and Diameter
start each invocation at 1 and 2 respectively, independently of Circle and
SubDCylinder settings. Construction allows at most 100,000 control vertices
and faces.

The shared sphere getter supports a center and radius or spatial radius point,
Diameter, Circumference, Area, 2Point, 3Point, 4Point and Orientation. Three- and
four-point modes also accept a Radius after their section points. Tangent accepts
point or curve/edge contacts, a fixed radius, late Radius, FromFirstPoint, or
Enter after two contacts. It uses the ordinary Circle contact rule, while Splop
retains its distinct reference-sphere rule.

FitPoints uses selected points and displayed selected control points, including
rational controls. With fewer than three eligible preselected points, select
points or grips and press Enter. Collinear fits finish without output or a
modeling Undo record; preselection is retained and postselection released.
AroundCurve accepts a preselected or picked guide and a center location. Scripts
name the guide with `Curve=id` or `Edge=id,index` before the center and size.

Scripts use world coordinates. Viewport input uses the shared coordinate,
unit, filter and snap tools and latches the first pick's construction plane.
Hover previews show three construction great circles. Construction stays outside
the document until completion adds one unselected SubD on the current layer in
one Undo record. Escape, stale sources/history and changed tolerance cancel the
draft without consuming pending Redo. Existing SubD display, control-point
editing, transforms and native 3DM interchange apply.

The [qualification report](../subd-sphere-qualification.json) compares 54 Rhino
`8.32.26160.13001` recipes: 47 constructions, two successful no-output fits and
five cancellations. It checks 49 command and 54 getter cases against topology,
winding, tags, control points, vertex limit stations, sampled density-two limit
surfaces, source attributes/selection, grips and actual Undo/Redo at `1e-6`.
Physical picks replay acknowledged projected world contacts. Fourteen separate
exploratory recipes retain rejected option guesses and fallback geometry; they
are excluded from supported comparisons.

General fitted seam/axis parity, surface-edge guides, all option clocks,
material/texture channels, native face packing and comparative performance need
further qualification. High-resolution interpolation has not been benchmarked.
Sampled facets do not establish a continuous Hausdorff bound.
