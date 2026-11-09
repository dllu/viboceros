# Two-surface matching

`MatchSrf Mode=Position|Tangency AverageSurfaces=Yes RefineMatch=No` adjusts both
picked surfaces. A disposable preview leaves the document and pending Redo
unchanged. Enter accepts both replacements in one Undo entry; Escape cancels.
Both UUIDs, names, layers, colors and groups survive. The two edited objects move
to the end of object order, source first. The averaging preference survives
cancellation; `AverageSurfaces=No` returns to one-sided matching.

The portable `try_average_match_nurbs_surfaces` and native
`viboceros_smlib::average_match_surface_supports` APIs return two owned NURBS
supports. They share clamping, degree preparation and interpolation adapters
with one-sided matching. Each result moves halfway from its prepared original
to an independent one-sided match in the same rational basis.

The SDK-style `SurfaceAverageOptions` retains separate traversal flags:
`matching.reverse_match` for the first edit and `reverse_target` for the second.
Inconsistent flags can give different boundary curves, as in the native SDK.
The command derives coherent flags from the original boundary picks.

Support currently covers open, untrimmed positive-weight supports, position and
tangency modes, all natural sides, iso policies, preservation and closest-point
contacts. Averaged curvature and refinement remain pending. Their native
observations are retained separately; no continuous continuity bounds or full
Rhino/performance parity are claimed.

[Finite validation](surface-average-validation.json) compares both surfaces,
degrees, counts, knots, surface grids, complete boundary-curve deviation,
topology and 3DM. It also checks physical viewport picks, both identities and
attributes, cancellation, option recall, object order and Undo/Redo.
