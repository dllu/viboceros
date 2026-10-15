# ExtrudeSrfAlongCrv

Build with `--features native-smlib`. `ExtrudeSrfAlongCrv` translates surface
boundaries along an open NURBS path with fixed profile orientation. The source
stays at its original position, independently of the path's location. Curved
source faces and trim holes remain in the caps when `Solid=Yes`.

Start the command, select surfaces and press Enter, then click a path curve.
Preselected surface objects and face components work. At the path picker, edit
`Solid`, `DeleteInput`, `Reverse` and `PathLength`, or enter `Path=id` /
`PathName=name`. Enter accepts a unique selected curve. PathLength accepts units
in the application; direct commands use document model units. Failed geometry
keeps the picker and source selection for retry. Esc/Cancel releases the picker.

With selected source objects:

```text
ExtrudeSrfAlongCrv Path=object-id
ExtrudeSrfAlongCrv PathName=rail Solid=No Reverse=Yes
ExtrudeSrfAlongCrv Path=object-id Face=3 DeleteInput=Yes
ExtrudeSrfAlongCrv Path=object-id Faces=0,2 PathLength=10
```

`FaceRefs=id:index,...` addresses original face indices without object selection.
Repeated targets extrude once. Specify faces for whole polysurfaces. Each face
produces an independent result. The headless command also accepts one selected
path curve alongside its selected source faces. Named paths must be unique and
selectable. Source picking preserves inline face selectors and options.

Defaults are `Solid=Yes DeleteInput=No Reverse=No`, with the whole path used.
`Reverse` changes the path direction before trimming. `PathLength` takes an arc
length from that prepared start; it does not move the source to a picked point.
The path object is always retained. DeleteInput removes extruded source faces,
retaining any remainder's identity, attributes and groups. All products stage
before document edits and form one Undo/Redo entry. Frozen source snapshots
prevent a changed face index from targeting another face during picking.

The adapter delegates boundary chain construction and translational wall sweeps
to usd-brep's `SmPrimitiveCreation::CreateCurveSweepFromEdges`. It copies actual
source faces and translates copies by the path endpoint displacement for caps,
then stitches, orients and validates the B-rep. An aggregate control-net budget
bounds tensor-product allocation before invoking the native producer. Solid
output must close as a manifold.

Closed or degenerate paths, closed sources without a boundary, invalid face
indices and unsupported native imports decline. Self-intersection, folding and
near-degenerate cases require broader qualification. Automatic direction from
the clicked path end, native SubCurve point picking, ToBoundary,
SplitAtTangents controls, live previews and saved option defaults remain future
work. Use `Reverse` and `PathLength` explicitly. Continuous error and comparative
performance certificates are not claimed.

Seven independent Python `surface_path_extrusion` fixtures cover planar and
warped caps, reversed paths, rational holes, walls, partial paths and translated
sources. They run the actual Rust command; the pending Rhino side calls public
[BrepFace.CreateExtrusion](https://developer.rhino3d.com/api/rhinocommon/rhino.geometry.brepface/createextrusion).
No licensed Rhino capture was obtained because of the recorded
[Wine SSPI startup failure](../oracle-sspi.md). Local results and validation are
recorded in [qualification](../surface-path-extrusion-qualification.json).
See also [Rhino's command reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/extrudesrfalongcrv.htm).

```sh
cargo test -p viboceros-smlib --features native --test surface_path_extrusion
cargo test -p viboceros-command -p viboceros --features native-smlib extrude_surface_path
python3 -m tools.rhino_oracle viboceros tools/rhino_oracle/fixtures/surface_path_extrusion.json --native-smlib
```
