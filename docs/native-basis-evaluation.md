# Native basis and point evaluation

The shared native basis evaluator now computes actual unclamped endpoint
limits. Its old endpoint shortcuts always returned the first or last control
point, even when the knot vector did not clamp that end. Curve and tensor
surface point queries inherited that error.

Evaluation now selects an exact nonzero knot span: inward at active endpoints,
and on the requested side at interior breaks. Full-order groups and inactive
endpoint padding retain their separate limits. Identity-row shortcuts remain
for genuinely clamped ends. Positive finite knot widths on the active domain
are accepted without a dimensionful `1e-24` cutoff. The legacy editing span
helpers and extrapolation cutoff remain separate.

`N_BasisDerivs` uses the same span selection and fixed stack scratch storage.
`N_CrvDerivs` reuses it instead of maintaining a second basis algorithm, while
retaining its existing rational derivative conversion. Nonfinite basis
derivatives return a numerical error. Generated-source patches compile the
repairs without modifying the pinned upstream checkout.

The direct regression covers 368 parameter/side configurations and 1,104
weighted curve/surface cases. Independent Bernstein polynomials, uniform
quadratic/cubic identities, explicit irregular endpoint coefficients and
isolated blocks supply expected values and derivatives. Degrees `0`, `1`, `2`,
`3`, `5` and `17`, chart scales `2^-360`, `1`, `2^360`, origin `1e12`, both
sides, three common weight gauges, full-order breaks, inactive padding and a
one-ULP final span are included. Derivative orders reach four at unit scale and
remote origin, and two at the extreme chart scales.

Basis values use `2e-13`, chart-normalized basis derivatives `2e-11`, points
`1e-10`, and chart-normalized curve derivatives `2e-9`, each multiplied by
`max(1, abs(expected))`. Homogeneous point differences divided by the common
weight gauge use `5e-12` with the same relative floor. Source curve controls and
knots remain unchanged. These bounds are test comparisons, not modeling
tolerance changes.

The opt-in `viboceros_smlib_basis_evaluation_baseline` links the two pinned
original evaluator objects ahead of the repaired archive, retaining all other
local fixes. The [validation record](native-basis-evaluation-validation.json)
retains those raw results, candidate and sanitizer runs, exact SDK requests and
responses, completed checks and source hashes. The original-object control
passes all checks at 228 of 368 stations; the repaired and sanitizer runs pass
all 368.

The fresh Rhino comparison covers continuous sources at 44 curve and 44
surface stations with identical stored knots and parameters. It checks curve
points and first derivatives, and surface points. The independent polynomial
matrix separately covers the extreme scales, full-order discontinuities,
inactive padding and higher derivatives. Maximum SDK differences are below
`9e-16` for points and `1.2e-13` for curve first derivatives, within the absolute
`1e-10` comparison bound.

Release validation passes 415 bridge tests and 1,600 command tests, with zero
failures and four existing ignored tests. Workspace lint and formatting pass.
All 112 saved join outcomes are byte-for-byte unchanged. Their existing strict
Rhino definition differences remain: four of 48 curved results match, while
the observed 32 local and one translated isocurve results retain their raw
mismatches. The other 31 translated cases have no recorded Rhino observation.

This qualification does not cover arbitrary span ratios, overflowed support
widths, extrapolation accuracy, every native evaluator or continuous Rhino
parity. The subsequent [splitter repair](native-exact-splits.md) qualifies
`N_CrvSplit` separately. Sanitizer
execution checks memory access with leak detection disabled; no leak-free or
performance result is claimed. Upstream USD, Python binding, OCCT, GUI and
`repo.sh`/`prog_test` harnesses were not run.

```sh
cargo test -p viboceros-smlib --features native --test basis_evaluation
```
