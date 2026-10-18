# Native isocurve pairs

The native surface producer now retains the complete varying-axis NURBS basis
and returns a UV line in the correct coordinates. DivideAlongCreases requests
that native pair directly, replacing its separate UV-segment construction and
reparameterization workaround.

`SmBSplineSurface::CreateIsoParametricCurve` previously fed its extracted curve
through a raw-data constructor that repaired nearby knots. Valid short spans
could merge, and an interval such as `[1e12,1e12+1]` could collapse. The generated
repair adopts the existing extraction allocation into a fresh owning curve,
without copying controls, fitting, knot cleanup or a change of parameterization.
The fresh object has no borrowed data or derived caches. Its cleanup guard takes
over only after allocation ownership transfers; failed extraction or requested
trimming leaves the outputs empty. The pinned third-party checkout is unchanged.

`SmIsoCurve::CreateUVIsoLine` also repeated its constant-U branch, selected the
constant axis's interval and added the domain minimum twice. Its repaired line
is `(constant,t)` over the V interval or `(t,constant)` over the U interval.
Both axes and nonzero/unequal domain origins are covered. The optional UV line
is published only after requested 3D construction and UV creation succeed.

The direct producer regression has sixteen untrimmed rational tensor cases:
both directions, varying-axis widths `2^-40`, `1` and `2^40`, origin `1e12`,
and ordinary or single-ULP end spans. The fixed axis is `[-3,5]`, queried at `-1`.
An independent two-row homogeneous combination supplies every expected control
and weight. Full knots and all weighted controls agree exactly; the UV interval
and evaluated coordinates agree exactly too. Source controls and both knot
vectors remain unchanged. Both returned curves remain readable with the same
data after the source surface is destroyed.

The parent-object control changes knots in twelve cases and fails to supply a
correct UV partner in all sixteen. The repaired producer passes all sixteen.
A separate full-order fixed-axis case verifies that a native rejection leaves
both outputs null and preserves the source. It permits a future exact supported
result, instead of making the current upstream rejection permanent.

AddressSanitizer builds the complete native libraries, C ABI and producer probe.
Both probe modes pass without a sanitizer diagnostic. Leak detection is disabled
because the default process-global API context has no public teardown contract;
this run checks invalid memory access and allocation lifetime. The crease
regressions additionally retain kink/tangent selection, asymmetric charts,
trim holes, shared topology, source purity and idempotence.

```sh
cargo test -p viboceros-smlib --features native --test isocurve_pairs --test face_creases
```

The opt-in CMake target `viboceros_smlib_isocurve_pair_baseline` overrides only the
two producer objects, preserving the other kernel repairs. The
[validation record](native-isocurve-pairs-validation.json) retains its source and
raw output, the candidate/ASan outputs, actual job results and artifact hashes.

This repair does not replace the [checked Rust-facing extractor](smlib-isocurves.md).
That adapter still handles full-order fixed-axis breaks, unclamped fixed-axis
endpoints, extreme gauges and chart conditioning. General optional trimmed-domain
pairing, arbitrary numerical ranges and the native ray-query path remain
unqualified. No new Rhino observation or performance parity is claimed.
