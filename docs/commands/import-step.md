# ImportStep

```text
ImportStep "part.step"
ImportStep Native=Yes "part.step"
```

The default imports display meshes. `Native=Yes` imports supported editable B-reps
without tessellation, with assembly placements, occurrence names, source units
converted to document units, and objects assigned to the current layer.
Quote filenames beginning with `Native=Yes` to treat that text as a filename.
The `ImportStp` alias accepts the same syntax.

Native mode supports straight-edged planar faces, including valid polygon holes,
and NURBS/B-spline surfaces with matching curved 3D edges and UV trims. Analytic
circle and ellipse arcs on supported faces convert to exact multi-span rational
NURBS edges and UV curves. Bounded parabola and hyperbola edges and UV trims
also convert to exact quadratic NURBS. Cylindrical and nonsingular conical faces
with straight UV iso-trims spanning up to one turn convert to exact rational
NURBS patches,
including a full-turn wall with a paired `SEAM_CURVE` and explicit face-local
UV curves. This changes the angular parameterization within each arc while
retaining the surface and trim geometry. Ring-torus patches and spherical bands
away from the poles with straight UV iso-trims, including certified collinear
higher-degree p-curves, also convert to exact rational patches. Full major-angle
torus strips and full-longitude sphere bands can use
paired seams; pole trims still require singular-topology support. Linear
extrusions of line and B-spline/NURBS directrices convert to exact tensor-product
NURBS surfaces. Revolutions of those directrices with straight UV iso-trims also
convert to exact rational patches over angles up to one turn, including paired
seams on a full turn. Faces with multiple loops support certified straight
segment UV trims, including collinear higher-degree NURBS trims. The polygon
constructor validates hole containment and intersections. Curved UV holes with
one closed four-span quadratic NURBS trim are supported inside convex
polygonal outer loops when control sectors certify a simple, separate hole.
Certified quadratic outer loops can also contain holes inside their endpoint
quadrilateral. Other curved multi-loop regions remain unsupported. Periodic
seam arrangements, missing trims, and invalid topology fail the import; there
is no automatic mesh fallback. Assembly and representation warnings
follow the existing STEP importer and are counted in the result message.

All shells belonging to one shape occurrence are combined into one object.
Outer/cavity orientation is retained, and coincident topology is not welded.
This is topology assembly, not a Boolean union or validation of solid cavity
containment. Distinct occurrences remain separate objects, even with equal names.

Geometry is converted and combined before insertion. Import is one undoable
command; failures preserve the document and its undo/redo history. Tests cover
quoted/repeated-space paths, physical unit conversion, editable object type,
Undo/Redo, malformed files, and a hollow cube imported as one object of volume 992.
A mixed-file regression begins with two supported shells, changes only the
later shell's plane to an unsupported cylinder, and verifies loss-free STEP
record parsing before import. Both `ImportStep Native=Yes` and
`ImportStp native=yes` fail on that later shell with the complete document and
redo history unchanged; the previously undone command remains redoable.

Imported native objects can be saved with `Export3dm`. The hollow-cube regression
exports and reimports the object, checking surface/curve data, topology, cavity
sense, area 624, volume 992, and Undo/Redo. Export leaves the source document
unchanged. The 3DM writer derives isoparametric flags for trims marked `NotIso`
using OpenNURBS' classification; STEP import does not currently populate those
flags itself. This archive metadata adjustment does not alter trim geometry.
Additional round-trip checks cover polygon holes in either face orientation and
explicit linear B-spline, rational B-spline, and two-point polyline trims. They
verify exact control data and parameter intervals, topology and face sense, plus
interior isocurve classification on hole boundaries.

See [file formats](../file-formats.md) and the
[native STEP conversion boundary](../step-brep-boundary.md) for limitations.
