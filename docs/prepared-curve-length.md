# Prepared curve length queries

`viboceros_geometry::ArcLengthSampler` prepares integration, inversion and
optional lookup brackets for repeated queries on a native curve. It supports
lines, conics, polylines, NURBS and polycurves through the existing curve kernel.

Use `try_new(CurveRef, tolerance)` to borrow a curve or
`try_new_owned(Arc<Curve3>, tolerance)` to retain shared immutable geometry.
Owned samplers can move independently of the caller without a self-reference;
cloning the caller's Arc does not duplicate the curve. Prepare lookup brackets
before sharing a sampler through an Arc.

`distance_at_parameter` maps a source parameter to distance from the active
start. `parameter_at_distance`, `point_at_distance` and `sample_at_distance`
perform the reverse query. Samples include the native parameter, point and unit
tangent. Parameters keep the original domain even when integration uses a private
normalized representation. The original source is available through `source_curve`.

`parameters_at_normalized_lengths(&fractions)` inverts a batch of arc-length
fractions in `[0, 1]`. It preserves input order, accepts repeated fractions and
empty batches, and inverts each distinct fraction once. It validates the entire
batch before inversion and permits up to 1,048,576 entries. The existing precise
scalar solver and optional prepared brackets perform every inversion.
For example, `[1., 0., 0.5, 0.5]` returns the end, start and two identical
midpoint parameters. Very narrow or shifted domains can quantize returned
parameters; `sample_at_distance` retains the stable geometric point directly.

`prepare_budgeted_repeated_sampling(preferred_subdivisions)` accelerates repeated
queries with integration prefix brackets. It reduces the requested density to fit
the existing aggregate limit of 1,048,576 parameter/length pairs, or retains
uncached integration when even the smallest table cannot fit. Brackets still
refine their queries using the existing integration and inversion tolerances.
A zero requested density is invalid. Failed preparation retains the prior tables.

Distances must be finite and lie in `[0, total_length()]`; parameters must be
finite and lie in the active source domain. Total length must be positive and
finite. Exact source endpoints and the kernel's stable linear-span sampling are
retained. Corners use the existing right-sided tangent policy, with the terminal
sample following the last segment. This API does not wrap closed curves or
extend open ones beyond their domains.

The owned and borrowed paths share one implementation. Regression tests compare
cached non-unit rational NURBS queries and verify that the owned sampler retains
and releases its source. The existing length tests also exercise extreme
parameter ranges, tiny terminal spans, rational-line speeds, composites and
forward/inverse correspondence.

This shared ownership supports the ongoing [Flow implementation](flow-development.md).

The Python oracle operation `arc_length_batch` accepts a `curve` NURBS definition
(`degree`, `control_points`, `knots`, optional `domain`) and `fractions`. It
returns `parameters`, evaluated `points`, `length` and the source `domain`.
The native probe requests a tight absolute tolerance, sorts and deduplicates
queries before calling the public batch API, and restores their original order.
Oracle batches are limited to 4,096 requests; the shared kernel API has the
larger bound described above.

The [batch validation](arc-length-batch-validation.json) covers 14 native SDK
cases and 1,815 requests: spatial and rational NURBS, exact knot refinement,
weighted lines, multiple spans, conics, stationary endpoints, non-unit domains,
unordered and repeated fractions, dense batches and empty input. All sampled
geometry agrees within the document's `1e-6` tolerance. Thirteen cases also
agree in raw parameters and coordinates within `1e-8`.

The remaining raw-parameter discrepancy is retained for the stationary-endpoint
curve `C(t) = (t², 0, 0)`. Rust follows the analytic inverse `t = sqrt(f)`;
Rhino's batch result differs by up to `6.89e-5` in parameter and `1.33e-7` in
position despite a tight requested absolute tolerance. Tests preserve this
observation and check the analytic result directly. Raw parameter error near a
stationary point therefore needs separate interpretation from geometric error.

Replay the independent batch path and compare its retained native observations:

```sh
cargo run -p viboceros-oracle -- tools/rhino_oracle/fixtures/arc_length_batch.json /tmp/length-batch-local.json
python3 -m tools.rhino_oracle.arc_length_batch_comparison tools/rhino_oracle/fixtures/arc_length_batch.json /tmp/length-batch-local.json tools/rhino_oracle/observations/arc_length_batch_rhino_20261016.json /tmp/length-batch-report.json
```

