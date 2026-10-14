# Native exact curve Blend

Linux `native-smlib` builds construct curve `Blend` controls with the same
usd-brep endpoint interpolator as `BlendSrf`. The shared
[`EndpointInterpolation`](../crates/viboceros-geometry/src/endpoint_interpolation.rs)
represents independent position, first-derivative and second-derivative rows on
`[0,1]`. Native `N_BasisDerivs`, LU and homogeneous back solving produce the
minimum-degree control net, without sampling or refitting the source ends.
Portable builds also use this shared endpoint construction.

The core retains endpoint picking, source tangent/curvature queries, the
Rhino-calibrated default handles, independent explicit handles and bulges, and
the existing domain policy. G0/G0 remains an exact direct line. Mixed or
endpoint-specific blends use `[0,1]`; ordinary equal-continuity G1/G2 blends
retain their independently integrated arc-length domains. Endpoint coordinates
are restored bit for bit after solving relative to their common origin.

Curvature constraints supply normal acceleration equal to the source curvature
vector times the square of the blend's endpoint speed. Tangential acceleration
remains zero. The constructor checks the returned polynomial weights, control
count, finite coordinates and endpoint jets before document insertion. Native
errors propagate through the command transaction. The command and Python
oracle select the same backend; source geometry is readonly.

The [native regression](../crates/viboceros-smlib/tests/curve_blend.rs) covers
324 combinations of line, arc and rational cubic sources, all nine continuity
pairs, all four endpoint pairs and three handle/bulge policies. It compares
complete controls and 33-point grids, checks source curvature and exact endpoint
coordinates, and retains the source identities. Degree-18 sources with common
weight gauges `1e-280` and `-1e280` also work: only their endpoint targets enter
the native degree-1–5 solver. Shared surface and batched monomial regressions
continue to cover 432 surface combinations and the 4,096-column endpoint budget.

The [retained native replay](smlib-curve-blend-comparison.json) runs all 282
existing Rhino SDK fixtures through `OracleClient(native_smlib=True)`. Line,
arc, polynomial/rational NURBS, polyline and polycurve cases retain degrees and
weights exactly, with control coordinate errors below `1e-12`. Known independently
integrated arc-length domain differences retain the earlier `5e-8` ceiling.
The measured maxima are `5.33e-15` for control coordinates and `4.59e-8` for
parameters.
The [Rhino observations](blend-oracle.md) remain unchanged. This replay does not
launch Rhino or qualify interactive command behavior and arbitrary conditioning.

`Blend` also accepts `G0`, `G1` and `G2` as continuity aliases. The command
regressions check its source retention and undo/redo. Malformed backend results
and injected backend errors are rejected in the core before any document edit.

```sh
cargo test --release -p viboceros-smlib --features native --test curve_blend
cargo test --release -p viboceros-oracle --features native-smlib --lib blend_curve::tests
cargo test --release -p viboceros-command --features native-smlib blend::tests
python3 -m tools.numerics.compare_native_curve_blends /tmp/blend-replay.json
python3 -m tools.rhino_oracle replay tools/rhino_oracle/fixtures/blend_lines.json \
  --observations tools/rhino_oracle/observations/blend_lines.json \
  --native-smlib --absolute-epsilon 5e-8 --timeout 600
```

The earlier [surface performance record](smlib-blend-hermite-performance.json)
keeps its original implementation through immutable Git source references.
Current kernel performance relative to Rhino, G3/G4, interactive shape handles
and blend trimming/joining remain further work.
