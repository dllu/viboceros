# Maelstrom live previews

[Command reference](commands/maelstrom.md) · [Architecture](architecture.md) · [Oracle](oracle.md)

Maelstrom shares prepared morph cages with Twist, Bend and Taper. Mouse motion
maps point/mesh wires, degree-elevated curve controls, surface display cages and
B-rep wire cages. Rigid objects share source display geometry and use the measured
object or top-group placement. Accepted operations use the full geometry fitter.
Pending previews do not edit the model or add Undo entries.

## Input and display

The first-radius getter shows a black circle through the cursor. The second
getter keeps that circle and adds its pending radius circle. After both radii are
accepted, the coil-angle getter keeps both guides and draws deformed wires in the
source color alongside the selected originals. Preview faces are not added;
surface interior wires remain visible in Wireframe, Shaded and Ghosted views.
A numeric zero second radius shows unchanged source wires as its preview.

Mouse angles use the accepted Circle frame and retain complete turns. Typed
coordinates retain those turns too; an explicit scalar angle replaces them.
Before any mouse angle, typed coordinates follow the measured branch documented
in the [command reference](commands/maelstrom.md). Snapping onto the axis hides
the pending deformation. Pointer departure and an incomplete point filter retain
the preceding preview. Cancel drops the pending preview; accepted copies stay
in their single Undo group. A new Copy placement starts without the old cursor.

The source cache tracks immutable geometry storage, wire density, group and model
tolerance. Repeated frames reuse the same image; changing the angle reuses the
prepared cages. Copy alone does not rebuild geometry. All viewports share the
session cache and accepted Circle frame.

## Evidence

Private Xvfb Rhino **8.32.26160.13001** captures retain 32 pending previews and
six earlier diagnostics. Coverage includes points, lines, cubic curves, surfaces,
solid boxes, colored meshes, Copy repeats, grouped Rigid objects, signed and
zero second radii, reversed and equal radii, all display modes, oblique planes,
spatial first-radius picks, cancellation and 450-degree mouse turns. Three
additional captures compare typed coordinates and scalar angles after mouse
motion. Three axis-input captures show that the hidden axial preview still accepts
an angle: zero after a quarter turn, or 360 degrees after a 450-degree turn.
Typed input uses the public
[RhinoApp.SendKeystrokes API](https://developer.rhino3d.com/api/rhinocommon/rhino.rhinoapp/sendkeystrokes)
after an owned framebuffer receipt.

SDK controls and curve samples are compared within `1e-11`, weights within
`1e-14`, native mesh vertices within `1e-6`, and rigid placements within `1e-7`.
Retained pixels constrain cubic curve cages within three pixels and surface
interior wires and black circle guides within four pixels. Camera replay checks
that previews and clicks use the same point within `1e-5`. Application tests
compare accepted geometry, cancellation, Copy resets and Undo.
The [provenance record](maelstrom-preview-provenance.json) hashes retained
fixtures, observations, PNGs and capture helpers.

The [first-circle getter](maelstrom-circle-input.md) adds construction and size
modes with independent frame and point-map captures. Native guide pixels for
those modes remain to verify.

These are sampled witnesses. Remaining Circle construction modes, complex trims,
additional drafting-aid combinations and original native performance comparisons
remain to verify. Dense fitted B-reps still need performance work.

```sh
cargo test --release -p viboceros maelstrom
python3 -m unittest tools.rhino_oracle.test_maelstrom_preview
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.maelstrom_preview_input \
  tools/rhino_oracle/fixtures/maelstrom_preview.json \
  --output /tmp/maelstrom-preview.json --scheme VibocerosOracleMaelstromPreview --timeout 420
```

The picker checks its owned Rhino process, window, viewport rectangle and bounded
mouse path, waits for a native motion acknowledgement, then captures private Xvfb
pixels before clicking or cancelling. No proprietary source was inspected.
