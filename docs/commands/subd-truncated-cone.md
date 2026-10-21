# SubDTruncatedCone

[Meshes and SubDs](meshes-index.md) · [Command input](../command-line.md)

Create a smooth SubD frustum from a base circle, signed height and top radius:

```text
SubDTruncatedCone AroundFaces=8 VerticalFaces=2 CapFaceStyle=Quad 0,0,0 3 5 1
SubDTruncatedCone Solid=No AroundFaces=8 VerticalFaces=3 0,0,0 3 -5 1
SubDTruncatedCone DirectionConstraint=None 0,0,0 3 3,4,6 Diameter 2
```

Start the bare command to pick these inputs interactively. The base supports
Radius, Diameter, Circumference, Area, 2Point, 3Point, Tangent and FitPoints.
Tangent accepts point or curve contacts; FitPoints accepts point objects and
visible curve grips, including mixed and partially preselected inputs.

Set `DirectionConstraint=Vertical|None` before picking the base. Vertical uses
the base circle's normal; None uses the picked endpoint as the axis. A signed
numeric height follows the accepted normal. Once height is accepted, top size
points project onto its perpendicular plane. Top Radius and Diameter interpret
the projected distance as the selected measurement. Enter accepts the remembered
size; switching Radius/Diameter retains that numeric default.

Top radius zero creates a cone. A collinear or coincident FitPoints base also
completes as an uncapped reverse cone when its top radius is positive, using
control-cage radii even with Solid=Yes. Ordinary typed base
radii must be positive. A larger top radius and equal end radii are supported.

Defaults are three around faces, eight vertical faces, height one, top size one,
solid triangle caps. `CapFaceStyle=Tri|Quad` selects cap fans; odd quad counts
fall back to triangles. Three sides use one triangular cap per end; four sides
use one Quad or a centered Tri fan. Capped dimensions locate the smooth control
cage. Open dimensions locate the ring limit stations, including a zero-radius
apex; the rounded surface between them remains a SubD.

Counts require at least three around and two vertical faces, with a limit of
100,000 control vertices and faces. BothSides, angle heights, AroundCurve,
Circle Orientation and top Circumference/Area are unsupported.

Previews stay outside the document until the final size is accepted. Completion
creates one object on the current layer and one Undo/Redo step. Cancellation
preserves pending Redo; stale document history or tolerance cancels the draft.
Settings are remembered independently from SubDCone and SubDCylinder.

The [qualification record](../subd-truncated-cone-qualification.json) compares
58 native command/getter recipes, including 46 completed constructions, at a
model epsilon of `1e-6`. It checks topology, winding, tags, control points, vertex
limit stations, sampled limit surfaces, source selection and grouped history.
Preliminary diagnostic captures are retained separately. Arbitrary input clocks,
material/texture channels and comparative performance remain unqualified;
sampled facets do not certify a continuous Hausdorff bound.
