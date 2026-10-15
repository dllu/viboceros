# ExtrudeSrfTapered

Build with `--features native-smlib`. `ExtrudeSrfTapered` uses usd-brep's native
draft-extrusion producer for surface boundaries. It retains the actual source
face as the base cap, including curved surfaces with planar boundaries and trim
holes. Positive draft contracts material: outer loops shrink and holes expand.
Negative draft expands material. Distance sign reverses extrusion while retaining
that inward/outward meaning.

Start the command, select surfaces and press Enter, then enter a distance or
pick two direction points. Preselected objects and B-rep face components work.
At the getter, edit `DraftAngle`, `Solid`, `Corners`, `DeleteInput` and
`Direction=x,y,z`; `FlipAngle` reverses the draft. `Direction` or `SetBasePoint`
restarts two-point input. Typed distances accept document units. Two points
specify displacement, with the source staying at its original location.
Invalid geometry keeps the getter and frozen source faces for retry. Changed
source geometry declines instead of reinterpreting an index. Esc/Cancel releases
the getter without editing geometry.

With source objects selected:

```text
ExtrudeSrfTapered 2 DraftAngle=10
ExtrudeSrfTapered -2 DraftAngle=-10 Corners=Round
ExtrudeSrfTapered 0,0,0 0,0,2 DraftAngle=10 DeleteInput=Yes
ExtrudeSrfTapered 2 Face=0 DraftAngle=0
ExtrudeSrfTapered 2 Faces=0,2 Direction=0,0,1 Solid=No
ExtrudeSrfTapered 2 Faces=All
```

`FaceRefs=id:index,...` explicitly addresses original faces without object
selection. Repeated targets extrude once. Specify faces for whole polysurfaces;
a single-face surface needs no selector. Each face produces an independent
result. Inline face selectors and options survive source picking.

Defaults are `DraftAngle=5 Solid=Yes Corners=Sharp DeleteInput=No`. Numeric
distance uses the native plane normal of a planar face, oriented toward its
largest positive world component. Curved faces use the active CPlane normal.
An explicit direction overrides this policy. Nonzero draft on a planar face
currently requires extrusion normal to its plane. Zero draft uses the existing
exact face translation, including oblique sweeps and curved end caps.

Sharp corners use extended offset joints; Round uses native fillets. Each source
loop is tapered separately, with reversed draft for holes. With Solid=Yes,
source faces copy as the base, stitching joins their boundaries to the walls,
and the native planar-face builder caps the remaining nested top loops together.
The top must be planar for nonzero draft. Solid=No creates walls only and also
supports nonplanar source boundaries. A requested solid must close as a manifold.

DeleteInput removes extruded source faces; any remainder retains its object
identity, attributes and groups. All output and remainders stage before document
edits. Results use the current layer, become selected and form one Undo/Redo
operation. A later failing face preserves the full batch, selection, history
and Redo. The native adapter uses independent profile copies, explicit document
tolerance and control/face resource checks; input geometry stays unchanged.

Nonplanar offset ends cannot currently be capped for nonzero draft. Seam/vertex
loops, unsupported imports, zero displacement and invalid angles decline.
Angles must be finite and strictly between -90 and 90 degrees. Smooth corners,
nonzero oblique planar sweeps, ToBoundary, saved option defaults, live previews
and full native getter behavior remain future work. Self-intersecting offsets,
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
No licensed Rhino capture was obtained because of the recorded
[Wine SSPI startup failure](../oracle-sspi.md). See
[qualification](../surface-tapered-extrusion-qualification.json) and
[Rhino's command reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/extrudesrftapered.htm).

```sh
cargo test -p viboceros-smlib --features native --test surface_tapered_extrusion
cargo test -p viboceros-command -p viboceros --features native-smlib extrude_surface_tapered
python3 -m tools.rhino_oracle viboceros tools/rhino_oracle/fixtures/surface_tapered_extrusion.json --native-smlib
```
