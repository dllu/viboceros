# ExtrudeSrfToPoint

Build with `--features native-smlib`. `ExtrudeSrfToPoint` creates ruled walls
from surface boundaries to an explicit apex. `Solid=Yes` retains the actual
source surface as the base cap, including curved surfaces and trim holes.

Start the command, select surfaces and press Enter, then pick an apex or enter
its coordinates. Preselected surface objects and B-rep face components work.
At the point getter, edit `Solid=Yes|No`, `DeleteInput=Yes|No` and
`SplitAtTangents=Yes|No`. Typed points
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

Initial options are `Solid=Yes DeleteInput=No SplitAtTangents=No`.
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
alone do not split a geometrically smooth boundary.

The native adapter queries public OpenNURBS for G1 or aesthetic smoothness
breaks, copies boundaries over their actual edge intervals and calls
`SmBSplineSurface::CreateRuledSurface`
with a weight-preserving collapsed copy at the apex. Native face construction
handles singular apex topology; stitching and orientation join the walls and
copied source faces. Shared interior edges on connected sheets do not generate
walls. Source geometry stays unchanged. Control-net and face-count limits bound
construction, and capped output must close as a manifold solid. Commands work
in coordinates relative to the source before restoring its original position.

Coplanar targets for planar sources, closed sources without a boundary, invalid
face indices and unsupported imports decline. General self-intersection,
folding and near-degenerate cases require broader qualification. Hole walls
meet at a singular apex; this does not establish vertex-manifold or downstream
meshing guarantees. Numeric distance shortcuts, ToBoundary,
live previews, saved option defaults and full native getter behavior
remain future work. Supply a point explicitly. No continuous error or
comparative performance certificate is claimed.

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
The SDK recipe combines disconnected walls for its geometry comparison;
the command corpus verifies separate output objects. See
[native qualification](../surface-apex-native-qualification.json),
[168 tangent-option comparisons](../surface-apex-tangents-qualification.json), and
[Rhino's command reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/extrudesrftopoint.htm).

```sh
cargo test -p viboceros-smlib --features native --test surface_apex_extrusion
cargo test -p viboceros-oracle --features native-smlib --test surface_apex_native
python3 -m tools.rhino_oracle replay tools/rhino_oracle/fixtures/surface_apex_tangents.json \
  --observations tools/rhino_oracle/observations/surface_apex_tangents.json \
  --native-smlib --absolute-epsilon 1e-6 --relative-epsilon 0
```
