# Native curve self-intersections

Linux `native-smlib` builds use usd-brep's `SmCurve::GlobalCurveSelfIntersect`
for IntersectSelf and SelSelfIntersectingCrv. Supported inputs enter one native
global search instead of the command's recursive span comparison. Unsupported
working frames retain the portable search; supported native failures propagate.
Native overlap output extraction uses the shared native segment adapter;
unsupported preparation also retains portable extraction. Output
attributes, selection, source preservation, limits and atomic history remain in
the command layer.

The adapter shares [curve-intersection preparation and acceptance](smlib-curve-intersections.md):
translated/scaled coordinates, unit parameter charts and common signed weight
gauges, copied event ownership, original-domain restoration, original-curve
point residuals, bounded local polishing and symmetric sampled overlap checks.
The native solver excludes equal-parameter visits; the adapter also excludes
closed endpoint seams. Degree above 17, mixed/excessive weight ratios,
full-order interior breaks and unrepresentable frames decline native preparation.
Source bounds are validated before entering the kernel, including pole rejection.

A [scoped source repair](../third_party/patches/usd-brep-curve-self-intersection-pairs.patch)
visits each unordered self-intersection span pair once. Without this repair,
the solver combines mirrored range proposals for a backtracking polyline into
one folded interval spanning both visits. Its endpoints coincide, but trimming
that interval produces the duplicated path rather than one overlap branch.
The repair culls mirrored pairs before local solving, retains correspondence
direction through range merging and requires both paired endpoint parameters to
match when joining self ranges. The shared Rust finalizer also keeps opposite
directions separate. Curve pairs using separate native trees retain their
existing native search. CMake applies the repair
to a generated copy after the existing convergence repairs, with fuzz disabled.
The pinned upstream source remains unchanged.

[Retained qualification](smlib-curve-self-intersections-comparison.json) contains
36 analytic C ABI cases in normal and full AddressSanitizer builds: a single-span
cubic crossing, a closed cubic seam, crossing and backtracking polylines, a
simple closed polygon and a straight curve, in two charts and three planes.
Expected paired parameters, physical residuals, complete source definitions,
owned copies, invalid tolerances and invalid input are checked. The same inputs
without the self-pair repair pass 30/36 cases; the six backtracking cases retain
the erroneous folded ranges. Seven additional multi-visit collinear paths check
chronological paired endpoints, nonzero intervals and complete scalar image
bounds from their degree-one knots; these pass in normal and ASan builds.
They exposed range merges that collapsed one parameter interval at a turnaround.
SMLib, SM_API and the bridge are instrumented;
process-global context leak accounting alone is disabled.

Rust tests cover 216 chart/plane/common-gauge combinations, including parameter
scales `2^-360` and `2^360`, plus a rational circle, a projected crossing with
distinct spatial heights, unsupported degree and pole rejection. Command tests
cover basic results, source purity, selection, fallback and Undo/Redo.
Degree-18 retracing curves explicitly exercise portable overlap extraction.

The Python oracle now accepts `self_intersect_probe` through its existing
`OracleClient`. A fixture supplies `curve` or `vertices`; `include_selection`
observes SelSelfIntersectingCrv. `include_api=true` additionally returns native
paired parameters and requires `native_smlib=True`. The portable command replay
disables that optional native-only API field; the retained requests and complete
responses make this distinction explicit.

Ten basic command/selection cases from saved Rhino records form the command
parity gate. All 29 saved cases are retained, including the already documented
nested-collinear overlap and backtracking discrepancies. Full raw Rhino parity
is not claimed; command fields and native API events have separate comparisons.
Fresh Rhino captures remain blocked by the existing Wine licensing startup
failure. Sampled acceptance does not certify continuous overlap or event
completeness, and performance relative to Rhino remains unqualified.

```sh
cargo test --release -p viboceros-smlib --features native --test curve_self_intersections
cargo test --release -p viboceros-command --features native-smlib --lib intersect_self
cargo test --release -p viboceros-oracle --features native-smlib --lib self_intersect
cmake -S tools/usd_brep -B /tmp/self-intersections -G Ninja \
  -DCMAKE_BUILD_TYPE=Release \
  -DVIBOCEROS_CURVE_INTERSECTION_DISTANCE_PATCH=ON \
  -DVIBOCEROS_CURVE_SELF_INTERSECTION_PAIRS_PATCH=ON
cmake --build /tmp/self-intersections --target viboceros_usd_brep_self_intersection_adapter_probe
/tmp/self-intersections/viboceros_usd_brep_self_intersection_adapter_probe
```

For baseline diagnostics, disable only the self-pair repair. ASan uses
`RelWithDebInfo`, compiler flags `-fsanitize=address -fno-omit-frame-pointer` and
linker flags `-fsanitize=address`. See
`python3 -m tools.usd_brep.qualify_curve_self_intersections --help` for retained
comparison inputs.
