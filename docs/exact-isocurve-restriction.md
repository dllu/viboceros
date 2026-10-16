# Exact isocurve restrictions in correspondence proofs

The continuous surface/curve certificate now reduces constant-coordinate UV
paths before extracting tensor patches. This closes the remaining original-curve
proof on the retained `biquadratic_network` fixture: all six originals certify
for each of the three diagnostic Gordon candidates, at the unchanged `1e-6`
absolute tolerance.

Every homogeneous UV control must have exactly the same coordinate-to-weight
ratio in one direction. Equal endpoints or an approximately constant coordinate
are insufficient. The exact polar-form extractor evaluates the needed columns
of the original stored B-spline tensor in that direction, then extracts only
the variable-direction Bernstein span. Numerical public isocurve extraction
does not supply certificate geometry.

For a UV degree `m` and tensor degrees `(p,q)`, this reduces composition degree
from `m*(p+q)` to `m*p` or `m*q`. The canceled homogeneous factor is positive.
The existing rational-linear restriction remains available; higher-degree paths
reuse the existing exact Bernstein products. Outward-rounded interval proofs
and knot-crossing boxes also use the exact reduced net. General UV paths retain
the full tensor calculation.

The cache retains one coordinate restriction at a time. Its original controls
and Bernstein spans are bounded by the tensor's variable-direction controls and
spans; repeated arbitrary coordinate queries cannot accumulate separate caches.
At shared knots the supported original tensor is continuous, so both incident
patches agree with the canonical original-column evaluation. Full-order internal
discontinuities remain unsupported. Domain, degree, arithmetic, depth and work
limits are unchanged, including the two-million-operation exact budget.

Four new core tests cover exact polynomial identities against independent full
tensor products; rational UV weights and both directions; endpoints, internal
stations, translated domains and extreme signed weight gauges; repeated and
reversed cache visits across simple and C0 knots; conservative operation-count
comparisons; interval admission/rejection checked against full exact products;
and refusal to collapse a coordinate differing by one floating-point step.
The native regression now certifies all six original curves for both arc-length
and smooth common-station refits, preserving source domains and inputs.

The [current observations](../tools/rhino_oracle/observations/network_parameterization_isocurve_bounds_viboceros_20261016.json)
retain all 18 complete original-image certificates. The previously inconclusive
arc-length source index 1 now has bound about `9.9851e-7`. Its source index 4
also needs only 49 UV controls, versus 115 in the
[earlier observations](../tools/rhino_oracle/observations/network_parameterization_original_bounds_viboceros_20261016.json).
Both captures remain available; neither changes the saved licensed Rhino input.

These are directed continuous original-curve-to-surface correspondences, not
topology, injectivity, surface continuity or symmetric distance proofs. The
surface tensors and Rhino returned-point comparisons are unchanged. The closer
arc-length candidate still differs by about `7.33e-4`, above the `1e-6` criterion.
NetworkSrf remains unregistered; its interior fitting policy, broader oracle
comparison and performance qualification remain unfinished. Reduced exact work
in bounded unit cases is not a general runtime or Rhino performance claim.

The [validation record](exact-isocurve-restriction-validation.json) retains the
test, compiler, sanitizer and diagnostic evidence. Reproduce the diagnostic with
`cargo run --release -p viboceros-smlib --features native --example network_parameterization`.
