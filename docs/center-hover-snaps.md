# Curve-hover Center snaps

[Drafting controls](interface.md) · [Previous feature audit](composite-feature-snaps.md) · [Provenance and hashes](center-hover-snaps-provenance.json)

Center capture now follows the cursor's proximity to an analytic circle, arc or
ellipse, including arc leaves in polycurves. The returned model point is the
curve's center, which may be far from the cursor and off the construction plane.
Hovering over an empty center alone does not admit the snap. Hidden geometry is
excluded; locked objects and layers remain eligible.

Enabled direct point features take precedence over Center on the same object.
Candidates on different objects compete by capture distance: a circle's curve
can be closer than a nearby point object, even though the circle's center is
farther away. Exact-distance ties retain the existing feature-kind ordering.
These rules match the retained cases below, not every Rhino priority interaction.

## Calibrated evidence

The [22 requests](../tools/rhino_oracle/fixtures/center_hover_snaps.json) and
[raw observations](../tools/rhino_oracle/observations/center_hover_snaps.json) use
Rhino 8.32.26160.13001 in an owned private Xvfb session, never the user's desktop
or existing Rhino. Every case performs a real SplitEdge component pick and point
click, followed by actual Undo/Redo. Only public APIs and commands are used.

Four geometry families (standalone arc, polycurve arc, circle, ellipse) each have
curve-hover and empty-center clicks under both one-shot Cen and persistent
Point/End/Mid/Cen/Quad. Three further cases exercise same-object End/Mid/Quad
overlap, and three place other point objects at increasing cursor distances.

At each actual point prompt the harness records the public
[`GetTransform(World, Screen)` matrix](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Display_RhinoViewport_GetTransform.htm),
camera pose, viewport size, world aim, `WorldToClient` result and integer click
pixel. Matrix projection must agree with `WorldToClient` within `1e-7` pixels.
The persistent modes use the public
[`OsnapModes` enum](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/T_Rhino_ApplicationSettings_OsnapModes.htm);
original model-aid and tracking settings are restored even on command failure.

An initial pre-command calibration was insufficient: Rhino adjusted the viewport
when the point prompt opened. A strict equality guard detected this, but initially
left the command waiting after its timer stopped. That owned prompt was cancelled;
the final harness reprojects and records at the actual prompt. Fatal input errors
now request a fixed Escape only in the worker's verified owned window and suppress
further clicks. Python tests simulate prompt-time camera changes and cancellation
without a user desktop. Only the final prompt-time calibrated run is archived here.

Native replay applies the recorded matrix/camera to every source object and queries
the actual integer click with Rhino's configured 12-pixel capture radius:

- Fourteen captures feed the independently computed **native** snap point into
  the command adapter. Complete ordered geometry, attributes, selection and
  before/after/Undo/Redo states match at absolute epsilon `1e-9`, relative `1e-10`.
  Only Rhino-only event transcripts and camera metadata lack command equivalents.
- Eight empty-center misses check native admission only. Rhino's resulting
  unsnapped screen-to-edge parameters are retained but not replayed or counted
  as geometry matches.
- Two original nearby-point hypotheses remain unchanged in the requests
  (`circle-all-near-point`, `circle-all-other-point`). Both engines actually
  capture the circle Center. The dedicated calibrated test uses native capture,
  not the incorrect declared target or Rhino's output, to drive command replay.

The generic model-target adapter still replays `pick.point`; it does not itself
reconstruct the recorded camera. The dedicated Rust calibration test supplies
that additional step. Earlier uncalibrated snap archives and provenance remain
unchanged, including their explicit discrepancies.

## Implementation and limits

`object_snap/centers` shares the capture query between ordinary drafting and
edge-constrained point prompts. `ObjectSnap::distance()` now means capture distance:
distance to a direct feature or to Center's source curve, not always to its target.
`ObjectSnapModes` enables explicit feature sets in cached axis-aligned/projected
queries, skipping expensive disabled midpoint queries. The later
[snap controls](object-snap-controls.md) expose per-mode selection and scoped
one-shot overrides in ordinary and edge-constrained prompts.
The [Mid-only hover follow-up](mid-hover-snaps.md) extracts shared numerical
proximity and projected bounds into `object_snap/proximity`; Center keeps its own
target/priority rules.
The [polygon Center follow-up](polygon-center-snaps.md) adds cached corner-average
targets for closed linear boundaries and polygonal planar surfaces/faces without
holes, using the same capture-distance and per-object priority rules.

A conservatively expanded model-space cube provides a cheap projected bounding
box rejection. Overflowing or unprojectable corners disable this rejection.
This assumes affine/projective projection and consistent camera/clipping-plane
rejection, not an arbitrary nonlinear callback. Actual curve points, never
bridging projected chords, determine admission. The bounded search samples 64
angular intervals and refines the eight best intervals by up to 72 golden-section
steps, stopping earlier at floating-point stagnation. A high-zoom regression
exposed insufficient refinement at 40 steps; tests now include scales through
`1e12` pixels/model unit with an on-curve residual below `0.01` pixel.

This is a numerical UI query, **not a certified global closest-point solver**.
Extremely narrow visible slivers at camera-plane crossings may be missed, and
arbitrary scale/translation accuracy is not established. General rational-conic
and curved-boundary Center recognition, B-rep conic-edge Center, occlusion,
CPlane-relative Quad and broader priority/camera parity remain unfinished.

Independent tests cover analytic distance witnesses, arc-sweep exclusion,
per-object/across-object priority, mode filtering, large-origin relative queries,
partial visibility, broad-phase rejection and the four production viewport kinds.
A real egui pointer press/release checks that an off-cursor Center is returned
without accidentally selecting an object or edge. The ignored
`center_hover_scene_timing` test measures separated-circle query cost; it is a
local diagnostic, not a Rhino performance comparison or scene spatial index.
One release run on the local GB10 system measured approximately 9, 12 and 39
microseconds per query for 1, 100 and 1,000 separated circles respectively
(100 queries per scene after warmup). Overlapping conics and arbitrary geometry
can cost substantially more; no frame-rate or cross-engine guarantee is implied.

Verification checkpoint: 3,011 release-mode workspace tests, 274 Python tests,
seven offscreen GPU tests, formatting, and Clippy/Rustdoc with warnings denied.

```sh
cargo test --release -p viboceros-drafting
cargo test --release -p viboceros-oracle calibrated_center_hover
python3 -m unittest tools.rhino_oracle.test_center_hover_snaps tools.rhino_oracle.test_viewport_capture
cargo test --release -p viboceros-drafting center_hover_scene_timing -- --ignored --nocapture
```
