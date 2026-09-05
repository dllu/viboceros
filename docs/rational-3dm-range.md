# Rational 3DM numeric range

[File formats](file-formats.md) · [Full-order curves](curve-3dm-interchange.md)

The Rust kernel stores Euclidean control points and separate rational weights.
OpenNURBS stores homogeneous coordinates `(x*w, y*w, z*w, w)`. Multiplying those
values naively can overflow or silently erase a coordinate even when every input
coordinate and weight is finite. For example, `1e-200 * 1e-200` rounds to zero;
the previous writer exported an incorrect origin-valued control without error.

## Shared conversion policy

The bridge's `rational_coordinates.h` is shared by free NURBS curves and surfaces,
PolyCurve NURBS leaves, B-rep edge curves, UV trims, and face surfaces. Before
writing a complete control polygon or net, it:

1. Tries the unchanged weights first. Ordinary safe definitions retain their
   original weights; no unconditional normalization is performed.
2. Checks finite, nonzero weights and valid homogeneous coordinates, then checks
   Euclidean recovery by both direct division and OpenNURBS' reciprocal-based
   getter. Zero and nonzero coordinates cannot be silently interchanged. Other
   coordinates must recover within eight machine epsilons relatively.
3. If necessary, chooses one common binary exponent for the entire polygon/net.
   `scalbn` changes the weights without needing a representable standalone scale
   factor. Reversing that exponent must recover each original weight exactly.
   Therefore signed weight ratios are preserved, not fitted or individually
   normalized.
4. Prefers normal-range homogeneous values. If that is impossible, checks the
   remaining exponent candidates, including subnormal rounding boundaries,
   against the same recovery contract. A finite reciprocal can exist in the
   `2^-1024` exponent bin; products just above half the smallest subnormal can
   also be usable when the original coordinate recovers.

Failure to find a safe common scale is reported before the staged export replaces
its destination. The source model and document remain unchanged. This may change
absolute file weights, but not their projective meaning. The existing export
report's `adapted_curve_count` counts structural full-order decomposition, not
numeric weight rescaling.

Independent full-order curve pieces are decomposed **before** this conversion,
so each piece can retain or choose its own homogeneous scale. This numeric change
does not extend full-order decomposition to B-rep topology or surface knots.

## Import and signed surfaces

Import reads the actual homogeneous controls and divides by the weight directly.
It does not first compute `1/w`, which can overflow for a subnormal weight even
when the final Euclidean coordinates are ordinary finite numbers. This is used
for both flat and binary-codec geometry paths.

The B-rep surface decoder now accepts nonzero signed weights, consistently with
the kernel, free-surface path, and OpenNURBS validation. It previously rejected
negative weights solely in the B-rep payload path.

## Validation and limits

Thirteen native tests cover product underflow/overflow, subnormal weights and
coordinates, ordinary-weight preservation, negative global scales, full-order
piece integration, atomic failure, and the rounding-boundary cases above. B-rep
tests retain vertices, edge/trim associations, intervals, loop and trim types,
orientation, tolerances, and Euclidean controls across extreme scales. A checked-in
[pre-normalization file](../crates/viboceros-io/tests/fixtures/README.md) independently
protects subnormal-weight import; regenerating it with the new writer would no
longer exercise that condition.

The four `rational_3dm_range.json` cross-reader cases passed in licensed Rhino 8
on 2026-09-04. They inspect the actual Viboceros-written curve files at **zero
absolute epsilon**, relative `1e-12`, so erasing tiny coordinates cannot pass by
falling below an absolute threshold. The native probe additionally checks 65
native-parameter samples per written curve against the original source, also
without an absolute floor: two readers agreeing on an incorrect file is not
sufficient. Coordinate magnitudes range from `1e-200`
to `1e200`; input weights include `1e-320` and `-1e308`. Every file includes all
four visibility/locking combinations. This is curve-reader evidence, not an
independent Rhino surface/B-rep comparison or a kernel performance benchmark.

```sh
tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/rational_3dm_range.json \
  --absolute-epsilon 0 --relative-epsilon 1e-12
```

The format still has finite precision and OpenNURBS' coordinate validity limits.
Some polygons/nets have no common representable scale. Control recovery does not
prove a uniform curve/surface error near a signed-weight pole, nor repair data
already rounded away in an old file. Kernel within-span evaluation limitations
are documented separately in [rational numerics](nurbs-numerics.md).
