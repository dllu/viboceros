# SubDEllipsoid

[Meshes and SubDs](meshes-index.md) · [Command input](../command-line.md)

Create a smooth SubD ellipsoid from a base ellipse and a third size:

```text
SubDEllipsoid AroundFaces=8 VerticalFaces=4 CapFaceStyle=Tri 0,0,0 5,0,0 0,3,0 2
SubDEllipsoid Diameter -5,0,0 5,0,0 0,3,0 4
SubDEllipsoid Corner -5,-3,0 5,3,0 2
SubDEllipsoid FromFoci -4,0,0 4,0,0 0,3,0
```

Start the bare command to pick inputs interactively. The base supports ordinary
axis picks, Corner, a centered Corner after the center, Diameter and AroundCurve.
Choose Vertical inside Diameter to place the base perpendicular to the CPlane.
A numeric first or second size locks the length; then pick its direction.

The third size measures distance along the accepted base normal. Numbers may be
signed; their magnitude sets the size. Diameter halves numeric second and third
sizes. Point input projects onto the accepted measurement axis and uses its full
distance. Enter cancels the third prompt. A zero third measurement ends without
adding geometry. Numeric `0` denotes the original CPlane origin at the center,
first axis and third axis; at the second axis it is an invalid length that can
be corrected before picking a direction.

FromFoci finishes a spheroid immediately after the through point. In the captured
Rhino behavior, its extent compensation scales about the original CPlane origin,
which can shift the center and alter spatial axes. Viboceros preserves this
placement. MarkFoci is remembered independently from Ellipse; the captured native
SubDEllipsoid command produces no additional point objects with MarkFoci enabled.

Defaults are ten around faces, ten vertical faces and quad pole caps.
`CapFaceStyle=Tri|Quad` is available for even around counts; odd counts use
triangles. Set topology options before entering a base mode or picking its first
point. Counts require at least three around and two vertical faces, at most
100,000 control vertices and faces, and at most 1,000,000 evaluated facets.
Focal compensation reaches the evaluated-facet budget earlier. These cages use
the rational mesh sampler and compensate sampled extents; they do not interpolate
all analytic ellipsoid samples.

Previews stay outside the document. Completion creates one object on the current
layer and one Undo/Redo step. Cancellation preserves pending Redo; changed history,
tolerance or guide geometry discards the draft. Guide selection and ordinary
Ellipse settings remain independent.

The [qualification record](../subd-ellipsoid-qualification.json) covers 64 native
workflows, including 53 constructions, at `1e-6`. It compares topology, tags,
controls, vertex limit stations, every density-two quad under a common
triangulation, source state and history. Arbitrary option timing, every guide
type, native packing, material/texture channels and comparative performance
remain unqualified. Sampled facets do not certify a continuous Hausdorff bound.
