# ExtrudeSrfToPoint

Build with `--features native-smlib`. `ExtrudeSrfToPoint` creates ruled walls
from surface boundaries to an explicit apex. `Solid=Yes` retains the actual
source surface as the base cap, including curved surfaces and trim holes.

Start the command, select surfaces and press Enter, then pick an apex or enter
its coordinates. Preselected surface objects and B-rep face components work.
At the point getter, edit `Solid=Yes|No` and `DeleteInput=Yes|No`. Typed points
use shared drafting input, including document units, CPlane coordinates and
object snaps. Failed geometry keeps the getter and frozen face sources for
retry. Esc/Cancel releases it without editing geometry. A changed source
snapshot declines instead of interpreting a stale face index.

With selected source objects:

```text
ExtrudeSrfToPoint 1,1.5,6
ExtrudeSrfToPoint 1,1.5,-6 Solid=No
ExtrudeSrfToPoint 2,2,2 Face=0 DeleteInput=Yes
ExtrudeSrfToPoint 2,2,2 Faces=0,2
ExtrudeSrfToPoint 2,2,2 Faces=All
```

`FaceRefs=id:index,...` explicitly addresses original face indices without
object selection. Repeated targets extrude once. Specify faces for a whole
polysurface; single-face surfaces need no selector. Each selected face produces
an independent result. Inline face selectors and options survive source picking.

Initial options are `Solid=Yes DeleteInput=No`. `Solid=No` omits the source cap
and creates boundary walls only. DeleteInput removes extruded source faces,
retaining any remainder's identity, attributes and groups. All output and
remainders stage before document edits. Results use the current layer, become
selected and form one Undo/Redo operation. A failure on a later selected face
preserves the entire document batch, source picks, history and Redo.

The native adapter copies boundary curves over their actual edge intervals,
splits at G1 discontinuities and calls `SmBSplineSurface::CreateRuledSurface`
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
meshing guarantees. Numeric distance shortcuts, ToBoundary, SplitAtTangents
controls, live previews, saved option defaults and full native getter behavior
remain future work. Supply a point explicitly. No continuous error or
comparative performance certificate is claimed.

Nine independent Python `surface_apex_extrusion` fixtures cover planar and
warped bases, targets above/below/off-axis, rational holes, uncapped walls,
box-face inputs and translated sources. The Rust side runs the actual command.
The pending Rhino recipe uses public
[Surface.CreateExtrusionToPoint](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Geometry_Surface_CreateExtrusionToPoint.htm),
retains a duplicated source face and joins the resulting B-reps. SDK mock tests
check allocation cleanup and plumbing, and do not establish native parity.
Licensed Rhino capture remains unavailable because of the recorded
[Wine SSPI startup failure](../oracle-sspi.md). See
[qualification](../surface-apex-extrusion-qualification.json) and
[Rhino's command reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/extrudesrftopoint.htm).

```sh
cargo test -p viboceros-smlib --features native --test surface_apex_extrusion
cargo test -p viboceros-command -p viboceros --features native-smlib extrude_surface_apex
python3 -m tools.rhino_oracle viboceros tools/rhino_oracle/fixtures/surface_apex_extrusion.json --native-smlib
```
