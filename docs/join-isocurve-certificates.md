# Exact isocurve uncertainty for Join

[Kernel](brep-edge-joining.md) · [Previous audit](join-trim-certificates.md) ·
[Complete replay report](join-isocurve-certificates-comparison.json)

Join can now certify interior constant-U/constant-V boundaries and natural
boundaries whose fixed surface direction is unclamped. These previously retained
the document-tolerance floor after rebuilding. Ordinary clamped natural sides
still use their cheap exact control-row/column path.

## Geometry proof

Fixing one parameter in a rational tensor surface gives homogeneous curve
controls `H_i = sum_j M_j(s) * w_ij * (P_ij, 1)`. The implementation evaluates
these controls with exact rational de Boor arithmetic, retaining the original
varying-direction knots. It never projects those controls to rounded Euclidean
points or uses the ordinary binary64 isocurve extractor as an exact certificate.

The shared span extractor now borrows either binary64 weighted points or exact
homogeneous controls. Both feed the same restricted Bernstein-product proof,
including reversed subintervals and proposed projective correspondences. A
signed common gauge is removed exactly; the resulting isocurve denominator must
have sign-coherent, nonzero controls. Mixed/zero resulting weights remain
unsupported. The tensor path is bounded to degree 16 in both directions and
charges the existing shared work budget before allocation and recurrences.

At a full-order knot in the fixed direction, both one-sided isocurves are
certified. At a full-order varying-direction knot coinciding with a trim endpoint,
both endpoint values are bounded. This retains the previous discontinuity guard
for both vertex and edge uncertainty. Continuous full-order knots can still
have zero error. Nonisoparametric trims and unsupported certificates keep
conservative propagation; input geometry and original uncertainty are unchanged.

Tests independently evaluate global tensor B-spline basis functions, rather than
calling the production local recurrence. They cover both axes and knot sides,
unclamped/repeated/full-order knots, reversed restrictions, large UV origins,
negative/subnormal/large weight gauges, degree limits, and work exhaustion.
An interior rational isocurve whose exact coordinate is `2*t/(1+t)` at binary64
`t=0.1` explicitly rejects a zero bound for its rounded spatial representation.

## Command evidence, including incomplete observations

The [64-case discovery fixture](../tools/rhino_oracle/fixtures/join_isocurve_certificates.json)
contains rational extrusions and nonseparable rational tensor surfaces, both UV
orientations, clamped/unclamped directions, and parameter origins `0`/`1e12`.
Every side is an interior rectangular trim. The oracle source builder accepts
optional `surface_face.trim_bounds: [[u0,u1],[v0,v1]]`, preserving the complete
underlying surface. Invalid intervals are rejected rather than ignored.

All 32 per-source 3DM artifacts passed native roundtrip checks. All 64 native
Join/JoinCopy operations succeed. The independently constructed seam planes are
`z=0` and `z=1/2048`: native seam uncertainty bounds their half-gap, `1/4096`.
Incident edges and vertices now receive certified uncertainty too.

The first full Rhino batch exceeded its **300-second response limit** while
processing translated UV inputs. Its [failure record](../tools/rhino_oracle/observations/join_isocurve_batch_timeout.txt)
is retained; it is not a complete response or a kernel timing measurement.
Separate owned private-Xvfb runs then completed:

- [32 local-UV cases](../tools/rhino_oracle/fixtures/join_isocurve_local.json),
  with [raw observations](../tools/rhino_oracle/observations/join_isocurve_local.json).
- [One translated case](../tools/rhino_oracle/fixtures/join_isocurve_origin.json),
  with its [raw observation](../tools/rhino_oracle/observations/join_isocurve_origin.json).

All 33 observed cases agree on command/document state, topology, rebuilt geometry
and uncertainty, apart from area values and redundant outer-knot representation.
**None is a full raw match.** The remaining 31 translated cases have native-only
evidence; they are not counted as matches, mismatches, or native errors.

The [independent area calculator](../tools/rhino_oracle/references/join_isocurve_areas.py)
uses global tensor basis derivatives, the homogeneous quotient rule and Gaussian
quadrature over the original trim rectangle. Orders 32/64 at 80/100 decimal
digits agree past twenty decimal places. This is a numerical accuracy witness,
not a formal integration error enclosure.

| Profile | Independent area, rounded | Largest native error | Largest local-UV Rhino error |
| --- | ---: | ---: | ---: |
| Clamped extrusion | 42.1146894731649207 | 1.9e-15 | 2.73e-10 |
| Unclamped extrusion | 15.5043313117302562 | 3.5e-15 | 3.24e-10 |
| Clamped tensor surface | 48.9420469782293236 | 9.7e-15 | 4.54e-10 |
| Unclamped tensor surface | 14.2464529512372429 | 4.9e-15 | 6.23e-10 |

In the isolated translated case, Rhino's area is approximately `0.000278` below
the same reference. Native areas retain their accuracy after exact UV translation
and transposition. Accuracy is not reduced to manufacture a Rhino match.

The previous 884 pass/fail classifications are unchanged. Sixteen unclamped
records have tighter uncertainty but retain other raw differences; the other
868 rows are unchanged. A separate comparison of every native raw outcome against
the copied `9138205` executable, excluding only elapsed time, confirms that only
component tolerances changed in those sixteen cases. Including the 33 new comparable cases, the replay now
has **651 full matches, 32 native errors, and 234 other differences in 917 cases**.
Fixture, raw observation, source, executable and shared-artifact hashes accompany
the report. Seam coalescing, general nonisoparametric certificates and ordered
corner policies remain unfinished.

```sh
cargo test --release -p viboceros-geometry join_edges
cargo test --release -p viboceros-oracle interior_isocurve_join
python3 -m tools.rhino_oracle replay tools/rhino_oracle/fixtures/join_isocurve_local.json --observations tools/rhino_oracle/observations/join_isocurve_local.json --absolute-epsilon 1e-10 --relative-epsilon 1e-12
# Optional independent reference; requires mpmath:
python3 tools/rhino_oracle/references/join_isocurve_areas.py --order 64 --digits 100
```

Replay deliberately exits 1 for the retained discrepancies. The 64-case discovery
fixture is not paired with a complete Rhino response; use the observed subsets
for replay.

Verification passed 2,881 Rust tests (22 ignored in the regular run), 232 Python
tests, formatting, and Clippy/Rustdoc with warnings denied. README remains 43 lines.
