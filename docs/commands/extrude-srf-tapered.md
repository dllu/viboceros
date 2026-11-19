# ExtrudeSrfTapered

`ExtrudeSrfTapered` uses usd-brep's native
draft-extrusion producer for surface boundaries. It retains the actual source
face as the base cap, including curved surfaces with planar boundaries and trim
holes. Positive draft contracts material: outer loops shrink and holes expand.
Negative draft expands material. For the default axis, negative distance
sweeps the opposite side with the same draft.

Start the command, select surfaces and press Enter, then type a distance
or click a point that sets it along the extrusion axis. Preselected objects
and B-rep face components work. The viewport previews the tapered walls as
the cursor moves. `Direction` takes two points to set the axis and returns
to the distance getter. `SetBasePoint` changes the reference for point-based
distance input. Type `DraftAngle` or `Corners` to open its getter, or enter
values inline. `FlipAngle` reverses the draft. `Solid` and `DeleteInput`
toggle by name. Typed distances accept document units. Enter repeats the
last signed distance. Failed geometry keeps the getter and frozen source
faces available for retry. Esc cancels the command.

Choose `ToBoundary` at the distance getter to pick a cutter face. The cutter
getter also accepts `Direction`. A successful cut uses the actual cutter
surface as its end cap, including oblique and warped cutters. The temporary
taper uses the diagonal of the combined source and cutter bounds, so a narrow
cutter or a collapsing inward taper can fail even close to the source.
A cutter hole inside the section leaves an opening. One that crosses the
rim can leave the full temporary sweep and report a failed cut. A contracting
source opening certified to collapse during the temporary sweep is filled by
a separate face at the base. An oblique sweep with gaps between adjacent walls
retains its base-capped temporary sweep and reports a failed cut. A continuous
oblique sweep can be cut and capped by a planar face that covers the section.
Warped cutters, cutter holes and partial results from collapse still need
qualification for slanted sweeps.

With source objects selected:

```text
ExtrudeSrfTapered 2 DraftAngle=10
ExtrudeSrfTapered -2 DraftAngle=-10 Corners=Round
ExtrudeSrfTapered 0,0,0 0,0,2 DraftAngle=10 DeleteInput=Yes
ExtrudeSrfTapered 2 Face=0 DraftAngle=0
ExtrudeSrfTapered 2 Faces=0,2 Direction=0,0,1 Solid=No
ExtrudeSrfTapered 2 Faces=All
ExtrudeSrfTapered ToBoundary=cutter-id:face-index DraftAngle=-10
```

`FaceRefs=id:index,...` explicitly addresses original faces without object
selection. Repeated targets extrude once. Specify faces for whole polysurfaces;
a single-face surface needs no selector. Each face produces an independent
result. Inline face selectors and options survive source picking.

Initial options are `DraftAngle=5 Solid=No Corners=Sharp DeleteInput=No`
and the initial distance is 1. A successful command remembers these options
and its signed distance. Numeric
distance uses the native plane normal of a planar face, oriented toward its
largest positive world component. Curved faces use the active CPlane normal.
An explicit direction overrides this policy. Reversing the axis with
`Direction` reverses the draft. Slanted sweeps of planar faces offset each
boundary span in the direction perpendicular to its tangent and the sweep axis.
`Solid=Yes` keeps the actual source face at the base and leaves the far end open.
`Solid=No` creates separate objects for walls that have no shared edges. Zero
draft uses the exact face translation, including oblique sweeps and curved end caps.

`Corners=Sharp` extends the offset segments, `Round` joins them with circular
arcs, and `Smooth` uses cubic tangent joins. Each source
loop is tapered separately, with reversed draft for holes. With Solid=Yes,
source faces copy as the base, stitching joins their boundaries to the walls,
and the native planar-face builder caps the remaining nested top loops together.
The top must be planar to receive a cap. `Solid=No` creates walls only and also
supports nonplanar source boundaries.

DeleteInput removes extruded source faces; any remainder retains its object
identity, attributes and groups. All output and remainders stage before document
edits. Results use the current layer and form one Undo/Redo operation.
Retained sources stay selected after extrusion and are released on replay.
A later failing face preserves the full batch, selection, history
and Redo. The native adapter uses independent profile copies, explicit document
tolerance and control/face resource checks; input geometry stays unchanged.

Nonplanar offset ends cannot currently be capped for nonzero draft. Seam/vertex
loops, unsupported imports, zero displacement and invalid angles decline.
Angles must be finite and strictly between -90 and 90 degrees. Axes in the
profile plane and corner treatment near the normal still need qualification.
Self-intersecting offsets,
collapsing profiles, touching holes and near-degenerate cases require broader
qualification. Native UV trims can be closed within tolerance without identical
endpoint coordinates; the pure certified trimmed-bounds query can decline those
outputs. Native tight bounds provide their diagnostic extents without snapping
trim geometry. No continuous error or comparative performance certificate is
claimed. The existing generated-source sliver-domain patch also applies to this
producer; the pinned upstream submodule is unchanged.

Eleven independent Python `surface_tapered_extrusion` fixtures cover inward and
outward drafts, hole expansion, rounded corners, reversed extrusion, raised
Bezier bases, zero draft, nonplanar walls and translated sources. The Rust side
runs the actual command and records solid flags, face counts, volumes and native
tight bounds. The pending Rhino recipe joins copied edge curves, calls public
[Brep.CreateFromTaperedExtrude](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Geometry_Brep_CreateFromTaperedExtrude_1.htm),
retains the actual base face and caps the planar top. Zero draft uses public
BrepFace.CreateExtrusion. Mock tests cover SDK dispatch and owned-object cleanup.
Headless Rhino 8 command captures cover signed distances, direction and
base-point input, remembered settings and source selection. Boundary captures
cover flat, oblique and warped cutters, crossings at holes, and coincident caps.
Rhino subdivides planar inward taper walls into more faces. See
[qualification](../surface-tapered-extrusion-qualification.json) and
[Rhino's command reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/extrudesrftapered.htm).

```sh
cargo test -p viboceros-smlib --features native --test surface_tapered_extrusion
cargo test -p viboceros-command -p viboceros extrude_surface_tapered
python3 -m tools.rhino_oracle viboceros tools/rhino_oracle/fixtures/surface_tapered_extrusion.json
```
