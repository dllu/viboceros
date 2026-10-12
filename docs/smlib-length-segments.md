# Native numeric-length and midpoint segments

Linux `native-smlib` builds route numeric-length, locked-direction and midpoint
SubCrv NURBS extraction through the same usd-brep segment producer used by
point/parameter SubCrv. The command's numeric helpers prepare the anchor-relative
sampling curve and final partial NURBS intervals with the shared extraction
callback. Previews and accepted results use these same helpers; analytic leaves
retain their native types and polycurve leaves retain their local domains.

Length integration, inversion, confirmation-side selection, full-traversal limits,
open-end clamping and midpoint-radius policy retain their existing implementation.
Closed-seam preparation retains its existing representation policy and now uses
[native refinement and extraction](smlib-curve-seams.md). The subsequent requested
NURBS trim also uses usd-brep. No new integration, inversion or interpolation solver is added.
Kernel handles stay outside document history, and preparation is readonly.

`Curve3::try_subcurve_at_arc_length_with_operations` supplies both exact
extraction and seam relocation while preserving the endpoint in the original source chart. The native
adapter exposes `subcurve_at_arc_length` with that endpoint. The command helpers
now return `CommandError`, retaining geometry errors through the existing typed
conversion and allowing native extraction failures to propagate to previews and
acceptance. Pure cursor/point-order helpers keep their geometry error types.

The geometry/command/bridge native suite passes 3,568 tests (11 ignored).
Four native-feature SubCrv preview tests pass, including numeric/midpoint options,
locked direction, cancellation, readonly caching and surface-edge reuse.
Workspace lint passes in both feature configurations with warnings denied;
workspace formatting also passes.
Independent native tests compare signed-length pieces against the portable
reference, including domains, source endpoints, seam crossings and source purity.
Requested anchors remain exact; solved endpoints use the existing tight numeric
bound because native seam arithmetic can change them by an ULP.
A shared-policy test checks both extraction stages and invalid-request rejection
before backend entry. The existing command regressions cover numeric direction,
midpoint clamping, markers, option memory, selection, grouping and undo.

The [20 signed-length SDK captures](../tools/rhino_oracle/observations/signed_length_subcurves.json)
were refreshed on isolated Xvfb using private scheme
`VibocerosOracleSmlibBridgeFinal20261010`. All 20 raw records match at the previously
established absolute `1e-6` ceiling and relative `1e-12`.
The [comparison](smlib-length-segments-comparison.json) retains input/observation
hashes and a stricter absolute `1e-8` diagnostic: 15/20 match there, with five
inherited length-inversion differences up to `7.99e-8`. These are existing SDK
versus sampler differences, not evidence of a usd-brep trimming defect. See the
[earlier qualification](signed-length-subcurves.md) and its original `1e-6` ceiling.
The additional 307 retained editing, decomposition, intersection, splitting and
surface-cutting records still match at their original tighter tolerances.

These finite checks do not certify arbitrary length inversion or all native
weight/chart ratios. The [shared segment limits](smlib-curve-segments.md) apply,
and timing records include the harness rather than establishing kernel speedups.

```sh
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo test --release -p viboceros-smlib \
  --features native --test length_segments
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo test --release --features native-smlib subcurve_preview
```
