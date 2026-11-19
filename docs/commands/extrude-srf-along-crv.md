# ExtrudeSrfAlongCrv

`ExtrudeSrfAlongCrv` translates surface
boundaries along an open NURBS path with fixed profile orientation. The source
stays at its original position, independently of the path's location. Curved
source faces and trim holes remain in the caps when `Solid=Yes`.

Start the command, select surfaces and press Enter, then click the path near
the end you want to start from. Preselected surface objects and face components
work. At the path picker, toggle `Solid`, `DeleteInput`, `SubCurve` or
`SplitAtTangents` by name or set them to `Yes` or `No`. Enter accepts a
unique selected curve. You can also enter `Path=id` or `PathName=name`.
Esc cancels the getter.

Choose `ToBoundary` at the path picker, then pick a surface or a polysurface
face and select the path near its start. The cutter closes the result where
its finite trim spans the sweep. The path must reach the cutter. With
`SubCurve=Yes`, the wire preview shows the cut for the current interval.
Enclosed cutter holes leave openings in the result. A cutter hole that
interrupts the rim can leave a full sweep and report that the cut failed.
In a batch, `DeleteInput=Yes` removes only source faces that produce a cut.

With `SubCurve=Yes`, choose the path, then pick two points along it or type
their coordinates. The first pick anchors the interval, and the second has a
live wire preview of the translated surface. The direction comes from the
original path click, regardless of the order of the two point picks. Press
Enter at the first point to use the whole path. Enter at the second point
ends the command without adding geometry. For scripted intervals, use
`PathRange=t0,t1` in the original curve domain. Rhino 8's reversed intervals
use negated parameters, which can extrapolate a curved rail past its ends.

With selected source objects, you can use these command forms.

```text
ExtrudeSrfAlongCrv Path=object-id
ExtrudeSrfAlongCrv PathName=rail Solid=No Reverse=Yes
ExtrudeSrfAlongCrv Path=object-id Face=3 DeleteInput=Yes
ExtrudeSrfAlongCrv Path=object-id Faces=0,2 PathLength=10
ExtrudeSrfAlongCrv Path=object-id PathRange=0.3,0.65
ExtrudeSrfAlongCrv Path=object-id ToBoundary=cutter-id:face-index
```

`FaceRefs=id:index,...` addresses original face indices without object selection.
Repeated targets extrude once. Specify faces for whole polysurfaces. Each face
produces an independent result. The headless command also accepts one selected
path curve alongside its selected source faces. Named paths must be unique and
selectable. Source picking preserves inline face selectors and options.

Initial defaults are `Solid=No DeleteInput=No SubCurve=No SplitAtTangents=No`
and `Reverse=Auto`. A successful extrusion remembers the four Yes/No options.
`SplitAtTangents=Yes` adds wall edges at tangent joins with a curvature break.
Sharp bends in the rail split in either mode. The `SubCurve` preview shows
both sets of edges as the endpoint moves.
`Reverse=Yes` and `Reverse=No` explicitly choose the original end or start.
Typed paths and Enter use the original start in automatic mode. `PathLength`
takes an arc length from the chosen start. It accepts units in the path
picker and uses document model units in direct commands.
Open results produce a separate object for each set of walls joined by edges.
The path object is always retained. DeleteInput removes extruded source faces,
retaining any remainder's identity, attributes and groups. All products stage
before document edits and form one Undo/Redo entry. Frozen source snapshots
prevent a changed face index from targeting another face during picking.

The adapter uses usd-brep's exact translational sweeps for the prepared
profile pieces and rail segments. It retains the actual source faces as caps
and translates the far cap by the rail's endpoint displacement. It stitches
the result and validates solid closure. An aggregate control-net budget
bounds tensor-product allocation.

Closed or degenerate paths, closed sources without a boundary, invalid face
indices and unsupported native imports decline. Invalid picks can be retried.
A prepared sweep that misses its cutter leaves the geometry and undo history
intact. Self-intersection, folding and near-degenerate cases require broader
qualification.

Headless Rhino 8 command captures cover the factory cap setting, clicks on
both halves of a rail, whole-path confirmation, and `SubCurve` on linear,
quadratic, rational and multispan rails. Tangent split captures cover smooth
joins, corners, trim holes and sharp bends in the rail. Boundary captures cover
flat, oblique, warped and coincident cutters. Seven independent
Python `surface_path_extrusion` fixtures also exercise the kernel with planar and
warped caps, rational holes, partial paths and translated sources. The SDK
probe calls [BrepFace.CreateExtrusion](https://developer.rhino3d.com/api/rhinocommon/rhino.geometry.brepface/createextrusion).
For the open ring, both commands return two one-face objects. The Rhino SDK
returns a single B-rep with three faces and the same overall bounds.
[Rhino SDK capture](../../tools/rhino_oracle/observations/surface_path_extrusion_rhino.json)
See [Rhino's command reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/extrudesrfalongcrv.htm).

```sh
cargo test -p viboceros-smlib --features native --test surface_path_extrusion
cargo test -p viboceros-command -p viboceros extrude_surface_path
python3 -m tools.rhino_oracle viboceros tools/rhino_oracle/fixtures/surface_path_extrusion.json
```
