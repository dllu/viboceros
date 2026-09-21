# Polygon and planar-face Center snaps

[Snap controls](object-snap-controls.md) · [Analytic Center hover](center-hover-snaps.md) · [Provenance and hashes](polygon-center-snaps-provenance.json)

Center now recognizes closed polylines, straight-span NURBS and linear polycurve
boundaries, plus polygonal boundaries of planar surfaces and hole-free B-rep
faces. Hover near the boundary to capture the corner average; hovering over its
empty center or a surface's interior is insufficient. The target can be off the
construction plane. Enabled direct features still take precedence on the same
object, while different objects compete by capture distance.

The target is the arithmetic mean of corner occurrences, excluding the duplicate
closing endpoint, **not** the area centroid or bounding-box center. Stored
collinear and non-adjacent repeated polyline vertices count. A straight surface
edge contributes its endpoints, not knots subdividing its parameterization.
Singular sides are not extra corners. Reversing winding or moving the seam between
existing corners does not change the average.

[Rhino's Center description](https://docs.mcneel.com/rhino/8/help/en-us/user_interface/object_snaps.htm)
mentions planar polylines, but the retained nonplanar curve picks also capture
their corner averages. Native behavior follows those observations. Surface/face
recognition still requires planarity at the document's absolute tolerance;
the warped-surface control does not capture. Inner loops exclude face Center.

## Evidence

The [44 requests](../tools/rhino_oracle/fixtures/polygon_center_snaps.json) and
[raw observations](../tools/rhino_oracle/observations/polygon_center_snaps.json)
use Rhino 8.32.26160.13001 in an owned private Xvfb. They perform real SplitEdge
component/point clicks and actual Undo/Redo. Each family tests both one-shot Cen
over all five persistent modes and persistent Cen alone. Point-prompt matrices
are checked against the public WorldToClient result within `1e-7` pixels.

Thirty-four captured native targets drive complete ordered geometry, attributes,
selection and history replay at absolute epsilon `1e-9`, relative `1e-10`.
Ten open, nearly closed, empty-center, surface-interior and warped-surface misses
check admission only; unsnapped screen-to-edge results are not geometry matches.
The requests retain two incorrect subdivided-surface hypotheses (`x=4.5`): the
four boundary corners actually average to `x=4.75`. Replay computes native targets
from geometry rather than feeding recorded result positions back into commands.

SplitEdge constrains the captured point to its selected edge. Separate coordinate
permutations of the nonplanar polygon constrain all three average components
through the resulting x coordinate. These are not direct unconstrained GetPoint
observations. Hole exclusion and singular-side handling have independent native
tests; those cases are not part of the retained Rhino click matrix.

## Implementation and limits

`object_snap/polygon_centers` owns recognition, exact corner averaging and capture.
`FiniteSum` accumulates each coordinate exactly before one final mean rounding,
including when an intermediate sum would overflow. The camera-independent cache
retains failed recognitions too. Its source snapshots include curve/surface data
and relevant face boundary curves/orientations, not B-rep UV trims or attributes.
Geometry changes, tolerance edits and Undo invalidate entries; deleted or
ineligible object types are evicted. Hidden geometry cannot supply cached snaps.

Common-sign degree-one spans are sampled on their exact knot sides. Higher-degree
curves require every extracted Bézier span to pass the kernel's zero-tolerance
linearity predicate. Adjacent parts must meet exactly; no bridging segment is
invented across a gap. Mixed-sign weights are excluded rather than assuming a
bounded line locus across a possible pole. Projected bounding boxes reject distant
targets before boundary proximity tests. Cold recognition and hot source comparisons
still scale with scene geometry; this is not a scene spatial index or a fixed frame
budget. Curved/conic NURBS recognition, toleranced gap recovery, occlusion, arbitrary
camera/priority equivalence and cross-engine performance remain incomplete.

Tests cover representation variants, failed/gapped boundaries, exact mean range,
cache reuse/edit/Undo/tolerance/conversion, visibility, planarity far from world
zero, four production viewport kinds and a real off-CPlane pointer pick.

## Surface-construction corrections exposed by this work

The subdivided-surface replay exposed unit parameter intervals on native natural
UV trim curves. These now retain the matching U or V interval, independently of
boundary direction and the spatial edge's local parameter frame. This agrees with
the open-source `ON_Brep::NewOuterLoop` construction in
[OpenNURBS](../third_party/opennurbs/opennurbs_brep_tools.cpp). UV control coordinates,
surface domains and stored edge geometry are not normalized away in comparisons.
Older shared-input `surface_face` recipes deliberately retain their original
unit-domain trim definition in the oracle adapter; optional explicit `trim_domains`
can define other intervals before edge splitting. Thus their existing raw
before/after/history records compare against unchanged inputs, independently of
the corrected kernel constructor. No archived output fields or hashes are changed.

A valid irregular polygon with a rectangular hole was rejected because arbitrary
axis divisors turned collinear trim samples into a tiny spurious triangle during
constrained triangulation. That stage now uses independent power-of-two axis scales.
It retains the existing positive-area, constraints and total-area checks; no
tolerance was loosened. Other topology predicates retain their existing unit-range
normalization. This removes the measured normalization artifact, not all possible
translation/subnormal rounding or near-degenerate triangulation limitations.
Regression tests check both boundary windings, area `20`, tessellated area,
non-unit/translated trim domains, seams, singular sides and large UV frames.

Verification checkpoint: 3,055 release-mode workspace tests, 280 Python tests,
seven offscreen GPU tests, formatting, and Clippy/Rustdoc with warnings denied.

```sh
cargo test --release -p viboceros-drafting polygon
cargo test --release -p viboceros-geometry brep::rectangular_surface
cargo test --release -p viboceros-oracle calibrated_polygon_centers
python3 -m unittest tools.rhino_oracle.test_polygon_center_snaps
```
