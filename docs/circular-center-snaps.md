# Circular NURBS and boundary Center snaps

[Snap controls](object-snap-controls.md) · [Analytic Center](center-hover-snaps.md) · [Polygon Center](polygon-center-snaps.md) · [Provenance](circular-center-snaps-provenance.json)

The [elliptical NURBS follow-up](elliptic-center-snaps.md) resolves the four
previously unsupported elliptic captures in this retained audit. Original inputs
and Rhino observations are unchanged; current replay has 38 full matches.
The [fresh conic audit](conic-center-audit.md) adds full-coordinate API and
calibrated ellipse-snap evidence, with a center-stability screen for short arcs.

Center now recognizes circular NURBS curves and arc leaves, natural surface
boundaries, and spatial B-rep edges. Capture follows the original curve: hovering
over an empty center or the missing part of a supporting circle does not capture.
Targets retain model-space elevation. Circular hole edges are eligible even
though the separate polygonal face-center rule excludes faces with inner loops.
Direct features still suppress Center on the same object; different objects use
capture distance.

## Recognition and cache

`NurbsCurve::circular_center` and `circular_radius` share the kernel's
[whole-span recognition](numerical-robustness.md#whole-span-circularity). A curvature
jet proposes a center/plane/radius, then every rational Bézier span must satisfy
absolute plane/radial coefficient bounds. This is a conservative floating-point
test, not an exact algebraic predicate or a sparse sample fit.

Candidate jets use a normalized span domain so tiny/huge native parameters cannot
overflow derivatives merely through parameter speed. A stationary endpoint can
fall back to a regular midpoint/end or later span. The whole-span test still checks
the original control geometry. Recognition does not replace the source curve,
invent its sweep, or turn an ellipse into a circle. Common negative homogeneous
gauges remain geometrically equivalent. Independent tests cover translated
origins, reversal, refinement, elevation, signed gauges, extreme intervals and
a stationary quartic parameterization, plus the existing adversarial degree-nine
perturbation invisible to three sampled second-order jets.

`ObjectSnapCache` now retains one source/bounds snapshot per NURBS leaf or edge.
Mid integration and circular-center recognition are independent lazy results,
including failures; using Cen alone does not integrate arc length. Geometry,
tolerance, Undo and type changes invalidate those results. Surface entries share
their extracted natural boundaries. Polycurve failures keep their own source
slots. Center bounds are checked before recognition; Mid preserves its existing
eight-projection distant-source rejection. Both use the same sided-span proximity
query, but only Mid can admit a direct target hit because its target lies on the
curve. Circular recognition in the UI is capped at degree 32; the kernel API has
no such UI cap. This does not bound total scene work or establish a frame budget.

## Retained Rhino evidence

The [44 requests](../tools/rhino_oracle/fixtures/circular_center_snaps.json) and
[raw results](../tools/rhino_oracle/observations/circular_center_snaps.json) come from
Rhino 8.32.26160.13001 in a private owned Xvfb. Each family uses both one-shot Cen
over five persistent modes and persistent Cen alone. Public point-prompt camera
matrices agree with `WorldToClient` within `1e-7` pixels; native queries use the
actual integer click and a 12-pixel capture radius.

- 34 native captures drive complete ordered geometry, attributes, selection and
  Undo/Redo replay at absolute epsilon `1e-9`, relative `1e-10`. Cases include
  circles/arcs, reversal, elevation, refinement, positive weight gauges, translated
  domains, domains of width `1e-170` and `1e170`, polycurve leaves, off-plane curves,
  extruded boundaries, disks and both annulus boundaries.
- Four empty-center/outside-arc misses compare admission only. Both outside-arc
  clicks fail to split, so those records have no Undo/Redo. The other 42 do.
- Four elliptical cases originally captured only in Rhino; the subsequent
  [elliptical recognizer](elliptic-center-snaps.md) now supplies their native
  targets and full command/history replay, bringing the total to 38. The
  altered quadratic (`noncircle`) captures at `x=3.75`; its original declared
  hypothesis `x=4` stays in the request. The full ellipse captures at `x=4`.
- Two negative-weight arc cases capture natively but miss in Rhino. The same
  curve with positive weights captures in both. The native geometry-invariant
  recognition is retained, and these differences are not counted as matches.

These are constrained SplitEdge observations, not direct unconstrained GetPoint
results. Projected coincident boundaries do not prove depth/occlusion ordering.
Native tests additionally exercise all four production viewport kinds, ordinary
and constrained queries, shared lazy data, invalidation and an actual off-plane
egui point click. Unrestricted elliptic NURBS recognition, Rhino's approximate-conic
option, high-degree UI completeness, occlusion and unrestricted camera/priority
parity remain unfinished. Analytic ellipse objects retain their existing Center
support; conservative NURBS ellipse recognition is now covered by the follow-up.

One release timing run on the local GB10 measured about `0.65`, `0.67` and
`1.19` ms per cached query for 1, 100 and 1,000 separated circular NURBS objects
(100 queries after a cold query). This measures the drafting query, not drawing,
application FPS or a comparison with Rhino. Overlapping curves and more complex
boundaries can cost substantially more; no global spatial index is claimed.

Original circular-implementation checkpoint: 3,064 release-mode workspace tests, 282 Python tests,
seven offscreen GPU tests, formatting, and Clippy/Rustdoc with warnings denied.

```sh
cargo test --release -p viboceros-geometry nurbs::circularity
cargo test --release -p viboceros-drafting circular
cargo test --release -p viboceros-oracle conic_nurbs_and_edges
python3 -m unittest tools.rhino_oracle.test_circular_center_snaps
cargo test --release -p viboceros-drafting circular_nurbs_hover_scene_timing -- --ignored --nocapture
```
