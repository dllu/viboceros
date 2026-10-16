# Shared cubic curve-family fitting

The optional native kernel exposes `fit_gordon_curve_family(inputs,
interval_spans, fit_tolerance, numerical_tolerance)`. Each input supplies an
original NURBS curve and ordered `CurveFitStation` contacts, including both
natural endpoints. All inputs prescribe the same increasing unit output
stations. The returned polynomial cubics share exactly the same clamped knot
array, with simple interior knots and therefore C2 continuity.

The fitter reuses usd-brep's constrained least-squares operation
`N_FitCrvWeightedLstSqKnots`, with hard position and endpoint-derivative
equations. Bounded closest-parameter correction and the existing
`N_FitCrvApproxPts` geometric refitter can improve a candidate without changing
its basis. The bridge retains the existing kernel mutex, private stack owners
and owned NURBS transfer.

Private normalized and trimmed copies provide equal-arc-length samples. A
monotone quintic scalar clock joins the prescribed stations with shared first
derivatives and zero second derivatives in arc length. Positive stored
Bernstein control increments qualify each clock piece; collapsed proposals
reject. This avoids forcing abrupt speed changes into a C2 fitted curve. It
does not identify Rhino's preparation policy.

The translated family frame uses a bounded binary magnification of 1,024 to
keep native absolute stopping tolerances useful. No kernel globals, requested
model tolerances or proof budgets change. Sources and their independent
parameter domains remain intact. Common-sign rational weights use the existing
working-curve importer; the fitted output is polynomial.

Admission has distinct limits:

- Original contact targets must certify within the fit tolerance. Endpoint
  targets additionally retain their original positions within the absolute
  numerical tolerance.
- Every sampled point receives a fresh curve-point distance measurement in
  the private frame. The reported maximum must satisfy the fit tolerance.
  Newton refinement's returned parameter is a proposal; the curve is evaluated
  again even when the tighter Newton stopping test does not converge.
- Every prescribed contact certifies on the actual restored world curve using
  exact rational one-sided evaluation. The new
  `NurbsCurve::positions_within_distance` method reuses the existing certificate
  engine and its unchanged work limit.

The sample maximum is a diagnostic, not a continuous distance certificate or a
bound on world-restoration rounding. Callers constructing a surface must
separately certify each complete pre-fit original image. The regression and
example perform that check on the actual
[prepared Gordon surface](prepared-gordon-families.md) at `1e-6`.

Limits are 1–32 sources, 2–32 contacts, degree at most 16, 128 controls per
original and 2,048 total original controls. Each network interval has at least
four uniform output spans; all intervals together have at most 125 spans,
giving at most 128 fitted controls. Sampling uses eight points per span plus
the final endpoint. The native adapter independently guards layout and finite
data. Refitting is bounded by 16 parameter-correction passes and four calls to
the existing 100-iteration geometric refitter. Inadequate bases, failed native
solves and inconclusive contact proofs reject without output.

The retained example reads only original fixture curves to choose its common
stations (mean normalized contact arc lengths). Its saved Rhino tensor supplies
comparison outputs only. No reference-derived curve or station enters the
candidate. It fits at `1e-7`, constructs at `1e-6`, reports complete original
image bounds and compares all 25 saved closest-point outputs. Those comparisons
measure returned point differences, not continuous surface separation.

The [retained observation](../tools/rhino_oracle/observations/network_family_parameterization_viboceros_20261016.json)
uses 60 spans per interval and produces a 123×123 cubic tensor with simple
interior knots. The largest sampled fit distance is about `3.98e-8`; all six
complete original-image bounds are below `1e-6`. Its maximum Rhino query-output
difference is about `0.005695`. That still fails the requested `1e-6` comparison
and is worse than the earlier crossing-constrained arc-length candidate
(`0.0007333`). This smooth-clock policy remains experimental and unselected.

```sh
cargo run --release -p viboceros-smlib --features native \
  --example network_family_parameterization -- 60 > /tmp/shared-family.json
```

This is an experimental construction primitive. Independent NetworkSrf
interior matching, broader input preparation, topology, boundary continuity
and performance qualification remain unfinished. The application NetworkSrf
command remains unregistered.

The [validation record](shared-gordon-curve-fit-validation.json) retains 143
distinct Rust tests, native sanitizer/compiler checks, complete original-image
evidence and the failed Rhino interior comparison. No new licensed capture was
needed for this milestone.

[Local transitions](local-gordon-curve-clock.md) add an explicit clock policy
that keeps affine arc-length behavior between contact transition windows. The
existing entry point retains its whole-interval default. The local fixture
retains C2 continuity and all original-image proofs, with a smaller but still
failing Rhino query-output difference at the same 123×123 control count.
