# Fractional curve derivatives

[Fractional samples](curve-parameter-sampling.md) · [Exact recovery](curve-fractional-recovery.md) · [Near calibration](near-snaps.md)

Whole-curve and individual-span samplers now expose
`evaluate_with_derivative(fraction) -> (Point3, Vector3)`. The vector is the first
derivative with respect to that sampler's `[0,1]` fraction, **not** native knot
parameter speed or an arc-length tangent. Source knots, controls and weights stay
unchanged. This supplies first derivatives for the upcoming projected Near query;
native Near capture and UI integration remain pending.

```rust
let sampler = curve.parameter_sampler()?;
let (point, whole_derivative) = sampler.evaluate_with_derivative(0.3)?;
for span in sampler.spans() {
    let (start, outgoing) = span.evaluate_with_derivative(0.0)?;
    let (end, incoming) = span.evaluate_with_derivative(1.0)?;
}
```

## Numerical contract

For a fractional interval `[a,b]`, `dC/df = (b-a) dC/dt`. Computing native speed
first and multiplying afterwards is insufficient: the native derivative may
overflow on a tiny domain or underflow on a huge one even when fractional speed
is ordinary. A rational line with controls `0,2` and weights `1,2`, for example,
has `x(f)=4f/(1+f)` and `dx/df=4/(1+f)^2` even on the smallest subnormal interval.

The ordinary path scales homogeneous derivative controls before evaluation and
the rational quotient rule. It shares local-coordinate centering, weight-gauge
normalization and guarded homogeneous preparation with native curve jets. Range
loss in control differences, interval arithmetic, scaled coefficients or quotient
terms triggers exact recovery. Suspect zero derivatives after cancellation are
also recovered; true stationary or constant coordinates return zero, not an
arbitrary limiting tangent.

The cancellation check includes the derivative control envelope. Looking only
at the final quotient terms can miss a residual already lost in homogeneous
blending. For the quadratic with X controls `0,0.5,-0.5`, binary64 `f=1/3`
has exact fractional speed `2^-54`; rounded complement arithmetic can double
that small speed. The recovered result retains the independent exact value.

The exact path forms the fractional parameter and interval width as binary
rationals, evaluates the shared exact homogeneous derivative net, applies the
quotient rule and interval scale, then rounds the final point/vector. It does not
round a native speed first. It covers the existing sampler's exceptional station
cases as well as mixed weights and derivative range failures. A true pole remains
`ZeroWeightAtParameter`; an unrepresentable fractional derivative remains
`NonFinite` even if its point is representable.

Span endpoints stay on their own side of a knot, including a positional jump.
Whole-domain sampling uses the right-hand branch at exact interior knots and the
left at the natural end. At a discontinuity these are one-sided branch values,
not a derivative across the jump. An exact binary64 fraction slightly below a
knot remains on the incoming branch even if its rounded native parameter would
land on the knot.

## Validation and limits

Analytic tests cover domain widths from the smallest subnormal through `1e308`
and an overflowing `[-1e308,1e308]` width; large and unshiftable knot origins;
positive/negative/extreme weight gauges; full-order discontinuities; translated
quadratics; zero derivatives; poles; invalid fractions; subnormal outputs and
truly overflowing fractional speeds.

The [independent reference generator](../tools/numerics/generate_fractional_derivative_reference.py)
uses Python `Fraction` basis-function sums and their derivatives, not production
de Boor evaluation or engine outputs. Its 493 rows include degrees 1–8,
unclamped/multispan curves and sided endpoints. Tests compare exact recovery
bit-for-bit with those references and check public dispatch with a per-vector
relative error bound `2e-13` (one smallest-subnormal floor). The existing native
jet and fractional-point reference tables regenerate unchanged.

```sh
python3 tools/numerics/generate_fractional_derivative_reference.py | \
  diff - crates/viboceros-geometry/src/nurbs/sampling/derivative_reference.txt
cargo test --release -p viboceros-geometry nurbs::sampling
```

Verification checkpoint: 3,103 release-mode workspace tests and 298 Python tests
pass; 28 opt-in Rust diagnostics/GPU tests are ignored by the ordinary run.
Formatting, all-target Clippy and workspace Rustdoc pass with warnings denied.

## Local cost diagnostic

The [example](../crates/viboceros-geometry/examples/fractional_derivative_sampling.rs)
measures four stations on one rational cubic, excluding curve/frame construction
but including per-query derivative control preparation. Five rounds rotate
domain order and alternate the two unit-domain methods. The
[raw timing record](curve-fractional-derivatives-performance.csv) gives these
approximate medians in nanoseconds per point/derivative pair:

| Domain / method | ns/query |
| --- | ---: |
| Unit domain, native parameter API | 141 |
| Unit domain, fractional span API | 112 |
| Knot origin `1e12`, fractional | 112 |
| Width `1e-170`, fractional | 124 |
| Width `1e170`, fractional | 124 |
| Smallest subnormal width, exact fractional | 74,790 |

Each ordinary entry averages 80,000 queries per round; each subnormal entry
averages 200. The machine was not CPU-pinned and workspace tests were active;
the first unit-domain round was about twice as slow as later rounds. These are
bounded local diagnostics, not a frame-time guarantee or Rhino comparison.
The exact subnormal path is substantially more expensive and remains an
optimization target for repeated closest-point work.

```sh
cargo run --release -p viboceros-geometry --example fractional_derivative_sampling
```

This does not make ordinary floating-point jets universally correctly rounded,
recover already-rounded input knots, or certify a global closest-point solver.
Derivative overflow is an error, not a normalized-tangent fallback. Repeated
queries currently rebuild first-derivative controls; query-local coefficient
reuse and end-to-end Near timing remain future work. Existing point-only sampling
and native-parameter derivative APIs are unchanged. These independent numerical
checks are not new Rhino UI observations or a cross-engine performance claim.
