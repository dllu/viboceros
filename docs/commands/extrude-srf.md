# ExtrudeSrf

`ExtrudeSrf` sweeps surfaces and B-rep faces along a straight displacement using
usd-brep's native topology sweep.
Trim holes and nonplanar source surfaces remain in the solid's caps.

Start the command, select surface objects and press Enter, then pick a distance
point or enter a length in document units. Preselected surfaces and B-rep face
components also work. The live preview shows the translated rims and walls,
including cap wires when `Solid=Yes`. The hovered side sets the sign for a
typed length. Press Enter to reuse the last successful length. A typed length
keeps its sign and a picked distance is stored as a magnitude. Type `0` or `w0`
to use the active CPlane or world origin.
In views looking down the extrusion axis, the mouse measures radial distance
from the guide's screen location. Picks below the guide use the negative side.

Enter `Direction` and pick two points to set the axis, then specify the distance.
`Direction=x,y,z` sets the axis directly. Press Enter in either direction prompt
to return to the previous picker with the current axis. `SetBasePoint` changes
where the distance is measured from. Press Enter there to restore the source
bounds center. Toggle `BothSides`, `Solid`, `DeleteInput` and
`SplitAtTangents` by name or set them to `Yes` or `No`. A successful extrusion
saves these four options for the next invocation, which starts with a fresh
direction and base point. A failed distance placement keeps the getter and source picks
available for retry. Esc cancels and discards pending option edits.

Enter `ToBoundary` at the distance prompt and click a surface or B-rep face
to finish the sweep at that surface. `Direction` remains available during
boundary selection. The sweep extends toward the cutter on one side and uses
the original face and the cutter as caps. Enclosed cutter holes leave open
rims. `DeleteInput` and `SplitAtTangents` apply to the result. Scripts can use
`ToBoundary=id` or `ToBoundary=id:face`.

A coplanar cutter that meets the source rim completes the sweep using the
original face as the closing cap.
A rejected cutter ends the picker. A cutter that crosses the sweep walls
can leave an untrimmed solid. The solid uses the diagonal of the combined
source and cutter bounds as its length. In a batch, completed cuts and
retained solids share one Undo step. `DeleteInput` applies to completed cuts.
The command leaves the picked cutter and retained preselected sources selected.

With selected objects:

```text
ExtrudeSrf 2
ExtrudeSrf -2 Solid=Yes BothSides=Yes DeleteInput=Yes
ExtrudeSrf 0,0,0 1,0,2 Solid=No
ExtrudeSrf 2 Face=3
ExtrudeSrf 2 Faces=0,2 DeleteInput=Yes
ExtrudeSrf 2 Faces=All
```

`FaceRefs=object-id:face-index,...` explicitly addresses selectable faces without
object selection. Repeated targets extrude once. Face indices refer to the
original B-rep. Specify faces for a whole polysurface; a single-face object needs
no selector. Source selection preserves inline face selectors and options.
Choose typed face selectors when starting the command, before its distance getter.

Initial options are `BothSides=No Solid=No DeleteInput=No SplitAtTangents=No`.
Each selected face is swept independently. Numeric distance uses the native
plane normal for a planar face, oriented toward its largest positive world
component. Curved faces use the CPlane normal captured when the interactive
command starts. An explicit direction overrides that policy. A point gives the
signed distance along the guide axis through the center of the source bounds or
the custom base point. The two-point script form supplies the full displacement.
`BothSides=Yes` extends from minus the displacement to plus the displacement,
doubling the indicated distance.

`Solid=Yes` keeps the original surface and its translated copy as caps and
requires a closed manifold result. `Solid=No` creates boundary walls only.
Disconnected boundary loops, including hole walls, produce separate objects
with `Solid=No`. Sharp corners always split the walls. `SplitAtTangents=Yes`
also splits at tangent joins with a curvature break, such as line-to-arc joins,
and adds the seams to the live preview. Original seams are retained and
changes in parameter speed alone leave the walls continuous.
Sources remain unchanged unless `DeleteInput=Yes`: whole selected surface
objects are deleted, while selected faces are removed from a polysurface and
its remaining faces retain the original object identity, attributes and groups.
All geometry and remainders stage before document edits. Results use the current
layer and form one Undo/Redo operation. For distance sweeps, whole-object
preselection stays on retained inputs. Explicit face targets and deleted-input
workflows select the results.

The adapter uses `SmTopologySweep` with `SmTranslationalSweepGeometry`, sweeps
only lamina boundary edges higher, and copies caps to their translated positions.
Shared interior edges remain in cap topology and do not generate interior walls
on connected sheets. The native plane query classifies NURBS control points with
`SmPointSet3d`; the generic surface predicate's padded pseudobox can conservatively
decline exact planes. Source geometry is copied before sweep, stitching and
orientation. Native construction and Rust B-rep transfer both validate results.

Tangential planar extrusion, zero displacement, closed sources without a
boundary and unsupported native imports decline. General self-intersection and
degenerate surfaces need broader qualification.

The Python `surface_extrusion` operation takes an owned `source`, original `face`
index, `displacement`, `solid` and `both_sides`. Eight independent fixtures cover
planar, curved, oblique, two-sided, holed, uncapped and box-face inputs.
The Rhino SDK run measures bounds, volumes, solid flags and face counts. The
SDK groups uncapped hole walls into one object. The command returns the outer
wall and hole walls as individual objects.
[BrepFace.CreateExtrusion](https://developer.rhino3d.com/api/RhinoCommon/html/M_Rhino_Geometry_BrepFace_CreateExtrusion.htm)

Separate private headless command captures cover world and CPlane origins,
custom base points, direction picks, mouse distance in Front, Top and Bottom
views, tangent wall topology and option and distance reuse. The initial kernel
qualification predates these captures. Boundary cases cover planar and warped
cutters, negative and oblique sweeps, and enclosed cutter apertures.
[Initial kernel qualification](../surface-extrusion-qualification.json)
· [Rhino command reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/extrudesrf.htm)

```sh
cargo test -p viboceros-smlib --features native --test surface_extrusion
cargo test -p viboceros-command -p viboceros extrude_surface
python3 -m tools.rhino_oracle viboceros tools/rhino_oracle/fixtures/surface_extrusion.json
```
