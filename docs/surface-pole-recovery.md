# Rational surface poles and finite-jet recovery

[Surface jets](surface-evaluation.md) · [Exact evaluation](surface-rational-range.md) · [Grid evaluation](surface-grid-evaluation.md)

## Reproduced failures

A bilinear patch with U domain `[0,1.5]`, weights `[1,-2]` repeated in V,
and X controls `[0,1]` has

```text
W(u,v) = 1 - 2u
x(u,v) = -4u / (3(1-2u))
```

At native `u=0.5`, the true denominator is zero. The previous floating-point
recurrence first rounded `u/1.5` to binary64 `1/3`, leaving a small nonzero
denominator and returning X=`-6004799503160661` instead of an error. Checking
only failed or nonfinite projections cannot detect this failure, including in
the grid cache. Coincident control locations do not make a zero denominator valid.

Continuation needs the same care even with common-sign controls: weights
`[1,4]` on that domain give `W=1+2u`, with a pole at `u=-0.5` outside the domain.
Previously, an adjacent representable station was also rejected as a pole.
No tolerance band around zero correctly distinguishes these stations.

Finally, a constant patch with weights `[1,2]` and a minimum-subnormal U width
has finite zero Euclidean derivatives, although its homogeneous weight derivative
overflows binary64. Previously it returned `NonFinite` for that intermediate
quantity. Constant equal-weight extrapolation can likewise overflow a blend
factor without making the requested geometric jet unrepresentable.

## Dispatch policy

Validation and exact one-sided span selection happen first. The existing
arbitrary-precision rational evaluator then handles:

- mixed signs within the selected active control rectangle;
- variable-weight boundary-span continuation outside the native domain;
- the existing preparation range-loss conditions;
- any remaining failure of ordinary floating-point evaluation.

It uses the original controls, weights, knots, and parameters, checks the exact
homogeneous denominator, and rounds only requested final components. The exact
recurrences and quotient rules did not change. The old uncentered failure retry
is replaced by this single recovery path. Source geometry remains unchanged.

The grid cache uses exact homogeneous U contractions for flagged nets, including
when a rounded projection would have succeeded. It does not project intermediate
rows, which may have zero weight even when the final point is regular. Exact
controls are shared across adjacent U stations within a span, and contractions
are reused until the V span changes. Regular same-sign nets, including
all-negative gauges, retain floating-point evaluation and cached contractions.
Calling an extended API at an in-domain station does not itself force exact work.

Exactly equal weights throughout the active rectangle prove a constant nonzero
denominator, including under continuation. This polynomial case stays fast. Its
shared de Boor recurrence uses `a + alpha*(b-a)` with fused multiply-add outside
knot intervals, retaining constant components even when `1-alpha` loses its unit
term. Nonfinite differences/factors still trigger exact recovery. Equality is
tested on original weights, not a tolerance or normalized approximation; weights
outside the active rectangle are irrelevant. Ordinary in-domain recurrence order
is unchanged.

This exception matters in practice: an intermediate implementation routed every
continuation exactly and made the existing degree-eleven periodic point-grid
construction test take minutes in debug. Polynomial dispatch restores that
focused test to **0.81 seconds**, without weakening its independently derived
orientation check.

The shared [exact dyadic specialization](exact-dyadic-evaluation.md) further
avoids repeated fraction reductions when control values and exact recurrence
factors have power-of-two denominators. This applies to guarded curves and UV
curves too; general-rational factors retain the original exact path.

## Validation

Ten analytic geometry tests cover genuine poles, adjacent finite stations,
first/second partials on both sides, common-negative gauges, U/V swaps, normal
error propagation, all four crossed one-sided limits, ordered grid callbacks,
source preservation, constant-jet recovery, polynomial jets and huge-parameter
constant coordinates, exact active-weight equality, and the ordinary fast path.

The independent Python `Fraction`/Bernstein table grows from 70 to **150 cases**.
Its original 70 range-loss records remain unchanged. New records include ordinary
mixed-sign nets through degree `(5,4)`, same-sign continuation, genuine poles and
both adjacent floats, nonzero mixed partials, and constant subnormal-domain jets.
Rust compares each requested order's final binary64 bits and errors, and checks
in-domain grid points directly against the same independent reference. Reference
generation does not call the Rust evaluator. Grid/scalar parity is separate
regression evidence, not a substitute for those mathematical checks.

