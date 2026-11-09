# Averaged refinement investigation

`MatchSrf AverageSurfaces=Yes RefineMatch=Yes` remains unsupported. The
[owned investigation](surface-average-refinement-investigation.json) retains
102 native cases, settings readbacks, composed public calls, inverse-parameter
queries, and the original failed launch. Every launch used isolated Xvfb.

A coupled three-pass prototype matches 25 of 30 pilot cases at the existing
`1e-9` geometry gate. Five polynomial cases still differ in geometry or knot
selection. Replacing direct correspondence with a blanket surface inverse
matches 24 cases. Both prototypes are rejected; their sources and failing
logs are frozen under [the proof directory](surface-average-refinement-proof/).
They are outside the production build.

On folded intermediate boundaries, Rhino's curve and surface inverse queries
can return different parameters for the same spatial point. This explains a
recorded tangent discrepancy but does not establish a general replacement
fitter. The stopping policy also remains unresolved: the scoped curvature
percentage sweep leaves native output definitions unchanged.

The catalog keeps four invalid composed calls distinct from valid results.
These diagnostics do not qualify continuous continuity, full Rhino parity,
or performance. [Unrefined averaging](surface-averaging.md) remains available.
