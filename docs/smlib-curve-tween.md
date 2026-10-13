# Native sampled TweenCurves

Linux `native-smlib` builds route TweenCurves sampled-source interpolation
through usd-brep's existing cubic basis and banded matrix routines. The bridge
exports `curve_tweens`; the shared `try_tween_nurbs_curves_with_sampling`
callback retains source division, exact coincidence, linear detection, control
blending, sample/count budgets and source order. Standalone command execution
and the `curve_tween_geometry` oracle use this backend.

Nonlinear sources use the existing open cubic target and endpoint-handle
policy. Open four-channel handle systems now admit 10,000 picked stations,
which covers the existing 9,999-division command range and its 10,002-control
cubic output. Their native five-wide matrix keeps storage, factorization,
back-solving and residual checks linear in point count. Other interpolation
layouts retain their previous native resource limits; public InterpCrv and
curve-through constructors still accept at most 256 input points.

The large-point callback uses the same cubic preparation and native producer
as smaller requests. Exact endpoints/handles, constant coordinate channels,
polynomial weights and local-origin conditioning remain shared. Portable builds
retain their original linear-memory tridiagonal path for large sample sets.
No basis evaluator or linear solver is added.

ControlPoint and Refit retain their direct control construction. Refit still
uses adaptive, arc-length-matched Hermite segments with shared knots and
one-sided kink directions; it does not need an interpolation solve. Compatible
and unequal control blending keep their first-source weight and final-control
rules. Linear sampled sources retain degree-one output. Every mode preserves
the existing aggregate million-control budget, source retention, attributes,
selection and history.

Native regressions compare complete bases, controls, weights and 129 parameter
samples with the portable producer for all permanent modes, short-spacing
sources and division counts 2, 3, 255, 256, 1,000 and 9,999. Large nonlinear
samples retain relative coordinate detail down to `1e-280`. Invalid sample and
aggregate output budgets keep typed geometry errors. Shared-policy tests
verify direct-mode bypasses, linear construction and invalid backend layout
rejection.

The [19-record comparison](smlib-curve-tween-comparison.json) matches the saved
pre-change producer at absolute `1e-9`, relative `1e-12`; maximum numeric
difference is `5.33e-15`. It includes all permanent modes, short sample spacing
and large one/three-output requests through 9,999 divisions. Input fixtures
and both audit hashes are retained. This reference is portable Viboceros
interpolation rather than a new Rhino capture.
The same binary replays 365 retained Rhino refinement/extraction/seam records
with 364 complete raw matches. The existing whole-surface unused-knot padding
discrepancy remains explicit in the primary report.

[Diagnostic timings](smlib-curve-tween-performance.json) at 1,000 and 9,999
divisions show the complete native and pre-change paths within two percent
of each other on this Linux ARM64 machine. Five requests per backend each run
an untimed warmup and three timed tweens; serialization is excluded, and other
compilation was active. These are two-case timings, not a general or Rhino
performance qualification.

The complete optimized geometry/command/bridge suite passes 3,637 tests (11
ignored), including TweenCurves output, selection, source retention and history
checks, large sampled requests and the existing interpolation consumers.
Workspace lint passes in both feature configurations with warnings denied;
formatting passes.

Kernel chart and LU restrictions still apply; arbitrary spacing/coordinate
ratios and continuous source-curve agreement are unqualified. Sampling and
adaptive refit retain their existing numerical/model policy. Fresh Rhino
captures remain pending the Wine licensing startup described in
[isocurve qualification](smlib-isocurves.md). See the
[limitations ledger](usd-brep-limitations.md).

```sh
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo test --release -p viboceros-smlib \
  --features native --test curve_tween --test curve_interpolation
```
