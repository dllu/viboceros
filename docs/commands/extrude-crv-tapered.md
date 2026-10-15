# ExtrudeCrvTapered

`ExtrudeCrvTapered` creates tapered sheets or capped B-reps through usd-brep's
native draft-extrusion producer. Build with `--features native-smlib`.

Start the command, select curves and press Enter, then enter a distance or pick
two direction points. Preselection works too. At the distance getter, edit
`DraftAngle`, `Solid`, `Corners`, `DeleteInput` or `Direction`; `FlipAngle`
reverses the draft. Typed distances accept document units. Picking two points
uses their displacement rather than moving the profile to the picked base.
Failed geometry keeps the getter, base point and source selection for retry.
Esc/Cancel exits the getter.

Headless examples, with curves selected:

```text
ExtrudeCrvTapered 2 DraftAngle=10 Solid=Yes
ExtrudeCrvTapered -2 DraftAngle=-10 Corners=Round DeleteInput=Yes
ExtrudeCrvTapered 0,0,0 0,0,2 DraftAngle=10 Solid=Yes
ExtrudeCrvTapered 2 Direction=0,0,1 DraftAngle=10 Solid=Yes
```

The initial options are `DraftAngle=5 Solid=No Corners=Sharp DeleteInput=No`.
Positive draft contracts closed profiles; negative draft expands them. Angles
must be finite and strictly between -90 and 90 degrees. A zero angle shares the
existing native linear-sweep path. Numeric distance uses the profile plane's
normal, oriented toward its largest positive world component; a line uses the
active CPlane normal. `Direction=x,y,z` overrides that direction. Distance sign
reverses extrusion while retaining the draft's inward/outward meaning.

Each selected curve produces one independent B-rep. Sharp corners use extended
offset joints; Round uses native fillets. `Solid=Yes` caps closed planar curves
and requires a closed manifold result; open curves produce sheets. Native
construction copies its profiles. All sources complete before document edits.
Results use the current layer, become selected, and form one Undo/Redo step;
`DeleteInput=Yes` deletes sources in that same operation.

Nonzero draft currently requires connected planar profiles and extrusion normal
to their plane. Oblique directions, nonplanar profiles, Smooth corners,
ToBoundary, SplitAtTangents control, live extrusion previews, saved native option
defaults and native getter parity remain future work. Nested closed objects are
extruded independently. Existing native curve-import degree, knot and rational
weight limits apply. Complex self-intersecting offsets and collapsing profiles
require broader qualification. No continuous error or performance certificate is
claimed.

The adapter sets the native B-rep tolerance explicitly. The upstream producer
uses 0.001 times that value for offset fitting; too-small underflowing tolerances
and nonfinite offset distances decline. A generated-source
[sliver-domain patch](../../third_party/patches/usd-brep-tapered-sliver-domains.patch)
fixes missing rectangle walls by measuring each ruling curve on its own interval.
The pinned upstream submodule is unchanged. Tests check circular and polygonal
frustum volumes, rounded rectangle volume, reversed extrusion, zero draft, shifted
parameter domains, open sheets, source preservation, rollback and history.

Seven independent Python recipes are available in
[the fixture](../../tools/rhino_oracle/fixtures/tapered_extrusion.json). They run the
Rust command and the public
[RhinoCommon tapered-extrusion API](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Geometry_Brep_CreateFromTaperedExtrude_1.htm).
Rhino capture remains pending because of the recorded
[Wine SSPI startup failure](../oracle-sspi.md). The recipes return face counts,
solid flags, bounds and volumes for audit; SDK topology and command interaction
are not asserted equivalent. Timings include document and diagnostic work.
See the [qualification record](../tapered-extrusion-qualification.json) and
[Rhino command reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/extrudecrvtapered.htm).

```sh
cargo test -p viboceros-smlib --features native --test tapered
cargo test -p viboceros-command -p viboceros --features native-smlib extrude_tapered
python3 -m tools.rhino_oracle viboceros tools/rhino_oracle/fixtures/tapered_extrusion.json --native-smlib
```
