# Fractional curve sampling

[Native parameters](curve-parameters.md) · [One-sided limits](curve-sided-evaluation.md) · [Surface wires](surface-wire-frames.md)

`NurbsCurve::parameter_sampler()` prepares a reusable point sampler without
changing the source's knots, controls, weights, or native domain. Its
`evaluate(fraction)` samples the full domain; `spans()` yields individual span
samplers with their own fractions in `[0,1]`. Fractions are parameter fractions,
not arc-length fractions. Invalid fractions return errors.

```rust
let sampler = curve.parameter_sampler()?;
let third = sampler.evaluate(1.0 / 3.0)?;
for span in sampler.spans() {
    let start = span.evaluate(0.0)?; // outgoing limit
    let end = span.evaluate(1.0)?;   // incoming limit
    // Subdivide this span independently; do not bridge discontinuities.
}
```

## Why a point sampler is different from a native parameter

At baseline `d5b1d2f`, a rational quadratic with controls `(0,0,0)`, `(1,2,0)`,
`(2,0,0)`, weights `1,2,1`, and domain `[0,2]` produced a smooth 16-segment
viewport approximation. Translating all knots exactly by `2^52` reduced it
to only three distinct sample locations. The controls and curve locus had
not changed: the intermediate native parameters had rounded onto unit steps.

A separate two-span discontinuous curve on `[2^52,2^52+2]` missed the end of
its first branch. Display and selection used `next_down()` as an incoming
limit; on a one-ULP-wide span that value is its start, not its end. Interior
stations could also round onto the following branch's start.

The kernel's new `nurbs/sampling` module shifts a same-sign knot domain toward
zero only when every stored knot, including exterior knots, subtracts the
chosen origin exactly. Curves needing no shift, or failing the exact check,
are borrowed; eligible curves get one temporary translated copy per sampler.
The shared error-free difference/origin check lives in `parameter` and is
also used by surface/B-rep frames. No scaling, fitting, or knot collapse is
permitted. The ordinary point evaluator and its exact-arithmetic recovery
remain responsible for homogeneous arithmetic.

Each span uses explicit right/left endpoint evaluation. Even if an interior
fraction rounds to a boundary, it stays on that span's side. Whole-domain
sampling retains the right-hand convention at interior knots. The closed-curve
predicate uses this sampler for its two interior checks, preventing a valid
closed cubic on a two-ULP/one-ULP native grid from being mistaken for a point.

`evaluate(t)` and `parameter_at(fraction)` still have their original native
contracts. A returned `f64` cannot encode an absent intermediate parameter.
Clients needing only geometry should avoid that intermediate conversion.

## Display and selection

`viewport/curve_sampling` owns one model-space segment visitor used by GPU
staging, click distance, and window/crossing projections. NURBS curves and
NURBS polycurve leaves use the kernel sampler. Analytic line/polyline leaves
use stored endpoints, and arc leaves use their direct angular fraction.
Native analytic domains therefore do not quantize these display stations.
Failed point evaluations break the polyline instead of connecting across them.

This remains the existing fixed subdivision scheme (16 segments per NURBS
span), not adaptive screen-space tessellation or an error-bounded curve mesh.
The visitor emits segments directly and does not allocate sampled point arrays.

## Validation

Native tests cover analytic rational coordinates with both common weight signs,
positive/negative knot offsets through `2^53`, unclamped multispan curves,
reversal, closed cubics, true discontinuities, nonshiftable exterior knots,
invalid fractions, overflowing domain widths, subnormal intervals, extreme
weight ranges, and zero-weight errors. The source/native API checks are retained.
Viewport tests verify translated GPU lines, projected segments, click distances,
NURBS leaves, analytic leaves, and failed-sample breaks.

The workspace run and complete updated oracle suite verify **2,517 distinct
Rust tests**, with 18 intentionally ignored, plus **195 Python tests**.
Formatting, strict all-target Clippy, and warning-free geometry documentation
builds pass. The documentation audit also corrected two pre-existing comments
whose array/interval notation was interpreted as broken links.

The [12-operation fixture](../tools/rhino_oracle/fixtures/curve_parameter_samples.json)
compares whole-domain and per-span samples of rational, C0, unclamped, and
closed curves, each local and translated in both directions. A licensed Rhino
8.32 run in private Xvfb passes every case at absolute/relative `1e-12`; the
maximum recorded coordinate difference is **`1.1102230246251565e-15`**.
The [native response](curve-parameter-sampling-native-reference.json),
[Rhino response](curve-parameter-sampling-rhino-reference.json), and
[comparison](curve-parameter-sampling-rhino-comparison.json) retain the results.

This is explicitly a **shape** comparison: the worker normalizes its privately
owned Rhino reference to `[0,1]` through public
[Curve.Domain](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/P_Rhino_Geometry_Curve_Domain.htm),
then samples public span domains and sided point evaluations. It records the
original domain separately and disposes its reference on success or failure.
It does not test Rhino's raw large-domain fraction rounding or closed-state
classification. Internal full-order discontinuities are tested natively;
OpenNURBS rejects those knot multiplicities. Sample agreement is not a general
curve-distance proof. Harness timings include different work and are not kernel
speed ratios.

```sh
cargo test -p viboceros-geometry nurbs::sampling
cargo test -p viboceros-oracle curve_parameter_samples
tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_parameter_samples.json \
  --absolute-epsilon 1e-12 --relative-epsilon 1e-12
```

## Cost and limits

The [release microbenchmark](../crates/viboceros-geometry/examples/parameter_sampling.rs)
compares the old viewport sampling loop with the framed loop, including frame
creation on each iteration. Five alternating-order rounds of 20,000 iterations
sample cubic curves with 4 and 19 controls (1 and 16 spans), on local and
`±1e12` domains. [Raw timings](curve-parameter-sampling-performance.csv) show
median increases of `1.1–1.9%` for local domains and `2.4–6.4%` for translated
domains: approximately 10–62 ns per single-span curve and 332–421 ns per
16-span curve. Other stress tests were active; this is a bounded cost check,
not a performance guarantee. Run it with:

```sh
cargo run --release -p viboceros-geometry --example parameter_sampling
```

The sampler does not recover already-rounded input knots, guarantee resolution
of arbitrarily narrow relative spans, or translate an unsafe exterior knot.
[Exact fractional recovery](curve-fractional-recovery.md) now handles declined
shifts, subnormal stations, adjacent-float spans, and positional discontinuities
without changing those knots; ordinary samples still use the fast path.
Closest-parameter queries, parameter-returning structure edits, and other
algorithms not using this sampler still have their own native-grid limits.
No stored geometry is silently reparameterized.
