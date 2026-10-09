# Rebuild trim-transfer performance

[Rebuild guide](commands/rebuild-surfaces.md) · [Certificate contract](surface-curve-certificates.md)

The October 8, 2026 measurement on this DGX Spark separates surface rebuilding
from `Brep::try_retrimmed_single_surface`. Source construction, geometry serialization
and output disposal are outside the measured calls. Three release-mode repetitions
of a radius-two sphere cap rebuilt to 10×10 degree-three controls took a median
64.7758 seconds to retrim at code `3356050b`.

Temporary phase instrumentation located the cost: image certification took about
63 seconds, projection about 0.28 seconds across three trims, and final B-rep
validation about 0.003 seconds. That instrumentation was removed from production.
An interval composition proof reduced the measured median to 7.1878 seconds;
interval knot-crossing boxes reduced it to 1.7712 seconds. Reusing coefficient
tables within an immutable surface certificate reduced it to 1.6077 seconds.
The recorded vertex positions, spline controls/weights/knots and topology have
the same canonical SHA-256 fingerprint in every before/after sphere-cap run.
The 73-edge representation and continuous `1e-6` trim/image tolerance are retained.
This is about a 40× reduction for this case, not a general kernel speedup.

A subsequent preparation cache reduces the sphere-cap median from 1.6077 to
1.3905 seconds in three separate repetitions, retaining the same geometry
fingerprint. Fixed projection isocurves are prepared once per trim, and interval
patch controls are converted once per immutable tensor patch. Unknown interval
conversions retain exact fallback. The swapped-cap and cylinder-band timings stay
near their preceding values; this is a case-specific measured gain. See
[cache profiles and hashes](retrim-cache-provenance.json). The earlier proof
sources are retained under `retrim-performance-source-73553044/`, preserving their
original hashes as the implementation evolves.

A prepared surface context then reduces the sphere-cap median from 1.3905 to
0.8764 seconds. Independent cubic image proposals and their recursive children
reuse exact tensor extraction, converted interval patches and coefficient tables.
Each proposal receives a fresh 2,000,000-unit rational work budget, including the
original surface preparation charge; cache hits charge the work actually performed.
The bound arithmetic, tolerances and subdivision limits remain unchanged. Tests
compare repeated prepared proofs with standalone proofs across signed/extreme
weight gauges, distinct domains, accepted/rejected offsets and invalid limits.
The swapped-cap median is 1.3417 seconds and cylinder-band median 0.5220 seconds,
both near their preceding values. All three complete recorded geometry definitions
remain identical. See [prepared-context profiles and hashes](retrim-prepared-provenance.json).
The preceding cache source files are retained under `retrim-cache-source/`.
Reusing cached work can change which proposals fit the work budget; these cases
do not establish general resource-admission equivalence or native performance parity.

Projection preparation then reduces the swapped-cap median from 1.3417 to
1.2219 seconds. Temporary phase timings located about 0.95 seconds in its fixed
contour projection. Its isocurve now prepares original-coordinate seed points and
a translated proposal curve once. Every seed still refines independently, and
final comparisons use the original curve/target. Failed target translation retains
the original-coordinate refinement path. An opt-in query retains at most 64 spans
of floating or exact jet coefficients, without retaining station values or pole status.
Chart fitting separately caches successful projections at identical UV bit patterns,
including periodic lifts, with at most 131,072 entries. Independent tests compare
prepared searches with standalone searches under signed poles, overflowing offsets,
large distance ties and misleading long spans, and compare retained jets with public
jets after eviction and invalid queries. Sphere/cylinder medians are 0.8644/0.5143
seconds, near their preceding values; every complete recorded output is unchanged.
See [projection profiles and hashes](retrim-projection-provenance.json).
The earlier prepared-context sources are preserved under `retrim-prepared-source/`.
This remains a bounded numerical closest search, without a global minimum certificate.

