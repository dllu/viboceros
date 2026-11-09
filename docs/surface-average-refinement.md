# Averaged refinement investigation

`MatchSrf AverageSurfaces=Yes RefineMatch=Yes` remains unsupported. The
[owned investigation](surface-average-refinement-investigation.json) retains
102 native cases, settings readbacks, composed public calls, inverse-parameter
queries, and the original failed launch. The [adaptive fitting follow-up](surface-average-adaptive-investigation.json)
adds 38 SDK cases and five physical command workflows, bringing the discovery
inventory to 145 observations. Every launch used isolated Xvfb.

A coupled three-pass prototype matches 25 of 30 pilot cases at the existing
`1e-9` geometry gate. Five polynomial cases still differ in geometry or knot
selection. Replacing direct correspondence with a blanket surface inverse
matches 24 cases. Both prototypes are rejected; their sources and failing
logs are frozen under [the proof directory](surface-average-refinement-proof/).
They are outside the production build. A curvature sign policy improves the
pilot to 27/30, but remains rejected. The broader position prototype matches
30/37 cases; closest-point matching, several boundary charts, reversed traversal
and unequal transverse bases still fail. Their complete sources, inputs and
failed logs are retained under [the adaptive proof directory](surface-average-adaptive-proof/).

On folded intermediate boundaries, Rhino's curve and surface inverse queries
can return different parameters for the same spatial point. This explains a
recorded tangent discrepancy but does not establish a general replacement
fitter. The stopping policy also remains unresolved: the scoped curvature
percentage sweep leaves native output definitions unchanged. Native position
refinement can raise the transverse degree and insert dozens of knots; simply
repeating the original-basis averaging fit does not reproduce it.

Four new physical command acceptances refine both supports; cancellation and
option recall are captured separately. The SDK clamps a requested distance
tolerance of `1e-8` to `1.490116119385e-8`; both settings are retained.

The catalog keeps four invalid composed calls distinct from valid results.
These diagnostics do not qualify continuous continuity, full Rhino parity,
or performance. A regression test keeps all 122 refinement discovery cases
explicitly rejected before backend work, preserving both inputs.
[Unrefined averaging](surface-averaging.md) remains available.

Reproduce the follow-up catalog with
`python3 -m tools.rhino_oracle.report_surface_average_adaptive` and its checks
with `python3 -m unittest tools.rhino_oracle.test_surface_average_adaptive`.
