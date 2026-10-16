# Prepared Gordon curve families

The native kernel exposes
`prepared_gordon_surface(u_curves, v_curves, u_stations, v_stations, edge_tolerance, interior_tolerance)`.
U stations locate V curves on U curves, and V stations locate U curves on V
curves. Both lists cover the shared unit chart in strictly increasing order.
Prescribed contacts must agree within the smaller requested model tolerance.
Each complete original input must then certify at its prescribed isoparameter
in the original world frame. A failed or inconclusive proof rejects; a general
pullback cannot substitute a different parameter chart. Sources stay intact.

This primitive requires already oriented, polynomial, unit-domain curves with
exactly constant common-sign weights. It shares auto-sorted construction's
working frame, ownership, mutex, certificates and allocation guards: 2–32 curves
per family, degree at most 16, 128 controls per input and 2,048 total controls.
Family sorting, fitting, topology and injectivity remain separate requirements.

The native adapter preserves an already shared station bit for bit; averaging
identical nonbinary values can otherwise move one by an ulp. Exact identity
maps skip alignment knot insertion and mapping. Other maps retain the existing
piecewise-affine preparation. Compatibility and Gordon construction still use
usd-brep's existing operations, preserving complete native output knots.

The regression with stations `0.2` and `0.4` retains a 7×7 cubic tensor with its
original simple knots, checks independent biquadratic points and certifies all
six complete curves. Other cases reject invalid charts, weights, domains and
contacts without mutation. Negative and tiny gauges work on rotated geometry
at a remote origin. The strict `1e-9` request rejects contacts displaced by world
rounding; the explicit `1e-6` request is independently admitted.

The shared-station refit regression also requires simple interior knots in the
auto-sorted result. That diagnostic tensor falls from 83×66 to 81×64 controls
and retains C2 continuity. Its global smooth-map policy still fails the Rhino
interior comparison and remains unselected. Raw biquadratic construction also
avoids unnecessary crossing knots.

The [curve-fitting request](../tools/rhino_oracle/fixtures/network_curve_fit.json)
and [fresh SDK capture](../tools/rhino_oracle/observations/network_curve_fit_rhino_20261016.json)
retain twelve public `Curve.Fit` results: six quadratic inputs at `1e-6` and
`1e-7`. Their midpoint parameters remain close to arc-length values, while the
saved NetworkSrf tensor uses different stations. Standalone Fit therefore does
not reproduce the network preparation on this fixture.

The extended `network_parameterization` example compares four independent
candidates, records knot multiplicities and complete original-image bounds,
reads those twelve fitted curves, and samples quadratic Gordon reconstruction
of the saved tensor at 4,225 parameter pairs. A separate native reconstruction
uses isocurves extracted from that tensor. Those reference-derived curves never
feed the independent candidates and cannot qualify an independent NetworkSrf
implementation.

The [schema-three observations](../tools/rhino_oracle/observations/network_parameterization_prepared_viboceros_20261016.json)
retain all 24 complete original-image proofs. The sampled quadratic residual is
about `3.8e-15`; native reconstruction from the reference-derived isocurves
matches all 25 returned query points within about `1.01e-9`, retaining the
47×61 tensor dimensions. The four independent candidates still fail `1e-6`
interior matching; their closest candidate remains around `7.33e-4`.

These diagnostics help distinguish curve-family preparation from construction
on one fixture; they do not identify Rhino's internal algorithm or prove
continuous surface separation. NetworkSrf remains unregistered. Compatible
family fitting, interior matching, broader inputs and performance remain
unfinished. No proprietary implementation was inspected.

```sh
python3 -m tools.rhino_oracle rhino tools/rhino_oracle/fixtures/network_curve_fit.json \
  --output /tmp/rhino-network-curve-fit.json
cargo run --release -p viboceros-smlib --features native \
  --example network_parameterization > /tmp/prepared-gordon.json
```

The [validation record](prepared-gordon-families-validation.json) retains the
tests, compiler/sanitizer evidence and diagnostic observations.