Curve coefficient state now persists across independent targets within the prepared
fixed-isocurve search. Original-coordinate ranking, translated refinement and
failed-translation refinement each own separate bounded states. Exact coefficient
nets store controls and span indices; each evaluation supplies its unchanged curve
and rechecks the station denominator. No previous target, derivative value or pole
status is reused. State-transfer tests compare public sided jets after invalid
queries and eviction; a directed test alternates ordinary and overflowing target
translations. Three-run sphere/swapped/cylinder medians are 0.8524/1.2066/0.5171
seconds, with identical complete outputs. These small changes do not establish a
general speedup. See [cross-target profiles and hashes](retrim-target-cache-provenance.json).
The preceding projection sources remain in `retrim-projection-source/`.

The [raw profiles and hashes](retrim-performance-provenance.json) retain all stages,
three final cases and their original source definitions. A fresh private-Xvfb
[SDK capture](../tools/rhino_oracle/observations/surface_retrim_profile.json)
uses those exact serialized NURBS source surfaces, matching UV trim bounds and
counts/degrees. It times public `Surface.Rebuild` and `Brep.CreateTrimmedSurface`,
retaining complete inputs/outputs and unchanged sources. Native rebuilding controls
match local controls at `1e-6`, with knots at `1e-12`. The native sphere-cap median
retrim time is 0.02768 seconds: a substantial performance gap remains.
Rhino runs through Python/RhinoCommon and Wine/FEX, while the local kernel also
constructs continuously certified spatial edges. Those differing contracts and
one-host, three-repetition measurements do not establish general performance parity.

## Continuous interval proof

Exact rational extraction still defines each UV/spatial Bézier span and tensor
patch. A fast path is used for nonlinear UV spans against cubic spatial spans.
Every rational-to-float conversion is checked against the exact rational value,
and adjacent finite floats must enclose it. Basic arithmetic widens each operation
outward using [`next_up` and `next_down`](https://doc.rust-lang.org/std/primitive.f64.html).
Exact zero/one identities are preserved. Overflow, subnormal operands/results,
uncertain denominator signs and inconclusive bounds return to exact arithmetic.
The floating arithmetic does not require a changed processor rounding mode.

Bernstein tensor composition and homogeneous image-minus-spatial products are
evaluated as intervals. With positive denominator coefficient lower bounds, the
complete rational difference lies in the convex hull of its Cartesian controls.
Outward normalized squared norms bound that hull. A reported Euclidean bound is
checked again with exact rational squared arithmetic; square root only proposes
the reported value. A proved endpoint outside tolerance rejects the correspondence;
an undecided hull subdivides at dyadic stations. This is a continuous proof,
not acceptance from sampled curve points.

At tensor-knot crossings, interval polar-form restriction encloses the same
exact patch/spatial boxes used by the preceding rational method. A sufficiently
small box proves correspondence. A lower bound above tolerance proves only that
this box method cannot certify the current interval, so subdivision continues;
it never declares the geometry outside tolerance from that fact. Uncertain cases
use the exact box computation. Each floating attempt has 65,536 work units and
eight hull-subdivision levels; the existing rational work, degree, bit-size and
subdivision limits remain in place. Coefficient tables are local to the prepared
surface, bounded by supported degrees, and shared across its proof spans.

Independent tests compare interval arithmetic with exact binary64 rational results,
compare accepted/rejected curved-span and knot-crossing box decisions with exact
proofs, and exercise cancellation, overflow, subnormal and zero limits. A curved
surface excursion invisible at all four cubic interpolation stations must still
be rejected. Native Rebuild replay retains its preceding physical boundary and
document-history tolerances.

## Reproduce

```sh
cargo run --release -p viboceros-geometry --example profile_surface_retrim -- sphere_cap 3
cargo run --release -p viboceros-geometry --example profile_surface_retrim -- swapped_cap 3
cargo run --release -p viboceros-geometry --example profile_surface_retrim -- cylinder_band 3
tools/rhino_oracle/run_headless.sh rhino tools/rhino_oracle/fixtures/surface_retrim_profile.json --scheme VibocerosOracleSurfaceReTrimProfileLocal --timeout 600 --output /tmp/retrim-native.json
cargo test --release -p viboceros-geometry surface_pullback::certificate
python3 -m unittest tools.rhino_oracle.test_retrim_performance
```

Run profiling separately from compilation and other tests. Current rendering,
document transactions, larger control nets and arbitrary geometry families are
outside these timing measurements. See [oracle timing interpretation](oracle.md#timing-interpretation).
