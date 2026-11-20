# Sweep1 section Refit

Choose `Simplify` then `Refit` during review. `RefitTolerance` edits the saved
distance, initially `0.01`. A script can use `Simplify=Refit RefitTolerance=0.01`
or `Refit=0.01`. Refit accepts positive finite distances. The preview follows
mode and tolerance changes, and Undo in review restores the prior draft.
`Simplify=None` turns simplification off while retaining the distance.

Section Refit supports straight and polygonal profiles and smooth open and
closed profiles. A polygon can share a sweep with cubic loops, ellipses, or
circles. The selected corners remain available as face boundaries after
refitting. Placement uses the original profile. Fitting follows seam and
direction edits.

For a radius-2 circle, the measured adaptive range reaches tolerance 0.00015
for one profile and 0.0004 for a sweep with two profiles. Tighter circular
subdivision and open fits requiring midpoints beyond the measured
eight-interval lookup remain unresolved.

The oracle accepts `refit_tolerance` independently of rail refitting and
rejects simultaneous section rebuilding. Across 116 captures, the complete
surface deviation bound is at most `1.5e-8` at epsilon `1e-7`. The comparison
reverses the Rhino rail parameter for the two-pole capture. The public
`Curve.Fit` diagnostics produce different controls. SDK captures preserve the
original section at distances up to twice the square root of machine epsilon,
including equality.

The [controlled inputs](../../tools/rhino_oracle/fixtures/sweep1_refit_common_handles.json)
and [Rhino outputs](../../tools/rhino_oracle/observations/sweep1_refit_common_handles.json)
isolate endpoint-handle fitting. Six cubic profiles share endpoints and tangent
directions, with common handle scales from 0.9 to 1.01. Sixteen SDK recipes use
one profile or two translated copies, at fitting tolerances 0.001 and 0.01.
Producer sources are frozen in the capture's content-addressed archive.

The same profile can produce different fitted boundaries when supplied alone
or as part of a pair. At scale 0.99 and tolerance 0.01, the single-profile output
retains the initial endpoint-frame cubic, while the pair uses adjusted handles.
A lone profile can get a second fitting pass. Each pass uses the source
length stations to select cubic pieces, rescales adjacent intervals to match
join derivatives, and checks the assembled curve against its source. The
section domain uses a Cash–Karp length estimate and retains the source length
for small relative changes. An analytic circle keeps its exact circumference
as the section domain. The [closed-profile captures](../../tools/rhino_oracle/fixtures/sweep1_refit_closed.json)
cover tight fits of analytic and rational circles, the shared grid for unequal
circles, and a cubic loop that retains a linear rail when RefitRail is off.

Closed profiles with retained curvature boundaries can match regions by
fitted length before sharing a knot vector. Cornered sections can take part
in that matching, with the polygon's controls staying on its straight
edges. Polygon and circle pairs combine their knots after mapping each
profile into the longest source's section domain. The
[polygon captures](../../tools/rhino_oracle/fixtures/sweep1_refit_polygon_profiles.json)
cover both policies, mixed straight and curved sections, and the faces
produced by the Rhino command. Captures mixing open and closed profiles use
the Rhino SDK. The
[point-end captures](../../tools/rhino_oracle/fixtures/sweep1_refit_polygon_poles.json)
check that the matched section domain survives collapsing one or both rail
ends when the sweep has multiple real profiles.

Smooth sets that mix noncircular loops with circles or open profiles can
also use the longest source's section domain. The
[loop captures](../../tools/rhino_oracle/fixtures/sweep1_refit_loop_pairs.json)
include scaled profiles, unequal region counts, and a three-section sweep.
The [mixed-profile captures](../../tools/rhino_oracle/fixtures/sweep1_refit_mixed_sections.json)
cover open sections paired with loops and closed sections without retained
curvature boundaries.

In the shared-grid cases, Rhino interpolates the retimed curves and their
derivatives on common dyadic intervals. The implementation accepts an
interval with a proven pointwise bound and subdivides when any profile has a
point proven to exceed the distance limit. Closed noncircular sets can also
accept a complete image bound that allows different parameter speeds.
If neither test decides an interval, review shows a fitting error and the
tolerance stays editable. The native interval acceptance rule still needs
qualification in that gap. For example, at scale 10 the native grid splits
at `0.1875` with tolerance `0.001` while the unsplit interval has a geometric
deviation of about `0.000474` from the retimed curve. The cut disappears near
`0.001083516`. Rotation, translation, rail length, and model tolerance
changes preserved that transition in the measured cases.

The final boundary can exceed the nominal tolerance. The
[exact Bernstein regression](../../tools/rhino_oracle/test_sweep1_refit.py)
retains a native 0.001 fit whose boundary is more than 0.0015 from a source point.