Precise independent inversion remains available alongside the incremental
lookup used by Flow. Automatic spatial direction selection remains unfinished.

`integration_checkpoints(fractional_tolerance)` returns accepted Cash–Karp 5/4
integration checkpoints. Each `ArcLengthCheckpoint` exposes the original source
parameter, approximate cumulative distance and embedded local error estimate.
The trace splits at native span boundaries, evaluates the interior derivative
on each side of a join, and scales speed by each span's precise length before
adaptive error control. Constant-speed and linear spans reuse their existing
lengths. Shifted or very narrow source domains can quantize checkpoint
parameters while integration retains its normalized domain.

The tolerance must be finite and strictly between zero and one. A trace permits
65,536 attempted adaptive steps across all spans and at most 1,048,576 returned
checkpoints; failures return no partial result. The embedded error is an
estimate. Linear interpolation between these checkpoints has no geometric
accuracy guarantee and does not replace precise inversion or its brackets.

The [integration comparison](../tools/rhino_oracle/observations/flow_integration_report_20261016.json)
retains 40 public Rhino SDK length queries: both guides of a rational quarter
circle, a spatial cubic, its exact knot refinement and a mixed polycurve, at five fractional
tolerances. All forward integrals agree within `1e-10 * max(1, length)`; the
observed maximum relative difference is `7.26e-15`. The same captures retain
prefix lengths, scalar inverses and four batch densities. Their inverse
differences remain visible alongside 32 matching independent inverse batches
containing 7,696 requested parameters. Maximum inverse parameter error is below
`4.3e-12`. This does not qualify every tolerance and curve family in Rhino's API.
Twelve observed linear inverse runs coincide with independently generated
integration checkpoints to below `1.6e-12` in normalized distance. These fitted
lines are diagnostic report outputs only.

```sh
cargo run -p viboceros-oracle -- tools/rhino_oracle/fixtures/flow_integration.json /tmp/flow-integration-local.json
python3 -m tools.rhino_oracle.flow_integration_report tools/rhino_oracle/fixtures/flow_integration.json /tmp/flow-integration-local.json tools/rhino_oracle/observations/flow_integration_rhino_20261016.json /tmp/flow-integration-report.json
```

The scalar rule uses the standard Cash–Karp tableau and the step controller
also present in the Apache-2.0
[usd-brep integrator](../third_party/usd-brep/source/SMLib/src/SmIntegrator.cpp).
This Rust path remains available without the optional native kernel.
Rhino observations are comparison expectations; neither native parameters nor
fitted coefficients construct its checkpoints.

`parameters_at_incremental_normalized_lengths(&fractions, fractional_tolerance)`
implements the sequential inverse. Fractions must be nondecreasing, finite and
inside `[0,1]`; repeats and empty batches are accepted. Each approximate result
becomes the next origin, so earlier queries affect later results. The inverse
retains checkpoints separated by more than one percent of each integration
interval and tightens its root brackets with newly integrated checkpoints.
Its per-increment stopping error scales with the complete integrated length,
or the containing component's length for polycurves. Errors can accumulate;
use the precise batch API for independent high-accuracy sampling.

Tolerance must lie strictly in `(0,1)`. The input limit is 1,048,576 fractions.
Initial integration permits 65,536 attempted steps; inverse refinements share
a separate 65,536-step budget and permit 80 bracket updates per query. Failure
returns no partial result and leaves prepared precise brackets intact. Tests
cover component scaling, repeated knots, model scales from `1e-200` to `1e200`,
shifted and quantized domains, invalid requests and exhausted budgets.

Flow builds 841 normalized inverse parameters with a `1e-6` integration policy,
interpolates that table in both directions, and obtains its guide lengths with
the separate `1e-8` forward policy. The retained native expectations use public
batch tolerances of `1e-4` for NURBS and `1e-6` for polycurves. This explicit
Flow policy does not implement a general translation of every public Rhino
fractional-tolerance setting. No native inverse parameters initialize the map.
