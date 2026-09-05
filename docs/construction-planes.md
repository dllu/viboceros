# Construction-plane primitives

[Interface](interface.md) · [Typed points](point-input.md)

`Circle`, `Polygon`, `Rectangle`, `MeshPlane`, `Box`, and `MeshBox` use the
active viewport's construction plane. Top/Perspective use XY, Front uses XZ
(normal -Y), and Right uses YZ (normal +X). The first accepted interactive pick
captures the construction orientation; switching views for later picks does
not replace it. Typed coordinates still resolve in the viewport receiving
the input, so world-prefixed points are useful when mixing views.

Enter `Box`, then `0`, `4,5`, and `4,5,6` to build a solid on the active plane.
Circle, Polygon, Rectangle, MeshPlane, and MeshBox also mix mouse and typed
picks. Rejected picks remain correctable, Escape discards the draft, and a
completed primitive is one undo step.

## Geometry rules

- Numeric Circle/Polygon radii use the plane's positive X axis. A picked
  radius uses the full 3D displacement and tilts the construction frame toward
  the pick. Circle supports a purely normal pick using the plane's Y direction;
  Polygon requires a nonzero in-plane component.
- Rectangle normalizes the two corners to increasing plane X/Y coordinates,
  starts at the lower-left corner, and traverses counterclockwise. Rectangle
  and Polygon use chord-length native parameters, not vertex-index parameters.
- Rectangle, MeshPlane, Box, and MeshBox project the opposite base corner onto
  the plane through the first corner. Box height is a signed displacement along
  its normal; the picked height's tangential coordinates do not affect it.
- MeshBox retains its base grid first and produces an outward-oriented solid
  for either height sign. Raw grid ordering is not identical to Rhino in every
  viewport. Box remains an exact six-face B-rep, not a Rhino extrusion object.

One-line command point arguments remain world-space. Their construction
orientation comes from the active viewport in the app, or from the caller's
`CommandContext` in Rust. `CommandRegistry::execute` uses World XY;
`execute_in_context` accepts any validated translated/rotated frame. The
context is separate from document objects and undo history. Command implementations
live in `viboceros-command/plane_primitives`; `Frame3` supplies shared coordinate
mapping without dropping small components at mixed floating-point scales.

This is not full CPlane-command support: custom viewport planes, named planes,
scalar constraints during point prompts, and adapting all remaining primitives,
transforms, and `BoundingBox CoordinateSystem=CPlane` are still pending.

## Verification

`plane_primitives.json` exercises 57 actual Rhino command executions, including
five construction planes, reversed corners, signed and picked heights, and
off-plane/normal radius picks. All 57 agree within absolute/relative `1e-9`/`1e-12`;
the maximum observed coordinate difference is `1.07e-14`.
Curves retain native domains and sampled points;
MeshPlane retains raw vertices and faces. Box boundaries compare all vertices,
edge samples, face samples, oriented normals, and lifted trim loops. MeshBox
compares all raw vertex positions (including duplicates) and oriented faces.
Box/MeshBox ordering is canonicalized without rounding the reported coordinates.

`plane_primitives_representation.json` separately retains ten raw B-rep/mesh
layout probes. Rhino's extrusion-derived B-rep indexing, UV domains, and some
MeshBox grid orders differ (seven raw comparisons differ, three agree).
Boundary agreement is not a claim of identical
subobject numbering or representation. UI tests separately verify plane capture,
command replacement, cancellation, rejected input, and undo/redo.
