# Two-surface matching

`MatchSrf Mode=Position|Tangency|Curvature AverageSurfaces=Yes RefineMatch=No` adjusts both
picked surfaces. A disposable preview leaves the document and pending Redo
unchanged. Enter accepts both replacements in one Undo entry; Escape cancels.
Both UUIDs, names, layers, colors and groups survive. The two edited objects move
to the end of object order, source first. The averaging preference survives
cancellation; `AverageSurfaces=No` returns to one-sided matching.

The portable `try_average_match_nurbs_surfaces` and native
`viboceros_smlib::average_match_surface_supports` APIs return two owned NURBS
supports. They share clamping, degree preparation and interpolation adapters
with one-sided matching. Position and tangency move halfway toward independent
one-sided matches. Curvature uses the source U-curve after moving its boundary
to the midpoint, averaged with target normal curvature. The mean tangent
direction stays unnormalized, retaining Rhino’s shortened handle and rational
second-row rule, including nonuniform cross knots.

The SDK-style `SurfaceAverageOptions` retains separate traversal flags:
`matching.reverse_match` for the first edit and `reverse_target` for the second.
Inconsistent flags can give different boundary curves, as in the native SDK.
The command derives coherent flags from the original boundary picks.

Support currently covers open, untrimmed positive-weight supports, position,
tangency and curvature modes, all natural sides, iso policies, preservation and
closest-point contacts. Averaged refinement remains pending. No continuous
continuity bounds or full Rhino/performance parity are claimed.

[G2 and regression validation](surface-average-curvature-validation.json) compares both surfaces,
degrees, counts, knots, surface grids, complete boundary-curve deviation,
topology and 3DM. It also checks physical viewport picks, both identities and
attributes, cancellation, option recall, object order and Undo/Redo.

The earlier [position/tangency record](surface-average-validation.json) remains
available with its original observations and pending-mode inventory.

[Refinement investigation](surface-average-refinement.md) records the remaining
Native mismatches and rejected prototypes.
