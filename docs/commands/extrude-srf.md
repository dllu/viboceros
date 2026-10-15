# ExtrudeSrf

`ExtrudeSrf` sweeps surfaces and B-rep faces along a straight displacement using
usd-brep's native topology sweep. Build with `--features native-smlib`.
Trim holes and nonplanar source surfaces remain in the solid's caps.

Start the command, select surface objects and press Enter, then enter a distance
or pick two direction points. Preselected surfaces and B-rep face components
also work. At the distance getter, edit `BothSides`, `Solid`, `DeleteInput` or
`Direction=x,y,z`. Enter `Direction` to restart the two-point direction getter.
Typed distances accept document units. Two picked points specify displacement;
the profile stays at its original location. Failed geometry preserves the getter
and source picks for retry; Esc/Cancel releases the getter without editing geometry.
Frozen source snapshots prevent a changed face index from silently targeting
another face.

With selected objects:

```text
ExtrudeSrf 2
ExtrudeSrf -2 BothSides=Yes DeleteInput=Yes
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

Initial options are `BothSides=No Solid=Yes DeleteInput=No`. Each selected face
produces an independent result. Numeric distance uses the native plane normal
for a planar face, oriented toward its largest positive world component. Curved
faces use the active CPlane normal. An explicit direction overrides that policy.
`BothSides=Yes` extends from minus the displacement to plus the displacement,
doubling the indicated distance.

`Solid=Yes` keeps the original surface and its translated copy as caps and
requires a closed manifold result. `Solid=No` creates boundary walls only.
Sources remain unchanged unless `DeleteInput=Yes`: whole selected surface
objects are deleted, while selected faces are removed from a polysurface and
its remaining faces retain the original object identity, attributes and groups.
All geometry and remainders stage before document edits. Results use the current
layer, become selected, and form one Undo/Redo operation.

The adapter uses `SmTopologySweep` with `SmTranslationalSweepGeometry`, sweeps
only lamina boundary edges higher, and copies caps to their translated positions.
Shared interior edges remain in cap topology and do not generate interior walls
on connected sheets. The native plane query classifies NURBS control points with
`SmPointSet3d`; the generic surface predicate's padded pseudobox can conservatively
decline exact planes. Source geometry is copied before sweep, stitching and
orientation. Native construction and Rust B-rep transfer both validate results.

Tangential planar extrusion, zero displacement, closed sources without a boundary
and unsupported native imports decline. Native Rhino's `0`/`w0` origin shortcut,
ToBoundary, SplitAtTangents control, live extrusion previews, saved native option
defaults and full native getter/selection behavior remain future work. General
self-intersection and collapsing/degenerate surface cases need broader
qualification. No continuous error or comparative performance certificate is
claimed.

The Python `surface_extrusion` operation takes an owned `source`, original `face`
index, `displacement`, `solid` and `both_sides`. Eight independent fixtures cover
planar, curved, oblique, two-sided, holed, uncapped and box-face inputs. The pending
Rhino side calls public
[BrepFace.CreateExtrusion](https://developer.rhino3d.com/api/RhinoCommon/html/M_Rhino_Geometry_BrepFace_CreateExtrusion.htm).
Native SDK capture is still unavailable because of the recorded
[Wine SSPI startup failure](../oracle-sspi.md); no fresh Rhino capture or full
parity claim is made. Raw local observations retain volumes, bounds, solid flags
and face counts; timings include command/document and diagnostic work.
See [qualification](../surface-extrusion-qualification.json) and
[Rhino's command reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/extrudesrf.htm).

```sh
cargo test -p viboceros-smlib --features native --test surface_extrusion
cargo test -p viboceros-command -p viboceros --features native-smlib extrude_surface
python3 -m tools.rhino_oracle viboceros tools/rhino_oracle/fixtures/surface_extrusion.json --native-smlib
```
