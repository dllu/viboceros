# Smooth Loft Refit handle correction

[Loft Refit](loft-refit.md) · [Validation](loft-refit-adjustment-validation.json)

Open single-Bezier profiles can now use one common positive scale for both
endpoint-tangent handles when the initial cubic fails. The complete corrected
curve must still have a continuous image-distance bound within half the requested
tolerance. Sources remain intact; unsupported fits leave the document unchanged.

The correction samples nine incremental length stations with fractional stopping
precision `1e-4`. Its midpoint target is projected onto the initial cubic. At that
candidate parameter, the two handle offsets give the positional influence of a
common scale. Projecting the residual onto that influence proposes the scale.
A midpoint already within the reserve cannot justify a correction; errors away
from it still require the full proof or subdivision. Nonpositive or nonfinite
scales and zero influence fail without producing an output.

The existing incremental sampler and closest-point search supply proposals;
the shared Bernstein correspondence certificate decides acceptance. No change
was made to precise length inversion or independent scalar-query semantics.

Native evidence separates the public batch tolerances: for these NURBS inputs,
`NormalizedLengthParameters` at `1e-2` corresponds to the internal `1e-4` batch,
while the public `1e-4` capture corresponds to `1e-6`. The SDK documents the
[public tolerance parameters](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Geometry_Curve_NormalizedLengthParameters_2.htm);
the correspondence here is measured. Rational and polynomial candidate variants
retain the same stations. Complete scalar SDK parity remains unfinished.

The references retain 18 closest queries, 28 finer-batch records, 56 fitting-clock
records, 15 elliptical records, 45 seeded-closest diagnostics and ten command
records. Corrected elliptical controls differ from Rhino by about `3e-9`, including
when the correction uses Rhino's public closest-point query. Those newly
supported records use a `1e-8` parity threshold; the other 219 smooth records
retain `1e-9`. This numerical comparison is distinct from the continuous
source-to-fit bound. Command references cover all five styles and Straight's
face splitting, source selection, attributes and output topology.
The [subdivision references](loft-refit-subdivision-validation.json) distinguish
whole-profile fits from independently trimmed fits, including source-domain
changes and reversals. Fitting each half separately does not reproduce the joined
result; those observations do not establish a subdivision or joining policy.
General subdivision, piece joining, multispan smooth sources, single-input smooth
preparation and explicit angle overrides remain unfinished. Degree, profile,
work and correspondence limits follow the [shared proposals](loft-refit-candidates.md).

```sh
cargo test -p viboceros-geometry --lib curve_fit::g1
cargo test -p viboceros-oracle --test loft_refit_coarse_native --test loft_refit_native
python3 -m unittest tools.rhino_oracle.test_loft_refit_adjustment
```
