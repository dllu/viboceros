# Native Bezier and single-span decomposition

Linux `native-smlib` builds delegate `ConvertToBeziers` and
`ConvertToSingleSpans` to usd-brep's `N_CrvDecomposeBez` and
[repaired tensor producer](native-tensor-bezier.md), `N_SrfDecomposeToBez`.
Curve conversion first acquires the existing exact analytic/polycurve NURBS
representation. Command selection, unit-domain output, fresh attributes,
creation order, deletion choices and atomic undo keep their existing policies.
The portable extractor remains available to other platforms and tight bounds;
these native command paths do not call its control-blending routine.

The adapter accepts complete full knot vectors, including periodic and
unclamped padding. Selected curves are privately clamped through the
[repaired NLib splitters](native-exact-splits.md). The tensor producer prepares
both axes privately with those same splitters, including inactive endpoint
padding. Exact binary chart conditioning protects tiny/large parameter
domains, and every output interval and control count is checked against the original nonempty spans.

One-axis strips decompose each control line independently, preserving its
common weight gauge and the untouched axis's entire knot vector/control count.
Both calls the tensor producer directly and retains U-outer, V-inner order.
Intermediate controls remain homogeneous, so an intermediate
zero weight can disappear during the second direction without being rejected
as a final control at infinity. Local coordinate origins and binary weight
scales reduce avoidable arithmetic overflow and cancellation; final controls
restore the original gauge and world placement.
Origins come from active controls and must round-trip every source coordinate;
an unusable coordinate origin falls back to zero. Inactive controls far from
the active image and vastly different coordinate magnitudes are retained in
regressions rather than silently collapsing nearby small controls.

The tensor producer now clamps a private net, starts independent full-order
control blocks at index zero, and completes degree-zero spans. The previous
U-then-V adapter composition has been removed. The
[direct qualification](native-tensor-bezier.md) retains complete independent
expected nets, safe-parent comparisons and sanitizer results. Directional
strips keep their curve producer because they preserve an untouched basis
and independent control-line gauges.

Kernel calls use the shared process lock. Native output snapshots remain owned
until every piece is valid; failures release all staged snapshots. Rust copies
validated definitions and frees every remaining snapshot even if one piece
cannot be decoded. No native handles enter document history.

Native degrees remain limited to 17, with one million input/aggregate output
controls. The shared command ceiling is 1,048,576 controls across sources.
A final zero-weight or nonfinite Euclidean control returns an explicit error.
General Both-direction weight ratios that cannot share a finite binary gauge
remain unsupported; directional lines may have independent scales such as
`1e-280` and `1e280`. Arbitrary extreme span ratios remain unqualified.
Qualification is bounded evidence, not a continuous geometry or performance
certificate.

The retained isolated Rhino 8 command captures contain 44 Bezier and 78
single-span cases. All 122 complete serialized records match at absolute `1e-8`, relative
`1e-12`, including output definitions, samples, domains, order, attributes,
selection and group tables. Maximum observed absolute errors are `2.67e-15`
and `1.34e-15`, respectively. The [comparison](smlib-bezier-comparison.json)
retains every outcome and fixture/observation hashes. The existing OpenNURBS
outer-knot serialization convention is shared by the oracle codecs; no new
comparison-time coefficient or attribute normalization is applied.

The tensor repair replays all 122 saved command records without comparison-time
normalization. Fresh public SDK checks separately compare the complete source
and native patch definitions at 144 paired sample stations. See the
[current validation record](native-tensor-bezier-validation.json) for completed
checks and raw artifacts. The original command captures used private scheme
`VibocerosOracleSmlibBridgeFinal20261010`; the fresh SDK checks use
`VibocerosOracleTensorBezier20261018`. Harness timings do not establish kernel
speedups.

```sh
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo test --release -p viboceros-smlib \
  --features native --test bezier
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo test --release -p viboceros-command \
  --features native-smlib bezier
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo test --release -p viboceros-command \
  --features native-smlib single_spans
```
