# Object snap controls

[Viewport controls](interface.md) · [Center capture](center-hover-snaps.md) · [Oracle provenance](oneshot-snap-provenance.json)

The **Snap modes** toolbar menu independently enables Point, End, Mid, Cen and
Quad. The Osnap button/F4 suspends persistent snaps without losing that selection.
Persistent checkbox edits keep the menu open; right-click a mode to isolate it,
then right-click it again to restore the previous set. An ordinary checkbox edit
starts a new set and discards the old isolation snapshot. These are session UI
settings, not document edits or model undo steps.

While a command requests a point, Shift-click a mode for the next point only.
Alternatively submit `Point`, `End`, `Mid`, `Cen`, `Quad` or `NoSnap` at the command
line, then pick or type the point. `Endpoint`, `Midpoint`, `Center`, `Quadrant`
and optional `_`/apostrophe prefixes are accepted. Submit the modifier separately
from coordinates; arbitrary multi-command macros are not parsed. Outside a point
prompt, `Point` still starts the modeling command.

The toolbar shows the pending override. It replaces, rather than adds to, the
persistent set—even when persistent snaps are disabled. `NoSnap` suppresses object
snaps for one point; it does not disable grid snap or SmartTrack. A successful
typed or picked point clears the override; rejected input retains it. Completion,
cancellation and command replacement discard pending overrides. View/display
commands and point options, including SplitEdge distance constraints, preserve
them. Transparent CPlane input has its own override, so it does not consume a
suspended modeling prompt's pending snap. Object/edge selection and CPlane Rotate's
numeric angle stage do not accept one-shot modifiers.

This follows the documented [persistent and one-shot distinction in Rhino](https://docs.mcneel.com/rhino/8/help/en-us/user_interface/object_snaps.htm).
Detailed native error/prompt-stack behavior is covered by application tests, not
claimed to be fully equivalent to Rhino in every interaction.

## Shared capture and evidence

`app/snapping` owns persistent selection and scoped overrides. Each frame passes
the effective `ObjectSnapModes` into the shared drafting cache. Parallel queries
retain their indexed point-cloud and relative-coordinate behavior; Perspective
uses the same mode mask with camera projection. The SplitEdge endpoint shortcut
now explicitly requires End, rather than incorrectly activating for any mode.

The [four retained requests](../tools/rhino_oracle/fixtures/oneshot_snap_modes.json)
and [raw observations](../tools/rhino_oracle/observations/oneshot_snap_modes.json)
each perform two actual SplitEdge picks, followed by Undo/Redo, in a newly owned
private Xvfb Rhino 8.32.26160.13001 session. One-shot Cen is followed by the full
five-mode persistent set; one-shot End, Point and Mid are each followed by Cen
only. Every point prompt records the actual world-to-screen matrix, camera, aim
and integer click, using the [calibration protocol](center-hover-snaps.md).

All eight native captures select the expected feature/object. Those independently
computed points drive complete ordered command geometry, attributes, selection and
history replay at absolute epsilon `1e-9`, relative `1e-10`. The second Rhino pick
has no one-shot prefix, establishing restoration for these cases. The replay uses
the core mode-filtered capture query; application lifecycle is separately tested.

Real egui pointer events cover normal, secondary and Shift-click menu input,
preservation of partial typed modeling input, and override consumption. Additional
tests cover all four viewport kinds, invalid points, command replacement, repeated
points, SplitEdge constraints, and transparent CPlane input.

Verification checkpoint: 3,023 release-mode workspace tests, 276 Python tests,
seven offscreen GPU tests, formatting, and Clippy/Rustdoc with warnings denied.
Toolbar regression tests also prevent model Undo/Redo during edge/group prompts.

## Remaining work

Coverage remains limited to the five currently implemented feature kinds. The
subsequent [Mid-hover implementation](mid-hover-snaps.md) adds whole-segment
capture when Mid alone is enabled, including one-shot Mid.
[Polygon Center](polygon-center-snaps.md) adds corner averages for closed linear
boundaries and polygonal planar surfaces/faces without holes.
[Circular NURBS Center](circular-center-snaps.md) adds circles/arcs and boundary
edges, including circular holes. General End/Near/Int/Tan/Perp behavior,
elliptical NURBS Center recognition,
CPlane-relative Quad, occlusion,
Alt suspension, full `Osnap` command grammar, persistence across app restarts and
arbitrary macros remain incomplete. New controls do not establish broad snap
parity or change the limits of the underlying geometry queries.
