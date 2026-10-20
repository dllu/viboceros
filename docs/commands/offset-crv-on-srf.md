# OffsetCrvOnSrf

Create surface-distance offsets from one curve or surface edge. Preselect a
curve, or start `OffsetCrvOnSrf` and pick a curve or edge. Pick a base surface
or B-rep face on the desired side, then enter a signed distance. `Flip` or a
viewport click reverses the side. `Number` sets the number of sequential
offsets; each copy advances the previous complete offset before face trimming.
This avoids duplicate copies when a hole splits only one of the offsets.

The viewport displays cached curves without editing the document. Enter
accepts the current distance. Escape discards the preview and preserves Redo.
Accepted curves use fresh attributes on the current layer; original selection
is retained and outputs remain unselected, matching the captured public
command. One Undo/Redo replays all copies. Distance and Number are remembered
for subsequent interactive invocations; Flip starts afresh.

At source selection, `SubCrv` picks a temporary directed portion: start and end
points, or a curve length with direction confirmation. `Direction=Locked|Free`
uses the shared directed-subcurve solver. Escape returns to source selection.
The source and any edge parent remain unchanged. Changes to a captured source,
face or model tolerance invalidate the getter.

Scripted forms use native curve parameters for SubCrv:

```text
OffsetCrvOnSrf 1 Source=<curve-id> Surface=<surface-id> Number=3
OffsetCrvOnSrf -0.5 Edge=<brep-id>,0 Surface=<brep-id> Face=1
OffsetCrvOnSrf 1 Source=<curve-id> Surface=<surface-id> SidePoint=0,3,0 Flip=Yes
OffsetCrvOnSrf 1 Source=<curve-id> Surface=<surface-id> SubCrv=6,2
```

Without `SidePoint`, positive distance follows the left side of the curve in
the face orientation. Explicit picks choose the physical side independently
of face reversal. Zero distance and Number outside 1–1024 are rejected.
Geometry is prepared before any output is added; failed operations preserve
the model, selection and history.

Planar open offsets extend along endpoint tangents to the natural surface
boundary, then clip against face loops and holes. This command policy differs
from the public SDK's source-endpoint-preserving producer. It reuses existing
curve boundary extension and certified face clipping. Closed offsets use the
qualified affine-planar gap reconnection policy where clipping is necessary.
Curved-surface open-end extension still needs native qualification.

Eight fixed public Rhino command captures cover signed distance, Flip,
Number persistence, line/NURBS inputs, preselection and postselection,
planar tangent extension, trimmed holes and grouped Undo/Redo. Both scripted
and UI paths replay their full output geometry and final metadata. Seven
meet the unchanged `1e-6` sampled geometry gate. The Bézier case remains
unqualified at about `1.23e-5`; a separate diagnostic ceiling prevents regressions
without treating it as parity. The earlier SDK matrix retains its five
unqualified cases. See [geometry and qualification evidence](../surface-geodesic-offset.md).

ThroughPoint, general closed-gap reconnection, corner joins, cut-locus removal,
pole crossings and complete topology certification remain pending. These
limits preclude a claim of complete Rhino compatibility.

```sh
cargo test -p viboceros-command --lib surface_curve_offset
cargo test -p viboceros-command --test surface_curve_offset_native -- --nocapture
cargo test -p viboceros-command --features native-smlib --test surface_curve_offset_native
cargo test -p viboceros --bin viboceros surface_curve_offset
python3 -m unittest tools.rhino_oracle.test_surface_offset_command
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.generate_surface_offset_command
```