```sh
python3 tools/numerics/generate_surface_rational_reference.py | \
  diff - crates/viboceros-geometry/src/nurbs_surface/evaluate/exact/reference.txt
cargo test -p viboceros-geometry nurbs_surface::evaluate::tests::poles
cargo test -p viboceros-geometry exact_surface_jets_match
cargo test -p viboceros-oracle pole_recovery_fixture
```

A fresh Rhino **8.32.26160.13001** capture in a private Xvfb/i3 session compares
four fixtures with sixteen regular stations, both global weight signs, and
continuation. All pass at absolute/relative `1e-12`; the maximum component
difference is **2.842170943040401e-14**. Native fixture tests also check gauge
identity and analytic mixed partials for `S=(x(u),v,x(u)v)`.

- [Native response](surface-pole-recovery-native-reference.json)
- [Rhino response](surface-pole-recovery-rhino-reference.json)
- [Comparison and raw harness timings](surface-pole-recovery-rhino-comparison.json)

```sh
tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/surface_pole_recovery.json --timeout 240 \
  --absolute-epsilon 1e-12 --relative-epsilon 1e-12
```

Exact pole/adjacent-float classification and subnormal-domain recovery are
mathematical tests, not attempts to copy Rhino's behavior at undefined points.

Final verification passes **2,541 Rust tests** in the complete release workspace,
with 18 existing opt-in/ignored tests, and **195 Python tests**. Strict all-target
Clippy, warnings-denied Rustdoc, formatting, deterministic reference regeneration,
and saved-response comparison reproduction also pass. The complete debug geometry
suite, including all 17,820 grid cells, passes separately; duplicate debug oracle
work was stopped after the complete release suite passed. Geometry tests took
15.22 seconds in release versus 533.26 seconds in the debug run on this host.

## Cost and remaining limits

The [before/after measurement](surface-pole-recovery-performance.json) compares
the preserved `f4a9a11` release oracle with the final implementation on nine
ordinary closest-point cases, five rounds of 200 iterations, pinned to CPU 5.
All **190 query results** remain bit-identical. Median curved-query changes range
from **+0.84% to +2.67%**; affine controls range from **-0.97% to +0.03%**.
One exhaustive debug grid test was still running, last observed on CPU 15;
pinning does not remove clock/load noise. This measures ordinary guard overhead,
not exceptional exact-path cost, and is not an isolated performance guarantee.

Arbitrary-precision evaluation is substantially more expensive than binary64.
In the saved exceptional-case capture, four-jet native batches took roughly
113–188 microseconds, versus 16–232 microseconds for Rhino's harness. These
uncontrolled harness measurements include different overheads and are not
kernel-speed comparisons; the native exact path is slower in three of four records.
This checkpoint does **not** establish the project's desired Rhino speed parity.

Exact-path time and memory depend on degree and rational coefficient sizes.
Exact grids reuse row contractions, and dyadic recurrences reduce arithmetic
overhead. Two existing retained-basis sweep fixtures nevertheless illustrate a
remaining expensive case: regular degree-(5,2) patches have one negative control
weight despite their positive denominator. Repeated closest-point queries use
exact scalar jets during refinement. An arbitrary near-zero cutoff cannot
replace the denominator check to recover performance.

The [adverse-workload record](surface-pole-recovery-signed-cost.json) makes this
regression explicit. For the two sweep constructions with one closest query
each, three alternating-order release process measurements give medians of
**5.28 ms before** and **293.02 ms after** (about **55.5× slower**). Definitions
and sampled points remain bit-identical. These are whole-process times, not
kernel timings. Exact half-subdivision of the stored Bernstein weight rows
independently proves a denominator lower bound of
`7543498508633175 / 36028797018963968 > 0` for both patches.

The initial correctness-only implementation took 2.95 seconds on this request;
exact grid reuse reduced it to 1.20 seconds and dyadic arithmetic to roughly
0.30 seconds in single coarse measurements. That improvement does not erase
the regression against the old floating-point path. The complete existing
sweep integration test passes in release mode (51.74 seconds in the focused
run); debug execution is much slower. README therefore recommends running the
complete, unfiltered test suite with `cargo test --workspace --release`.

Ordinary same-sign fast evaluation is not universally correctly rounded, and
this change does not solve every unflagged intermediate range-loss problem.
It does not locate unsampled poles, certify trimmed topology, implement singular
limiting normals, or change surface construction and parameter-mapping policies.
