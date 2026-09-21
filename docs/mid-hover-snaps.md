# Mid-only curve hover

[Snap controls](object-snap-controls.md) · [Composite features](composite-feature-snaps.md) · [Provenance and hashes](mid-hover-snaps-provenance.json)

With Mid as the only effective mode, hovering near a curve or segment captures
its midpoint even when that point is far from the cursor. This includes one-shot
Mid, which temporarily replaces the persistent mode set. With multiple modes
enabled, Mid still requires proximity to the midpoint itself. This distinction
follows [Rhino's Mid documentation](https://docs.mcneel.com/rhino/8/help/en-us/user_interface/object_snaps.htm)
and the retained clicks below.

The target is halfway along the segment's **arc length**, not its parameter
interval. Polyline/polycurve segments, natural surface boundaries and spatial
B-rep edges each supply their own Mid. Analytic circles and ellipses now supply
Mid opposite their stored seam, including in mixed-mode direct-feature queries.
Competing Mid-only candidates rank by cursor-to-curve distance, not distance to
their target points. Hidden objects are excluded; locked objects remain eligible.

## Calibrated evidence

The [37 requests](../tools/rhino_oracle/fixtures/mid_hover_snaps.json) and
[raw observations](../tools/rhino_oracle/observations/mid_hover_snaps.json) use
Rhino 8.32.26160.13001 in an owned private Xvfb. Each records real SplitEdge
component and point clicks using public APIs/commands. Point-prompt camera
matrices, aims and integer pixels follow the [Center calibration protocol](center-hover-snaps.md).

Nine families each exercise one-shot Mid, persistent Mid-only and Point+Mid:
line, nonuniform NURBS, arc, polyline, NURBS/arc polycurve leaves, natural surface
boundary, B-rep edge and rational quarter-circle. Eight conic cases cover isolated
hover, mixed-mode hover and direct target capture; two competition cases distinguish
curve-distance selection from midpoint-distance selection across objects/segments.

- Twenty-six native captures, computed from the recorded camera and 12-pixel
  capture radius, drive complete ordered geometry, attributes, selection and
  history replay at absolute epsilon `1e-9`, relative `1e-10`.
- Eleven mixed-mode misses compare admission only. Their unsnapped screen-to-edge
  geometry is not claimed to match. Ten performed actual Undo/Redo; the retained
  `rational-mixed` control failed to split and has no Undo/Redo observations.
- Two all-mode conic targets coincide with Quad. Their geometry establishes the
  captured position but cannot distinguish Rhino's displayed Mid/Quad label.

Native output points, not Rhino result positions, feed command replay. A preliminary
29-case diagnostic had an incorrect ellipse target hypothesis (`x=2`); it was
corrected analytically to `center.x - radius_x = 1` before the final fresh run.
Only the final 37-case response is retained as the replay fixture.

## Implementation and limits

`object_snap/mid_hover` handles admission separately from cached arc-length target
construction. `object_snap/proximity` shares projected bounds and bounded curve
refinement with Center. Ordinary and edge-constrained prompts use the same query.
`ObjectSnap::distance()` is capture distance, which need not be distance to the
returned model point. The target itself must remain projectable.

Each cached NURBS keeps its source, optional midpoint and common-sign control
bounds together, including failed midpoint slots. Surface entries retain their
actual extracted boundary curves. Geometry/tolerance changes and Undo invalidate
these data; deletion and conversion evict stale entries. Mixed-sign rational
weights disable the convex-hull broad phase. Analytic leaves need no NURBS integration;
empty NURBS-leaf discovery in composites is cached as well.

Visible lines and common-sign degree-one NURBS spans use projected segment
distance. Curved proximity samples 64 intervals and refines eight by up to 72
golden-section steps, stopping at floating-point stagnation. Each NURBS knot span
is evaluated separately in normalized, sided coordinates: neither extreme native
domains nor a discontinuous knot jump creates a bridging capture chord. Overflowing
or unprojectable bound corners disable culling. Projection callbacks must preserve
convexity in their visible half-space (affine/projective viewport transforms).

This is a numerical UI search, **not a certified global closest-point solver**.
Highly oscillatory spans and arbitrarily narrow visible slivers may be missed;
unrestricted scale/translation accuracy is not established. Mid-only queries reject
distant control bounds before cold integration. [Shared snapshots](snap-caching.md)
avoid source comparisons on unchanged objects. Work still scales with
scene/span complexity, not a fixed frame budget. Occlusion, broader priority rules,
Near/Int/Tan/Perp, general conic recognition and universal Rhino camera parity remain
unfinished. No cross-engine performance claim follows from these observations.

Independent tests cover extreme parameter domains, large-origin relative queries,
failed cache slots, edits/Undo, competing segments, discontinuities and invisible
targets. Production tests cover all four viewport kinds and real egui pointer
press/release returning an off-cursor, off-CPlane Mid without selecting geometry.

The ignored `mid_hover_scene_timing` diagnostic measures 100 queries after warmup
with an identity XY projection and separated objects. One local release run gave
these approximate microseconds per query:

| Geometry | 1 object | 100 objects | 1,000 objects |
| --- | ---: | ---: | ---: |
| Line | 0.04 | 1.6 | 16 |
| Quadratic NURBS | 33 | 38 | 271 |
| Box B-rep (12 edges) | 0.28 | 23 | 438 |

These are local query diagnostics, not rendering frame times or a Rhino comparison.
Overlapping curved spans can cost substantially more than these separated scenes.

Verification checkpoint: 3,038 release-mode workspace tests, 278 Python tests,
seven offscreen GPU tests, formatting, and Clippy/Rustdoc with warnings denied.

```sh
cargo test --release -p viboceros-drafting
cargo test --release -p viboceros-oracle calibrated_mid_hover
cargo test --release -p viboceros viewport::drafting
python3 -m unittest tools.rhino_oracle.test_mid_hover_snaps
cargo test --release -p viboceros-drafting mid_hover_scene_timing -- --ignored --nocapture
```
