# Native directed curve segments and parameter splits

Linux `native-smlib` builds reuse the checked usd-brep segment extractor for
point/parameter `Split` and directed point/parameter `SubCrv` extraction. The
SubCrv point/parameter preview uses the same preparation backend as acceptance.
Source identity, attributes, groups, selection, option parsing, and undo remain
with the existing command policies. [Numeric-length and midpoint preparation](smlib-length-segments.md)
now share the extraction backend while retaining their sampling policies.

The geometry layer now accepts an exact NURBS extraction callback while sharing
its existing interval validation, reversal, cyclic ordering and seam assembly.
Analytic lines and arcs remain analytic; partial NURBS leaves in polycurves
use the supplied backend and keep their composite/local parameter mapping.
No-op trims retain their definitions, and invalid requests fail before extraction.
Both portable and native command paths use this same geometry-policy layer.

The native adapter exports `subcurve`, `split_curve`, and
`split_curve_at_parameters`. Increasing NURBS intervals use the existing
`N_CrvExtractCrvSeg` producer. Open decreasing intervals use the shared exact
control/knot reversal; closed decreasing intervals extract the two seam portions,
shift the second portion's chart and reuse the existing homogeneous append policy.
Single SDK-style splits return two clamped pieces; multiple closed splits follow
sorted cyclic stations, with their final piece crossing the source seam.
Command point/parameter splits preserve their existing seam polycurve form.
Segments already bounded by full-order knot groups copy their isolated control
blocks exactly, preserving independent gauges without homogeneous roundoff.

Native splitting uses two checked segment extractions rather than invoking the
upstream splitter at a full-order interior break. This avoids its negative
right-control index and undersized left polygon; the previous endpoint-only
workaround did not qualify that interior split path. See the
[limitations ledger](usd-brep-limitations.md) and the
[underlying segment adapter](smlib-curve-segments.md).

Native degree, control-count, weight-gauge and chart restrictions remain those
of the shared segment adapter. Reversal and seam assembly introduce no new
interpolation solver. Definition comparisons use numeric tolerances; native
and portable arithmetic may differ by an ULP. Attribute and history checks
retain exact equality. Geometry sampling is bounded fixture evidence, not a
continuous equivalence or performance certificate.

Fresh isolated Rhino SDK captures cover seven split and nine directed-subcurve
cases, including periodic and closed seam behavior. Five actual cutting-object
Split commands are also retained. All 21 complete records match at absolute
`1e-10` for SDK geometry and `1e-8` for commands, with relative `1e-12`.
The [comparison](smlib-directed-segments-comparison.json) keeps every outcome and
input/observation hashes. The additional 286 retained editing, decomposition,
intersection and surface-cutting records still match. No new comparison-time
normalization is applied. Rhino captures ran on isolated displays under scheme
`VibocerosOracleSmlibBridgeFinal20261010`; timings include the harness and do not
establish kernel speedups.

The native geometry/command/bridge suite passes 3,560 tests (11 ignored), and
the expanded three directed-segment regressions pass. Shared-policy tests also
cover composite local domains, analytic leaf retention, no-op identity and
invalid requests failing before the extraction callback. Workspace lint passes
in both feature configurations with warnings denied, and formatting passes.
Four native-feature SubCrv preview tests pass, covering readonly preparation,
caching, hover/options updates, cancellation and surface-edge reuse.

```sh
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo test --release -p viboceros-smlib \
  --features native --test directed_segments
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo test --release -p viboceros-command \
  --features native-smlib split_curve
```
