# ExtrudeSrfToPoint

`ExtrudeSrfToPoint` creates ruled walls
from surface boundaries to an explicit apex. `Solid=Yes` retains the actual
source surface as the base cap, including curved surfaces and trim holes.

Start the command, select surfaces and press Enter, then pick an apex or enter
its coordinates. Preselected surface objects and B-rep face components work.
The viewport previews the trimmed boundaries and ruled walls as the apex
moves. Enter `0` for the active CPlane origin or `w0` for the world origin.
At the point getter, edit `Solid=Yes|No`, `DeleteInput=Yes|No` and
`SplitAtTangents=Yes|No`, or enter an option name to toggle it. Typed points
use shared drafting input, including document units, CPlane coordinates and
object snaps. Failed geometry keeps the getter and frozen face sources for
retry. Esc/Cancel releases it without editing geometry. A changed source
snapshot declines instead of interpreting a stale face index.

With selected source objects:

```text
ExtrudeSrfToPoint 1,1.5,6
ExtrudeSrfToPoint 1,1.5,-6 Solid=No SplitAtTangents=Yes
ExtrudeSrfToPoint 2,2,2 Face=0 DeleteInput=Yes
ExtrudeSrfToPoint 2,2,2 Faces=0,2
ExtrudeSrfToPoint 2,2,2 Faces=All
```

`FaceRefs=id:index,...` explicitly addresses original face indices without
object selection. Repeated targets extrude once. Specify faces for a whole
polysurface; single-face surfaces need no selector. Each selected face produces
an independent result. Inline face selectors and options survive source picking.

At the apex prompt, enter `ToBoundary`, pick one boundary surface or B-rep
face, then place the apex beyond it. The preview ends at the boundary and the
result uses the base and the picked surface as caps. Holes enclosed by the cut
section remain open and their rims appear in the preview. Partial coverage and
holes crossing the section rim leave the point prompt available for retry.
Scripts can use `ToBoundary=id` or `ToBoundary=id:index`.

Initial options are `Solid=No DeleteInput=No SplitAtTangents=No`. A successful
extrusion saves those choices for the next invocation. Esc discards provisional
option changes.
`Solid=No` omits the source cap
and creates boundary walls only. DeleteInput removes extruded source faces,
retaining any remainder's identity, attributes and groups. All output and
remainders stage before document edits. Results use the current layer, become
selected and form one Undo/Redo operation. A failure on a later selected face
preserves the entire document batch, source picks, history and Redo.

With `Solid=No`, disconnected outer and hole walls become separate objects,
in boundary order. Their common apex does not join the walls. All products and
source deletion still form one Undo/Redo operation.

`SplitAtTangents=No` keeps tangent boundary joins within a wall surface;
`Yes` splits at native smoothness breaks. Sharp corners split either way.
Closed-curve seams remain where the source placed them. Parameter speed changes
alone do not split a geometrically smooth boundary. Changing the option
updates the radial seams in the live preview, including sweeps to a boundary.

The native adapter queries public OpenNURBS for G1 or aesthetic smoothness
breaks, copies boundaries over their actual edge intervals and calls
`SmBSplineSurface::CreateRuledSurface`
with a weight-preserving collapsed copy at the apex. Native face construction
handles singular apex topology; stitching and orientation join the walls and
copied source faces. Shared interior edges on connected sheets do not generate
walls. Source geometry stays unchanged. Control-net and face-count limits bound
construction, and capped output must close as a manifold solid. Commands work
in coordinates relative to the source before restoring its original position.

Planar sources accept targets in their plane. Coincident rim and radial edges
keep separate topology, including when the apex lies on an edge. An apex at a
wall endpoint or closed seam omits that wall from the result and preview.
`Solid=Yes` keeps the base. The result can be open or have zero volume.
When every wall is omitted, `Solid=No` produces a point at the apex. Rhino
stores that point as a vertex-only B-rep.
With a planar cutter, `ToBoundary` also accepts very shallow and coplanar
sweeps when the picked face spans the full section. Perspective projection
preserves the exact rim, enclosed cutter holes and shared edge topology. A
coplanar section collapses to a line and the result has zero volume. The
analytic cut also works with a perpendicular cutter for a flat sweep. Rhino
declined that alignment in the captured rectangular case.

Flat sweeps to a warped cutter use the picked face's intersection with the
source plane as the rim. Sections outside the sweep are ignored. A flat cut
requires one connected section spanning the sweep. The cut retains the
source and wall topology and uses a collapsed cap bounded by the rim. One
tilted Rhino capture retained the full cutter face.

Very shallow noncoplanar sweeps to a warped cutter can use a tighter working
tolerance and an affine stretch along the source normal. The trim is restored
to the original coordinates on the warped cutter. Licensed captures cover
placements above and below the source plane and a tilted, translated source.

For a noncoplanar apex, a warped cutter can intersect the sweep in several
disconnected sections. The command keeps the nearest complete section and
leaves the apex getter open if a closer partial section interferes. The
viewport uses the first hit along each sampled ruling. Within one model
tolerance of the source plane, competing hits on a ruling hide the preview.
For a planar cutter that cannot be meshed, the preview intersects the plane
and tests its trims directly.

Closed sources without a boundary, invalid face indices and unsupported
imports decline. Extremely small nonzero heights, shallow near-parallel
cutters, and general self-intersection and folding require broader
qualification.

Nine independent Python `surface_apex_extrusion` fixtures cover planar and
warped bases, targets above/below/off-axis, rational holes, uncapped walls,
box-face inputs and translated sources. The Rust side runs the actual command.
The SDK comparison uses public
[Surface.CreateExtrusionToPoint](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Geometry_Surface_CreateExtrusionToPoint.htm),
retains a duplicated source face and joins the resulting B-reps. Its nine
licensed captures match within an absolute epsilon of 0.000001. Separately,
72 licensed command captures cover both Solid/DeleteInput settings,
preselection and source picking. Rust command and GUI replays check product
counts, bounds, volumes, retained source identity and Undo/Redo. Selection,
attributes and full getter equivalence are outside that comparison.
Private headless Rhino captures also cover mouse input, both origin shortcuts,
option toggles, reuse of successful option changes, and coplanar points on
rectangles, annuli, and closed seams.
Separate `ToBoundary` captures cover planar and warped caps, a circular
opening, incomplete section coverage, and oblique cuts of coplanar sweeps.
Ordinary sweeps use a native Boolean cut with a check that the base and
boundary cap survive.
The SDK recipe combines disconnected walls for its geometry comparison;
the command corpus verifies separate output objects. See
[native qualification](../surface-apex-native-qualification.json),
[168 tangent-option comparisons](../surface-apex-tangents-qualification.json), and
[Rhino's command reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/extrudesrftopoint.htm).

```sh
cargo test -p viboceros-smlib --features native --test surface_apex_extrusion
cargo test -p viboceros-oracle --test surface_apex_native
python3 -m tools.rhino_oracle replay tools/rhino_oracle/fixtures/surface_apex_tangents.json \
  --observations tools/rhino_oracle/observations/surface_apex_tangents.json \
  --absolute-epsilon 1e-6 --relative-epsilon 0
```
