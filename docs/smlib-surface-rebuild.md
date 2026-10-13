# Native surface Rebuild

Linux `native-smlib` builds use usd-brep for surface Rebuild commands, readonly
previews and the `surface_rebuild_geometry` oracle. The bridge exports
`rebuild_surface`. The shared geometry callback
`try_rebuild_nurbs_surface_with_operations` retains midpoint-isocurve arc-length
stations, uniform knots, degree/count policy, local coordinate conditioning,
polynomial weights, exact corners and poles, and periodic control repetition.
Command trim reconstruction, attributes, selection, source validation and history
use their existing paths.

Station preparation and boundary pole queries use the checked native isocurve
adapter. Each tensor direction then submits one `UniformInterpolationSystem`
with four homogeneous channels per row. The same native adapter used by curve
Rebuild evaluates the basis, factors once for all rows, and solves the complete
batch. Open and uniform periodic systems use native banded LU; periodic rows and
columns keep the existing cycle ordering and restore the original control order.
Exact span and normalized residual checks apply to each channel.

Native commands no longer build dense tensor basis matrices or use the faer
interpolation solver. Portable builds retain that producer through the same
preparation and result policy. Sampling the original surface and measuring
isocurve arc length still use the shared geometry layer.

Exactly constant target channels are retained after each tensor pass. Partition
of unity makes this an exact interpolation constraint. The previous solves
could perturb a constant row before the other direction amplified its roundoff
in a high-degree, small-count system. This correction is shared by both backends;
it does not quantize or snap nonconstant channels. Exact original corners and
constant source boundary poles retain their existing direct payload assignment.

Native regressions compare retained Rhino SDK nets for open, closed, swapped and
reversed sources, and exercise all command degrees 1–11 on bilinear surfaces,
spheres, cylinders, cones and tori. Independent bilinear Greville controls cover
the 256×256 resource boundary. Tiny-channel/common-gauge regressions include
coordinates down to `1e-280` and positive weight gauges `1e-300` through `1e300`.
Shared-policy tests verify the two batch layouts, source purity, weights,
validation before entry and rejection of inconsistent solver output.

The [retained comparison](smlib-surface-rebuild-comparison.json) contains 137
full records against the saved pre-change surface producer. All match at
absolute `1e-9`, relative `1e-12`; maximum numeric difference is `1.23e-10`.
All 12 retained Rhino SDK API records match at the original absolute `1e-6`,
relative `1e-10` qualification. Native bridge tests also compare 40 retained
SDK control nets, preserving complete bases, weights and periodic flags, with
17×17 parameter grids.
The same binary replays 365 retained refinement/extraction/seam records with
364 complete raw matches. The existing whole-surface unused-knot padding
discrepancy is retained in the primary report.

The complete optimized geometry/command/bridge suite passes 3,617 tests (11
ignored), including surface Rebuild trim/seam/pole reconstruction, readonly
previews, source validation, selection, attributes and undo/redo. Workspace lint
passes in both feature configurations with warnings denied, and formatting passes.

[Diagnostic timings](smlib-surface-rebuild-performance.json) for 64×64 and
128×64 warped tensors at degrees 3 and 11 show median native times 1.05–2.13
times faster than the pre-change producer on this Linux ARM64 machine. Five
requests per backend each run an untimed warmup and five timed rebuilds;
serialization is excluded, and other compilation was active. These measure
the complete preparation and tensor construction path.

The command retains its 256-point per-axis limit, degrees 1–11, positive source
weights and aggregate output budget. Direct native calls also obey NLib's
degree-17 limit for source and target; the portable geometry API accepts higher degrees. Native basis,
chart and LU restrictions still apply. Continuous approximation error, arbitrary
conditioning and speed relative to Rhino remain unqualified. Fresh captures
remain unavailable pending the Wine licensing startup recorded in
[isocurve qualification](smlib-isocurves.md). See the
[limitations ledger](usd-brep-limitations.md).

```sh
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo test --release -p viboceros-smlib \
  --features native --test surface_rebuild --test curve_rebuild --test loft_basis
```
