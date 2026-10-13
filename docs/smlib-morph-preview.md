# Native morph preview preparation

Linux `native-smlib` morph previews now reuse the existing usd-brep degree-
elevation, isocurve and curve-segment adapters. This extends
[native morph interpolation](smlib-morph-interpolation.md) to preparation and
wire generation; no new elevation, extraction or trimming algorithm is added.

`MorphPreviewOperations` supplies function-pointer producers for interpolation,
curve/surface elevation, isocurves and interval trimming. Curve, surface and
B-rep wire cages accept these operations through `try_new_with_operations`.
The command layer selects the native bundle; the viewport uses it for both
prepared geometry and boundary-only overlays. Standalone curve cages share
`prepare_morph_preview_curve` with the same degree policy.

Geometry callers retain portable defaults. Existing `try_new_with_solver`
cages now retain their selected solver, so a point map with no solver override
still uses the prepared producer. A point-map override takes precedence.
Template degree/domain consistency and existing control/wire ceilings are
checked before sampling. Sources remain immutable, and preserve-structure
requests retain their direct mapped controls.

Native elevation already composes independent full-order pieces. Low-degree
curve and tensor preview preparation therefore now accepts the tested interior
full-order cases that portable degree preparation rejects. Independent cubic
Bernstein controls verify the two separated curve images, and tensor samples
verify both sides of the surface break. The preview still follows the original
source parameter and side policy.

B-rep wire generation keeps the shared local UV frame, density sites, trim
interval discovery, zero-length filtering and hole handling. Each visible
isocurve uses the checked native extractor, and partial wire intervals use the
existing native curve trimmer. Whole intervals retain the extracted definition.
Higher-degree editing/extraction above the pinned degree-17 limit keeps the
existing portable producer; kernel failures at supported degrees propagate.
Mixed-degree native interpolation above 17 retains its earlier restriction.

The annulus regression also exposed an adapter chart-conditioning defect.
Source knots and generated trim/interpolation parameters can be subnormal
beside a moderate domain. The shared binary chart now checks every knot for
reversibility even without an explicit query list. Interpolation supplies every
station; trimming supplies both boundaries; refinement supplies its requested
knot. Exact spans, source data and kernel tolerances are preserved. Subnormal
trim tests retain exact interval endpoints and independently check the source
image; a fixed-row interpolation test retains its exact tiny knot and targets.
These repairs do not qualify arbitrary mixed-range charts.

Native regressions cover separated curve/surface pieces, source retention,
structure preservation, complete annulus wires clipped around a rational hole,
higher-degree portable extraction, and subnormal trim/interpolation charts.
The complete native bridge suite passes 190 tests (one ignored). All 93
viewport preview checks pass (four ignored); workspace lint passes with warnings
denied in both feature configurations, and formatting passes.

[Retained extraction replay](smlib-morph-preview-retained-replay.json) checks
365 raw Rhino records after the shared chart repair: 364 match, and the existing
whole-domain inactive-knot-padding discrepancy remains. Signed-length cases
retain their established `1e-6` inversion allowance; other operations retain
their recorded tolerances. These are existing captures, not fresh Rhino runs.

Viewport tests separately check source snapshots, cached geometry, accepted
command correspondence and boundary overlays. All cages remain display
approximations: this change makes no continuous fit, injectivity, embedded-solid
or Rhino-speed claim. See the [usd-brep ledger](usd-brep-limitations.md).

```sh
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo test --release -p viboceros-smlib \
  --features native --test morph_preview --test morph_interpolation
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo test --release -p viboceros \
  --features native-smlib preview
```
