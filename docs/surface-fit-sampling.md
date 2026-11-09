# Surface fitting sampling and controlled strips

[Fitting guide](geometry/surface-fitting.md) · [Validation record](surface-fit-sampling-validation.json)

Explicit-basis fitting now samples with streamed tensor contractions. Automatic
UV and image fitting reuse prepared span controls for source and candidate
queries. Scalar arithmetic, exact recovery, resource limits and complete
admission certificates remain shared with the previous implementation.

All 29 portable strip results retain identical controls, maps and bounds.
Both backends reproduce all 29 captured native bases within `1e-9`, recheck
every local source-image certificate, and pass support-surface 3DM round-trips.
The scalar/grid regression also compares control bits under translation,
negative weight gauges and extreme weight ratios requiring exact recovery.

A five-run **unoptimized development-build** benchmark interpolates one rational
degree-8 tensor into cubic bases. These are full interpolation timings, without
source-accuracy certification or a comparison against Rhino's performance.

| Controls per axis | Earlier median | Current median | Ratio |
| --- | ---: | ---: | ---: |
| 16 | 35.51 ms | 16.35 ms | 2.17× |
| 32 | 174.53 ms | 88.17 ms | 1.98× |
| 64 | 959.61 ms | 594.24 ms | 1.61× |

The private-Xvfb capture contains 29 surface and six section-curve fits through
public APIs. For this strip, native curved-axis knots survive changes in width,
transverse degree, V domain and uniform scale with scaled tolerance. Normalized
outputs agree under complete bounds. `Curve.Fit` selects different, non-dyadic
knots, so these observations do not justify substituting that fitter for native
surface policy. Full adaptive parity and the `FitSrf` command remain unfinished.

Frozen sources, capture provenance, terminal logs, the earlier implementation
and two superseded compile failures are retained in the validation receipts.
