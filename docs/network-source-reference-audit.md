# Network source preservation and reference outputs

The network oracle now accepts optional `source_sample_fractions`: up to 65
strictly increasing fractions in `[0,1]`, applied to every original input's own
parameter domain. Both implementations report the original point, returned
closest point, surface parameters and measured distance. Default requests keep
their existing response shape. Samples diagnose returned outputs; they do not
certify a continuous source-image distance or a global surface minimum.

The [request](../tools/rhino_oracle/fixtures/network_source_fit.json) repeats the
six-curve biquadratic network at two requested edge/interior tolerances. Each
operation retains the original 25 queries and adds 65 points per original
curve, including both endpoints. The API call uses position continuity; the
[public documentation](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Geometry_NurbsSurface_CreateNetworkSurface.htm)
defines continuity value 1 as position matching.

The [fresh Rhino 8.32 capture](../tools/rhino_oracle/observations/network_source_fit_rhino_20261016.json)
reproduces the saved `1e-6` tensor definition and all 25 query outputs exactly.
The requested tolerance and sampled returned-point distance differ:

| Requested source tolerance | Tensor controls | Source samples | Largest reported distance |
| --- | --- | --- | --- |
| `1e-6` | 47×61 | 390 | `4.4041e-5` |
| `1e-7` | 113×172 | 390 | `5.7315e-6` |

These are distances to the SDK's returned points, not certified lower bounds
on distance to the surface. A separate reference-derived readback asks Rust's
existing closest-point implementation about the worst sampled query on each
curve. Those tensors never enter independent network construction.
The [Rust readback](../tools/rhino_oracle/observations/network_source_reference_readback_viboceros_20261016.json)
agrees with the twelve SDK returned points within `8.7e-12`. The
[independent raw Gordon producer](../tools/rhino_oracle/observations/network_source_fit_viboceros_20261016.json)
returns source-sample distances below `5e-16` on both requests; that does not
qualify its different interior shape.

The portable `network_source_audit` example checks a stronger, narrower fact:
some reference outputs cannot meet both the source-preservation promise and
the existing `1e-6` returned-point comparison. It reuses exact one-sided source
position certificates, outward sums and `Point3::distance_bound_to`; a rounded
norm never decides incompatibility. All 780 newly sampled source points
certify exactly on their stored original curves.

For example, original query 10 is `[2.12, 0, 0.9964]`. It certifies on curve 0
at parameter `0.53` within `1e-12`. The saved and fresh `1e-6` reference outputs
are about `1.58173e-5` from that query. A surface preserving the whole original
curve within `1e-6` has some point within `1e-6 + 1e-12` of the query. Its
globally closest point therefore also satisfies that distance limit. If this
closest point matched the reference output within `1e-6`, the triangle
inequality would put the reference within `2e-6 + 1e-12` of the query. The exact
distance predicate proves that it lies outside the outward-rounded sum.

The [retained exact audit](../tools/rhino_oracle/observations/network_source_audit_viboceros_20261016.json)
retains one incompatibility among the original 25 queries at
`1e-6`, plus 46 of the 390 new source samples. At the tighter requested source
tolerance, nine new samples still prove incompatibility with a `1e-6` output
comparison. An absence of a witness does not prove compatibility.

This explains why passing all existing reference-output checks cannot be the
sole next gate for a source-preserving producer. No tolerance, proof budget or
comparison has been relaxed, no mismatch is relabeled as passing, and the
application NetworkSrf command remains unregistered. Independent family
parameterization and interior fitting remain unfinished; the earlier
[shared-family fitter](shared-gordon-curve-fit.md) retains its passing complete
original-image proofs and its failed reference comparison.

Reproduce from the repository root:

```sh
python3 -m tools.rhino_oracle rhino tools/rhino_oracle/fixtures/network_source_fit.json \
  --output /tmp/network-source-rhino.json --timeout 300
cargo run -p viboceros-oracle --example network_source_audit -- \
  tools/rhino_oracle/fixtures/network_source_fit.json \
  tools/rhino_oracle/observations/network_source_fit_rhino_20261016.json
python3 -m tools.rhino_oracle.network_surface_readback \
  tools/rhino_oracle/fixtures/network_source_fit.json \
  tools/rhino_oracle/observations/network_source_fit_rhino_20261016.json \
  /tmp/network-source-readback.json --source-distances
cargo run -p viboceros-oracle -- /tmp/network-source-readback.json /tmp/network-source-readback-rust.json
```

The [validation record](network-source-reference-audit-validation.json) retains
six distinct Rust tests (eight executions across native and portable builds),
eleven Python tests, both fresh SDK captures, compiler checks, and the exact
audit. It separates witnesses about incompatible output requirements from
sampled SDK distances and reference-derived readback.
