# Polycurve and surface feature snaps

[Drafting controls](interface.md) · [Previous snap audit](split-edge-snaps.md) · [Provenance and hashes](composite-feature-snaps-provenance.json)

Shared feature enumeration now supplies End/Mid on native polycurve leaves:
lines, arcs, polylines and NURBS. Mid is per segment, not halfway along the whole
composite. A polyline leaf supplies its interior vertices and individual segment
midpoints. NURBS Mid is at half arc length, independent of both its native domain
and the polycurve's outer parameter intervals.

Untrimmed NURBS surfaces supply Mid on each natural boundary. The former synthetic
UV-center Mid was incorrect and is removed, along with its incorrect test
expectation. Boundary isocurves are evaluated at the natural domain endpoints;
copying control rows would be wrong for unclamped/periodic surfaces. A failed or
collapsed boundary does not suppress other features. Surface centers are not
silently relabeled as Cen: general center recognition is separate unfinished work.

## Evidence

The [12 feature requests](../tools/rhino_oracle/fixtures/composite_feature_snaps.json)
and [raw results](../tools/rhino_oracle/observations/composite_feature_snaps.json)
use actual SplitEdge commands in owned private Xvfb sessions on Rhino
8.32.26160.13001. Each includes a real component pick, a five-pixel-offset feature
click, unchanged external targets and actual Undo/Redo. Only public APIs and
commands were used.

Eleven End/Mid target locations match complete native command replay at absolute
epsilon `1e-9` and relative epsilon `1e-10`. All ordered geometry, attributes,
selection and before/after/Undo/Redo states are compared; only Rhino-only event
transcripts lack native equivalents. Native feature queries separately capture
each declared target with its owning object ID. These are model-target and
feature tests, not camera/pixel equivalence tests.

One intended Cen click at the empty center of a mixed arc/line polycurve did not
capture: the raw resulting split x is `2.776693248934896`, while replaying the
declared model target yields x=4. This remains an explicit discrepancy, not a
twelfth passing comparison.

The [five Center follow-up requests](../tools/rhino_oracle/fixtures/center_capture_diagnostics.json)
and [raw results](../tools/rhino_oracle/observations/center_capture_diagnostics.json)
clarify that discrepancy. Both standalone and composite arcs capture their center
when the cursor is near the arc at (2.4,-2.8,0); clicks at the empty center (4,-4,0)
miss in these cases, including zero-offset clicks. Thus polycurve arc centers
are available in Rhino, but capture depends on curve proximity. No camera
calibration was retained, so the diagnostics are not counted as native pixel
replays. Output positions are never substituted into the request targets.

## Architecture and verification

`object_snap/features` shares cheap leaf enumeration without allocating converted
curves. `ObjectSnapCache` integrates only NURBS leaves and retains just their
spatial curves; analytic-only composites create no cache entries. Changes to
outer parameter intervals alone do not invalidate geometric midpoints. Exact
ordered leaf comparisons include length, preventing a stale prefix match when
leaves are added or removed.

Standalone surfaces cache boundary midpoints with a source surface/tolerance
snapshot, avoiding repeated boundary extraction and integration. Geometry edits,
Undo and tolerance changes invalidate entries; deletion/type conversion evicts
them on the next query. Hidden objects cannot supply cached snaps. B-rep entries
continue to retain only edge curves, not face surfaces/trims. Cold queries still
process all visible eligible curves, and hot queries compare source geometry;
these caches are not a scene spatial index or a cross-engine performance claim.

Independent tests distinguish parameter/arc midpoints, unclamped boundary/control
rows, per-segment/whole-curve Mid, and a projectively reparameterized rational
quarter-circle midpoint against its analytic circle bisector. Additional checks
cover collapsed boundaries, type conversion, edits/Undo, and cache reuse.
The production constrained viewport test also captures nonuniform polycurve and
surface Mid through its shared camera-space query.

Verification checkpoint: 2,999 release-mode workspace tests, 266 Python tests,
seven offscreen GPU tests, formatting, and Clippy/Rustdoc with warnings denied.

## Remaining limits

Native standalone Cen is still based on proximity to the center point, not the
hover-aware curve discovery demonstrated above; composite Cen is not implemented.
One-shot mode semantics, general conic/closed-boundary recognition, occlusion,
CPlane-relative Quad, mesh features and arbitrary camera equivalence remain
incomplete. No broad Rhino snapping parity follows from these fixtures.
See [Rhino's object-snap reference](https://docs.mcneel.com/rhino/8/help/en-us/user_interface/object_snaps.htm)
for the intended interface, including segment/edge Mid and one-shot behavior.
