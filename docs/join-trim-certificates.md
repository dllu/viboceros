# Certified uncertainty for partial natural boundaries

[Kernel](brep-edge-joining.md) · [Command](commands/join.md) ·
[Complete replay report](join-trim-certificates-comparison.json)

This records the natural-boundary implementation at `9138205`. The
[isocurve follow-up](join-isocurve-certificates.md) extends certification to
interior trims and fixed-unclamped directions; the raw historical audit remains unchanged.

Join now certifies partial natural surface boundaries against the original
surface, without treating rounded knot-insertion controls as exact. This resolves
all eighteen uncertainty-only differences from the [short-overlap audit](join-short-overlaps.md).
That 102-case fixture now has **96 full matches**, no native errors, and six
remaining offset-rebuilding, subdivision and raw-order differences.

## Exact restriction, not rounded extraction

An oriented UV isocurve on a clamped natural side traces a subinterval of an
exact surface control row or column. Its stored spatial edge, however, may have
rounded controls from previous splits or endpoint adjustment. The certificate
keeps the original row and its exact binary64 parameter endpoints. It normalizes
the restricted interval with arbitrary-precision rational arithmetic, retaining
the complete original knot vector and controls.

The existing polar-form extraction and rational Bernstein difference proof then
cover every active span within that interval. Knots outside the restriction can
fall outside `[0,1]`; the span partition clips them **before** inverse projective
mapping, which need not be defined outside its unit interval. Endpoint-based
projective proposals use derivatives of the restricted end spans, not the
original complete curve. Every proposal still requires the whole-span proof.

Vertex uncertainty uses the exact homogeneous endpoint of the restriction,
approached from its interior. At a full-order knot, the left and right values can
differ. Reversal changes which side is required. Neither default right-sided
evaluation nor rounded Euclidean endpoint extraction is substituted for this
certificate. Full clamped endpoints retain their fast exact point comparison.
At an internal full-order knot coinciding with a trim endpoint, the surface-image
adapter additionally bounds the opposite one-sided value, for both the vertex
and the exact stored-edge endpoint. A valid restriction proof alone cannot hide
a jump at the surface endpoint. Continuous full-order knots still admit zero bounds.

The **fixed** surface direction must be clamped for its row/column to be exact;
the **varying** direction may be unclamped. Unsupported interior isocurves,
fixed-unclamped directions and nonisoparametric trims still propagate conservative
uncertainty. Partial restrictions use the existing degree-16 certificate ceiling,
depth limit and shared work budget. Original component tolerances are never
erased; budget exhaustion and validation failure leave sources unchanged.

Independent tests cover exact global Cox–de Boor evaluation, reversed and
unclamped restrictions, repeated knots, discontinuous one-sided endpoints,
projective maps with outer knots beyond their inverse's domain, subnormal and
large weight gauges, large UV origins, and budget exhaustion. A line restricted
to `[0.1,0.2]` specifically proves that its rounded trim representation has
nonzero error: accepting its new controls as an exact image would understate
uncertainty.

## Fresh command evidence and remaining differences

The [80 new requests](../tools/rhino_oracle/fixtures/join_trim_certificates.json)
and [raw observations](../tools/rhino_oracle/observations/join_trim_certificates.json)
use Rhino 8.32.26160.13001 in an owned private-Xvfb session. Twenty source pairs
cover polynomial and rational curves, multiple spans, unclamped varying
directions, both UV orientations, origins `0` and `1e12`, short split pieces and
reversed profiles. Each pair has normal gap `0.0005`, document tolerance `0.001`,
and three pre-existing boundary splits. Both commands and both selection modes
are recorded. All forty shared 3DM artifacts passed native roundtrip checks and
Rhino insertion checks. Split parameters use spatial edges' local domains,
including negative domains on reversed edges, not native surface UV coordinates.

All eighty commands succeed natively and in Rhino, with matching face senses
and midpoint seam planes. Native mated-edge uncertainty is now the certified
half-gap, approximately `0.00025`. Nevertheless **none is a full raw match**:

- Native retains four mated seam pieces and ten total edges. Rhino coalesces a
  representation-dependent subset of adjacent pieces, leaving seven to ten
  total edges. Even tiny or rational pieces do not follow a uniform cleanup
  rule. Table order, UV trim representations and retained curve definitions
  therefore differ. No cleanup or reparameterization is hidden in comparison.
- Unclamped inputs also expose the known redundant outer-knot interchange
  difference: OpenNURBS reconstructs omitted first/last full-vector knots from
  their neighbors. This is not active-domain clamping or a changed surface
  shape. Those inputs retain the native conservative floor on incident edges
  whose fixed surface direction lacks a certificate.
- Large UV origins expose independent area differences, including on retained
  JoinCopy originals. The same shape should have the same area after parameter
  translation; native accuracy is not reduced to reproduce those values.

The [independent area calculator](../tools/rhino_oracle/references/join_trim_areas.py)
uses global basis functions and their derivative, the homogeneous quotient rule,
and knot-partitioned speed integration at extrusion height 3. Runs at 80 and 100
decimal digits agree beyond 70 digits. This is a numerical accuracy witness, not
a formally verified quadrature error enclosure; the quadratic additionally has
the closed form `45*(sqrt(2)+asinh(1))`.

| Profile | Independent area, rounded | Largest native error | Largest Rhino error at UV origin `1e12` |
| --- | ---: | ---: | ---: |
| Polynomial quadratic | 103.3014217226687133 | 7.5e-15 | 2.34e-5 |
| Rational cubic | 93.0397321960035926 | 5.6e-15 | 2.60e-3 |
| Unclamped quadratic | 35.4300367008035362 | 4.7e-15 | 1.50e-5 |
| Rational multispan | 126.2578819740879657 | 1.4e-14 | 3.94e-6 |

Across fourteen surface-command fixtures, **651 of 884** cases fully match,
**32** retain native errors, and **201** have other differences. Of the previous
804 rows, eighteen become matches and four offset cases have tighter bounds but
remain mismatches; all other **782 rows are unchanged**. The eighty new discovery
differences are retained in full. Fixture, observation, executable, baseline and
shared-artifact hashes accompany the report. Timings do not establish kernel
performance parity.

```sh
cargo test --release -p viboceros-geometry join_edges
cargo test --release -p viboceros-oracle presplit_curved_boundaries
python3 -m tools.rhino_oracle replay tools/rhino_oracle/fixtures/join_trim_certificates.json --observations tools/rhino_oracle/observations/join_trim_certificates.json --absolute-epsilon 1e-10 --relative-epsilon 1e-12
# Optional independent reference; requires mpmath:
python3 tools/rhino_oracle/references/join_trim_areas.py --digits 100
```

Replay intentionally exits 1 for the retained differences. Redundant seam
coalescing, general surface-image certificates and ordered corner policies remain
implementation work.

Verification passed 2,873 Rust tests (22 ignored in the regular run), 232 Python
tests, formatting, and Clippy/Rustdoc with warnings denied. README remains 43 lines.
