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
