# SubDTorus

[Meshes and SubDs](meshes-index.md) · [Command input](../command-line.md)

Create a smooth periodic SubD torus from a base circle and a tube size:

```text
SubDTorus AroundFaces=8 VerticalFaces=6 0,0,0 3 1
SubDTorus 0,0,0 Diameter 6 Diameter 2
SubDTorus 0,0,0 3 FixInnerDimension=Yes 4
```

Start the bare command to pick inputs interactively. The base supports Radius,
Diameter, Circumference, Area, 2Point, spatial 3Point, Vertical and Orientation.
Tangent uses point or curve contacts; FitPoints accepts point objects and visible
curve grips. AroundCurve uses a perpendicular base plane at the picked guide point.

The second Radius or Diameter sets the tube radius; numeric diameters are halved.
A picked point projects into the accepted base plane and measures its absolute
radial distance from the base circle, regardless of the second size mode.
Ordinary tube radii must be positive and strictly smaller than the base radius.
`FixInnerDimension=Yes` instead treats the base radius as the inner radius and
adds the tube radius to locate the major circle. A bare `FixInnerDimension`
toggles the setting at the second prompt.

Defaults are ten around faces, six vertical faces, and base and tube radii of
one. The two Radius/Diameter modes and remembered sizes are independent. Since
the fresh tube default equals the base default, accept a smaller tube to finish.
Invalid interactive sizes leave the prompt available for correction. At the
second prompt, numeric `0` denotes the original CPlane origin, as in Rhino.

Set counts before picking the base. Each direction requires at least three
faces, with at most 100,000 control vertices and faces. Dimensions locate vertex
limit stations; the surface between stations remains a smooth SubD approximation.
Collinear or coincident FitPoints stage a zero-radius base: ordinary tube input
remains invalid, and a fixed-inner construction ends without adding geometry.

Construction-circle previews stay outside the document. Completion creates one
object on the current layer and one Undo/Redo step. Cancellation preserves pending
Redo; changed history, tolerance or reference sources invalidate drafts.

The [qualification record](../subd-torus-qualification.json) covers 77 captured
recipes, including 58 constructions, at model epsilon `1e-6`. It compares control
points, vertex limit stations, topology, tags, every corresponding density-two
tessellation quad, sources, selection and history. Exploratory captures are
retained separately. Arbitrary option timing, fit seams, material/texture
channels and comparative performance remain unqualified. Facet comparisons do
not certify a continuous Hausdorff bound on the smooth surface.
