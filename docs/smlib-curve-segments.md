# Native curve segment extraction

Linux `native-smlib` builds use usd-brep's `N_CrvExtractCrvSeg` for partial NURBS
segments in intersection-overlap outputs and surface-cut clipping. These paths
share one command backend helper. Parsing, intersection classification, surface
arrangement, attributes, source identity, grouping, selection and atomic history
keep their existing policies. [Directed segments and parameter splits](smlib-directed-segments.md)
now reuse the same producer with shared seam policies. Normalized-end-weight
and numeric/midpoint sampling paths remain separate qualification work.

The adapter accepts the complete original curve definition, retains its degree,
parameter interval, interior knots and common weight gauge, and copies the native
result into validated Rust geometry. Kernel calls use the shared process lock;
no native handle enters document history. Whole-domain trims return the original
definition. Two-control polynomial lines use the shared exact affine path to
preserve zero crossings required by topology.

Private binary chart conditioning is exactly reversible for every knot and
requested bound. Coordinate origins come from active controls and must round-trip
all source coordinates, sharing the checked conditioning used by native Bezier
conversion. Source definitions are unchanged. Every output knot and control count
is checked against the expected clamped interval before projection.

The [repaired native extractor](native-curve-segment-limits.md) uses actual
endpoint multiplicities and exact knot groups. Unclamped active ends and inactive
suffix controls no longer need the adapter's reverse/split/reverse clamp or
suffix removal. Full-order endpoint payloads are copied from their exact source
rows, including interior groups and inactive padding. The historical
[direct probe](../tools/usd_brep/segment_extraction_probe.cpp) and
[limitations ledger](usd-brep-limitations.md) retain the original discrepancy.
Unclamped native point evaluation remains a separate limitation.

The public `trim_curve` API follows the existing geometry interval checks.
Native limits remain degree 17 and one million controls. Binary weight gauges
that underflow, final zero/nonfinite weights and nonfinite Euclidean controls
fail explicitly. Arbitrary extreme span ratios remain unqualified; this is
bounded fixture evidence, not a continuous native-validity or speed certificate.

Fresh Rhino SDK captures contain nine parameter trims, including unclamped and
periodic curves, active endpoints, one-span trims and knot boundaries. Eight
complete raw definitions match at absolute `1e-10`, relative `1e-12`. The full-domain
no-op deliberately preserves its two source outer entries while Rhino's codec
pads its omitted entries with neighboring values. The primary comparison retains
that raw mismatch. All nine reconstructed reference definitions agree with the
native results over 65 stations per interval. Native tests also cover source
purity, full-order breaks, inactive endpoint controls, chart scales `2^-360`,
`1`, `2^360`, and common weight gauges `1e-300`, `1`, `1e300`, `-1e300`.

The Python worker validates finite parameters before allocating a source and
releases owned source/results on invalid intervals, SDK failure and serialization
failure. These lifecycle boundaries are tested explicitly.

The [retained comparison](smlib-curve-segments-comparison.json) includes every raw
trim outcome, input/observation hashes and the direct native endpoint probe.
Fresh isolated command captures also retain all 121 curve/surface/B-rep
intersection and surface-cutting cases; all match at absolute `1e-8`, relative
`1e-12`, including complete output geometry and document records. These captures
use scheme `VibocerosOracleSmlibBridgeFinal20261010`; their timings include the
harness and are not kernel-speed measurements.

The original qualification passed 3,555 native geometry/command/bridge tests (11 ignored),
the expanded four segment regressions, all 1,097 Python tests, formatting and
workspace lint in native and portable configurations. The existing 165 retained
native editing/decomposition comparisons also still match.

```sh
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo test --release -p viboceros-smlib \
  --features native --test curve_segments
cmake --build /tmp/viboceros-usd-brep-build --target \
  viboceros_usd_brep_segment_extraction_probe -j 10
/tmp/viboceros-usd-brep-build/viboceros_usd_brep_segment_extraction_probe
```
