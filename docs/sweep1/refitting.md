# Sweep1 section Refit

Choose `Simplify` then `Refit` during review. `RefitTolerance` edits the saved
distance, initially `0.01`. A script can use `Simplify=Refit RefitTolerance=0.01`
or `Refit=0.01`. Refit accepts positive finite distances. The preview follows
mode and tolerance changes, and Undo in review restores the prior draft.
`Simplify=None` turns simplification off while retaining the distance.

Section Refit supports exact straight and polygonal profiles, full circles that
fit with four cubic spans, and regular polynomial Bézier sections when one cubic
is enough. It uses the original profile for rail placement and fits after seam
and direction edits. General adaptive subdivision, tight circular fits, and
smooth rational arcs remain open.

The oracle accepts `refit_tolerance` independently of rail refitting and
rejects simultaneous section rebuilding. Across 46 captures, the complete
surface deviation bound is at most `5.65e-9` at epsilon `1e-7`. The comparison
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
A lone open profile gets a second fitting pass. Equally spaced length stations
select cubic proposals and complete image bounds check the intervals between
them. The section domain uses a Cash–Karp length estimate and retains the
source length for small relative changes.

The final boundary can exceed the nominal tolerance. The
[exact Bernstein regression](../../tools/rhino_oracle/test_sweep1_refit.py)
retains a native 0.001 fit whose boundary is more than 0.0015 from a source point.
