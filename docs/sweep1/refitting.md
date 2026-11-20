# Sweep1 section Refit

Choose `Simplify` then `Refit` during review. `RefitTolerance` edits the saved
distance, initially `0.01`. A script can use `Simplify=Refit RefitTolerance=0.01`
or `Refit=0.01`. Refit accepts positive finite distances. The preview follows
mode and tolerance changes, and Undo in review restores the prior draft.
`Simplify=None` turns simplification off while retaining the distance.

Section Refit supports exact straight and polygonal profiles, full circles that
fit with four cubic spans, and regular open Bézier sections. It supports
adaptive pieces for smooth open profiles, including the captured rational
quadratics, and shared subdivision for multiple smooth profiles. Placement
uses the original profile. Fitting follows seam and direction edits. Open
fits requiring midpoints beyond the measured eight-interval length lookup and
tight circular fits remain open.

The oracle accepts `refit_tolerance` independently of rail refitting and
rejects simultaneous section rebuilding. Across 60 captures, the complete
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
A lone open profile gets a second fitting pass. Each pass uses the source
length stations to select cubic pieces, rescales adjacent intervals to match
join derivatives, and checks the assembled curve against its source. The
section domain uses a Cash–Karp length estimate and retains the source length
for small relative changes.

In the two-profile adaptive captures, Rhino interpolates the retimed curves
and their derivatives on a shared dyadic grid. Rebuilding each profile on
its own grid and inserting the other profile's knots changes the geometry.
The implementation accepts an interval with a proven pointwise bound and
subdivides when any profile has a point proven to exceed the distance limit.
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
