# Object-snap cache performance

[Viewport controls](interface.md) · [Display snapshots](viewport-caching.md)

`ObjectSnapCache` shares immutable document geometry snapshots with the display
cache and history. Unchanged sources pass a constant-time identity check instead
of comparing every control point, surface, or face boundary on each mouse move.
After a snapshot changes, the existing feature-specific comparison runs once:
outer polycurve parameter maps do not change leaf Mid targets, and B-rep UV trim
parameterization does not change polygon corners. Successful reuse updates the
snapshot stamp; a relevant change or tolerance edit rebuilds that object's data.
Undo and edits in cloned documents follow the same path. Failed recognitions and
empty NURBS-leaf discovery are cached too.

The retained snapshot owns the complete original geometry, including B-rep trims,
but does not copy that payload. NURBS leaf/edge feature data still owns its curve
copies; surface boundaries are extracted once. Old geometry can remain alive
while an invisible object's cache or suspended snapping retains it. Deleted and
ineligible objects are pruned on the next enabled query.

Pruning builds one temporary index of eligible document objects, replacing the
previous linear document search for each cached ID. Its expected work is
O(objects + cache entries), rather than O(objects × cache entries). A visible-layer
set also avoids searching every layer for each object. Hash tables are used only
for lookup: candidate traversal and exact-distance ties retain document order.
Locked objects remain snappable. Suspended modes still return before any indexing.

Direct feature enumeration respects the enabled modes before evaluation. In
particular, Center-only queries do not enumerate polyline End/Mid points, evaluate
NURBS endpoints or surface corners, or walk B-rep vertices. Mid-only queries reject
distant common-sign control bounds before arc-length integration. Mixed-sign
curves have no such convex-hull bound and retain their existing full query path.
The geometry algorithms, projected admission, and priority policy are unchanged.

## Measurements and limits

The opt-in release benchmark uses Center-only misses with one visible layer and
20 warm queries over separated degree-one NURBS objects. On the development host,
the same fixture before this change (`da5f418` plus the new benchmark) and afterward
measured:

| Objects | Before, per query | After, per query |
| ---: | ---: | ---: |
| 100 | 0.026 ms | 0.028 ms |
| 1,000 | 1.183 ms | 0.425 ms |
| 10,000 | 59.830 ms | 3.338 ms |

A separate 100-query benchmark over one 100,000-vertex **open** polyline measures
a cached polygon-recognition miss: 109.832 µs before, 0.116 µs after. This isolates
disabled-feature work and repeated source comparisons, not boundary hit testing.
The existing circular-NURBS hover benchmark, with a real Center capture, measured
0.655, 0.665 and 0.807 ms/query for 1, 100 and 1,000 sources after this change.

These local CPU timings exclude construction, cold recognition, application
layout, GPU submission, and presentation. They are not viewport FPS, a frame
budget, or a Rhino speed comparison. Hash-index allocation has overhead for small
scenes, as the 100-object result illustrates. Object-cache lookups still use ordered
maps, enabled features and proximity queries still cost work, and there is no
scene-wide spatial index. Mixed-mode Mid queries may still initialize distant
sources; this change does not establish a general cold-query budget.

Regression tests check source-comparison counters, equal replacement refresh,
clone/style reuse, removal/type changes under Point-only queries, layer visibility,
locked snapping, object-order ties and Undo, and every feature-mode subset.
Existing calibrated Rhino snapping replays remain unchanged; no new Rhino run is
used as evidence for this cache-only change.

Verification checkpoint: 3,093 release-mode workspace tests passed (28 opt-in
exclusions). All five explicit drafting diagnostics, seven GPU tests, 290 Python
tests, formatting, and warnings-denied Clippy/Rustdoc checks also passed.

```sh
cargo test --release -p viboceros-drafting
cargo test --release -p viboceros-drafting cache::performance_tests -- --ignored --nocapture --test-threads=1
cargo test --release -p viboceros-drafting circular_nurbs_hover_scene_timing -- --ignored --nocapture
```
