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
leaves the trimmed walls, and open planar profiles produce sheets. For an
outward convex line-and-arc profile and a parallel rectangular cutter, the
cutter must span the section. A planar cutter on a slanted sweep uses the
viewport distance captured when entering `ToBoundary`. The temporary sweep
must reach past the cutter. A headless cut uses the combined bounds.
A slanted sweep with a continuous rim can be cut by a planar or bilinear
cutter that spans the section. A warped bilinear
cutter must project one-to-one along the sweep direction. Holes outside the
cut section leave it intact. For a planar cutter, `Solid=Yes` fills each
enclosed hole with a separate cap face and `Solid=No` leaves the cut walls.
A hole that crosses the rim rejects the cut. With a captured viewport distance
and `Solid=Yes`, the uncut sweep remains, and `DeleteInput=Yes` replaces the
input curve in the same Undo/Redo step. A successful viewport cut remembers
its captured distance.

For slanted polyline sweeps with gaps, `Solid=No` keeps the wall at the
curve's start when the cutter spans the walls. An open polyline produces the
same wall with either `Solid` setting. For nonrational curved open sweeps
with monotone offset rails, the cut retains the connected walls at the
start of the curve.
On independent straight walls, a partial planar cutter can leave an uncut
wall and subdivide its edges where the trace reaches them. A hole can
interrupt the trace and leave both side edges subdivided. Cutter orientation
can change whether a closed sweep retains the trimmed leading wall or the
uncut closing wall. With `Solid=Yes`, a closed polygon
can report a failed cut and retain the base-capped preview stock. The
captured sweep must reach the cutter before it can leave this result.
Partial cuts on curved walls, off-plane base points, holes within warped
sections, and collapsing curved stocks need further qualification.

Headless examples, with curves selected:

```text
ExtrudeCrvTapered 2 DraftAngle=10 Solid=Yes
ExtrudeCrvTapered -2 DraftAngle=-10 Corners=Round DeleteInput=Yes
ExtrudeCrvTapered 0,0,0 0,0,2 DraftAngle=10 Solid=Yes
ExtrudeCrvTapered 2 Direction=1,0,2 DraftAngle=10 Solid=Yes
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

A slanted `Direction` offsets each profile span perpendicular to its tangent
and the chosen axis. Sharp corners can leave gaps between adjacent walls, and
`Solid=Yes` keeps the base cap of a closed profile with the far end open.
The slanted sweep uses the same span breaks for both `SplitAtTangents` settings.

Each selected curve produces one independent B-rep. `Corners=Sharp` extends the
offset segments, `Round` joins them with circular arcs, and `Smooth` uses cubic
tangent joins. `Solid=Yes` caps closed planar curves on normal sweeps. Open
curves produce sheets. Native construction copies its profiles. All sources
complete before document edits.
Results use the current layer and form one Undo/Redo step.
Retained curves stay selected after extrusion and are released on replay.
`DeleteInput=Yes` deletes sources in that same operation.

Nonzero draft currently requires connected planar profiles. Axes in the
profile plane and Rhino's corner treatment very close to the normal still
need qualification, as do some automatic seams on fitted rational offsets.
Nested closed objects are extruded independently. Existing native curve-import
degree, knot and rational weight limits apply. Complex self-intersecting offsets
and collapsing profiles require broader qualification. No continuous error or performance certificate is
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
