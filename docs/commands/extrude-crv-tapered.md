# ExtrudeCrvTapered

`ExtrudeCrvTapered` creates tapered sheets or capped B-reps through usd-brep's
native draft-extrusion producer.

Start the command, select curves and press Enter, then type a distance or
click a point that sets it along the extrusion axis. Preselection works too.
The viewport previews the tapered walls as the cursor moves. Choose
`Direction` and pick two points to set the axis, then specify a distance.
`SetBasePoint` changes the reference for point-based distance input. Type
`DraftAngle` or `Corners` to open its getter, or enter values inline.
`FlipAngle` reverses the draft. `Solid` and `DeleteInput` toggle by name.
`SplitAtTangents` toggles separate faces where curvature changes across a
tangent join. The live preview updates to show the resulting seams. Sharp
corners and discontinuities introduced by the fitted offset can create seams
in either mode.
Typed distances accept document units. Enter repeats the last signed distance.
Failed geometry keeps the getter and input curves available for retry.
Esc cancels the command.

Choose `ToBoundary` at the distance getter and pick the cutter face. Closed
planar profiles with `Solid=Yes` retain the base and cutter caps. `Solid=No`
leaves the trimmed walls, and open planar profiles produce sheets. The cutter
must span the temporary taper, whose length comes from the combined bounds.

Headless examples, with curves selected:

```text
ExtrudeCrvTapered 2 DraftAngle=10 Solid=Yes
ExtrudeCrvTapered -2 DraftAngle=-10 Corners=Round DeleteInput=Yes
ExtrudeCrvTapered 0,0,0 0,0,2 DraftAngle=10 Solid=Yes
ExtrudeCrvTapered 2 Direction=0,0,1 DraftAngle=10 Solid=Yes
ExtrudeCrvTapered ToBoundary=cutter-id:face-index DraftAngle=-10 Solid=Yes
ExtrudeCrvTapered 1 DraftAngle=-8 Solid=Yes SplitAtTangents=Yes
```

The initial options are `DraftAngle=5 Solid=No Corners=Sharp DeleteInput=No`
and `SplitAtTangents=No`. The initial distance is 1. A successful command remembers these options
and its signed distance. Positive draft contracts closed profiles along the
default axis, including sweeps at a negative distance. Reversing the axis
with `Direction` reverses the draft. Numeric distance uses the profile
plane's normal, oriented toward its largest positive world component.
A line uses the active CPlane normal. Angles must be finite and strictly
between -90 and 90 degrees. Zero draft uses the native linear sweep.

Each selected curve produces one independent B-rep. `Corners=Sharp` extends the
offset segments, `Round` joins them with circular arcs, and `Smooth` uses cubic
tangent joins. `Solid=Yes` caps closed planar curves
and requires a closed manifold result; open curves produce sheets. Native
construction copies its profiles. All sources complete before document edits.
Results use the current layer and form one Undo/Redo step.
Retained curves stay selected after extrusion and are released on replay.
`DeleteInput=Yes` deletes sources in that same operation.

Nonzero draft currently requires connected planar profiles and extrusion normal
to their plane. Oblique directions, nonplanar profiles and some automatic seams
on fitted rational offsets still need qualification. Nested closed objects are
extruded independently. Existing native curve-import degree, knot and rational
weight limits apply. Complex self-intersecting offsets and collapsing profiles
require broader qualification. No continuous error or performance certificate is
claimed.

The adapter sets the native B-rep tolerance explicitly. The upstream producer
uses 0.001 times that value for offset fitting; too-small underflowing tolerances
and nonfinite offset distances decline. A generated-source
[sliver-domain patch](../../third_party/patches/usd-brep-tapered-sliver-domains.patch)
fixes missing rectangle walls by measuring each ruling curve on its own interval.
The [smooth corner patch](../../third_party/patches/usd-brep-tapered-smooth-corners.patch)
adds the cubic offset joint used by the native sweep.
The [profile partition patch](../../third_party/patches/usd-brep-tapered-profile-partition.patch)
lets the curve command supply the seams for its draft sweep.
The pinned upstream submodule is unchanged. Tests check circular and polygonal
frustum volumes, rounded rectangle volume, reversed extrusion, zero draft, shifted
parameter domains, open sheets, source preservation, rollback and history.

Seven independent Python recipes are available in
[the fixture](../../tools/rhino_oracle/fixtures/tapered_extrusion.json). They run the
Rust command and the public
[RhinoCommon tapered-extrusion API](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Geometry_Brep_CreateFromTaperedExtrude_1.htm).
Headless Rhino 8 command captures cover distance entry, direction reversal,
base-point input, remembered settings, smooth corners and source selection across
Undo and Redo. Boundary picks in Rhino's viewport cover capped and uncapped rectangles,
an oblique cutter, reversed sweeps and an open line. The SDK fixtures audit
bounds and volumes independently of the getter.
See the [qualification record](../tapered-extrusion-qualification.json) and
[Rhino command reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/extrudecrvtapered.htm).

```sh
cargo test -p viboceros-smlib --features native --test tapered
cargo test -p viboceros-command -p viboceros extrude_tapered
python3 -m tools.rhino_oracle viboceros tools/rhino_oracle/fixtures/tapered_extrusion.json
```
